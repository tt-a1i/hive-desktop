import { createHash } from 'node:crypto';
import { open, stat } from 'node:fs/promises';
import type { FileHandle } from 'node:fs/promises';
import { historyWindowResult, resolveHistoryWindow, StaleHistorySnapshotError, type HistoryWindowOptions } from '../history-cursor.js';

export interface ByteSpan { start: number; end: number }
export interface IndexedHistoryRow extends ByteSpan { id: string }

export interface SavedHistorySource {
  snapshot: string;
  page(options: HistoryWindowOptions): Promise<ReturnType<typeof historyWindowResult<unknown>>>;
  byId(id: string, options?: {unique?: boolean}): Promise<unknown | undefined>;
  validate(): Promise<void>;
}

function fileVersion(info: Awaited<ReturnType<typeof bigintStat>>): string {
  return [info.dev, info.ino, info.size, info.mtimeNs, info.ctimeNs].join(':');
}

const bigintStat = (path: string) => stat(path, { bigint: true });

export async function savedHistoryFileVersion(path: string, optional = false): Promise<string> {
  try { return fileVersion(await bigintStat(path)); }
  catch (error) {
    if (optional && (error as NodeJS.ErrnoException).code === 'ENOENT') return 'missing';
    throw error;
  }
}

async function validateFileVersion(file: string, expected: string, optional = false): Promise<void> {
  if (await savedHistoryFileVersion(file, optional).catch(() => undefined) !== expected) throw new StaleHistorySnapshotError();
}

export interface SavedHistoryVersion {
  file: string;
  version: string;
  optional?: boolean;
}

interface HistoryIndex {
  rows: IndexedHistoryRow[];
  focus: Map<string, number>;
  toolFocus: Map<string, number>;
  ids?: Map<string, {first: number; count: number}>;
}

const MAX_CACHED_INDEXES = 8;
const indexes = new Map<string, Promise<HistoryIndex>>();

/** Keep byte ranges and row identities in memory. The saved file owns row bodies. */
export async function openIndexedHistory<T extends HistoryIndex>(options: {
  cacheKey: string;
  versions: SavedHistoryVersion[];
  signal?: AbortSignal;
  snapshot?: string;
  build: (handle: FileHandle) => Promise<T>;
  project: (row: unknown, index: T) => unknown;
}): Promise<SavedHistorySource & { index: T }> {
  const { cacheKey, versions, signal, build, project } = options;
  const primary = versions[0]!;
  const snapshot = createHash('sha256').update(JSON.stringify([cacheKey, versions])).digest('base64url');
  if (options.snapshot !== undefined && options.snapshot !== snapshot) throw new StaleHistorySnapshotError();
  const validate = async () => {
    signal?.throwIfAborted();
    await Promise.all(versions.map(({ file, version, optional }) => validateFileVersion(file, version, optional)));
    signal?.throwIfAborted();
  };
  const access = async <R>(fn: (handle: FileHandle) => Promise<R>) => {
    signal?.throwIfAborted();
    const handle = await open(primary.file, 'r').catch(() => { throw new StaleHistorySnapshotError(); });
    try {
      if (fileVersion(await handle.stat({ bigint: true })) !== primary.version) throw new StaleHistorySnapshotError();
      const result = await fn(handle).catch(async error => {
        await validate();
        throw error;
      });
      await validate();
      return result;
    } finally {
      await handle.close();
    }
  };
  await validate();
  const cached = indexes.get(snapshot);
  const pending = cached ?? access(build);
  indexes.delete(snapshot);
  indexes.set(snapshot, pending);
  if (indexes.size > MAX_CACHED_INDEXES) indexes.delete(indexes.keys().next().value!);
  let index: T;
  try {
    index = await pending as T;
    await validate();
  }
  catch (error) {
    if (indexes.get(snapshot) === pending) indexes.delete(snapshot);
    throw error;
  }
  const { rows, focus, toolFocus } = index;
  if (!index.ids) {
    index.ids = new Map();
    for (const [offset, row] of rows.entries()) {
      const prior = index.ids.get(row.id);
      if (prior) prior.count++;
      else index.ids.set(row.id, {first: offset, count: 1});
    }
  }
  const ids = index.ids;
  const findId = (id: string) => ids.get(id)?.first ?? -1;
  const findFocus = (id: string) => {
    const idx = findId(id);
    return idx >= 0 ? idx : focus.get(id) ?? toolFocus.get(id) ?? -1;
  };
  async function readRow(handle: FileHandle, {start, end}: IndexedHistoryRow) {
    const data = Buffer.allocUnsafe(end - start);
    let read = 0;
    while (read < data.length) {
      signal?.throwIfAborted();
      const { bytesRead } = await handle.read(data, read, Math.min(data.length - read, CHUNK_BYTES), start + read);
      if (bytesRead === 0) throw new StaleHistorySnapshotError();
      read += bytesRead;
    }
    return project(JSON.parse(data.toString('utf8')), index);
  }
  return {
    snapshot, index, validate,
    async page(window) {
      if (window.snapshot !== undefined && window.snapshot !== snapshot) throw new StaleHistorySnapshotError();
      const { startIndex, endIndex } = resolveHistoryWindow(rows.length, window, findId, findFocus, idx => rows[idx]?.id);
      const selected = rows.slice(startIndex, endIndex);
      const history = await access(async handle => {
        const result: unknown[] = [];
        for (const row of selected) result.push(await readRow(handle, row));
        return result;
      });
      return historyWindowResult(history, selected.map(row => row.id), rows.length, startIndex, snapshot);
    },
    async byId(id, options) {
      const match = ids.get(id);
      if (!match || (options?.unique && match.count !== 1)) { await validate(); return; }
      return access(handle => readRow(handle, rows[match.first]!));
    },
  };
}

const CHUNK_BYTES = 65_536;

// The saved formats are JSON objects containing potentially huge arrays. Visit
// one member at a time and retain byte offsets only for requested array items.
// JSON.parse validates every individual value, including members unused by paging.
export async function visitSavedHistoryJson(
  handle: FileHandle,
  indexedArrays: ReadonlySet<string>,
  visit: (key: string, value: unknown, span: ByteSpan, part: 'field' | 'array' | 'item') => void,
  signal?: AbortSignal,
): Promise<void> {
  type State = 'object' | 'firstKey' | 'key' | 'colon' | 'value' | 'firstItem' | 'item' | 'afterItem' | 'afterValue' | 'done';
  let state: State = 'object';
  let key: string;
  const keys = new Set<string>();
  let token: { start: number; kind: 'key' | 'item' | 'field'; parts: Buffer[]; quoted: boolean; escape: boolean; depth: number; primitive: boolean } | undefined;
  const invalid = () => new Error('Invalid saved history JSON');
  const whitespace = (byte: number) => byte === 32 || byte === 10 || byte === 13 || byte === 9;
  const {size} = await handle.stat();
  let offset = 0;
  while (offset < size) {
    signal?.throwIfAborted();
    const chunk = Buffer.allocUnsafe(CHUNK_BYTES);
    const { bytesRead } = await handle.read(chunk, 0, Math.min(chunk.length, size - offset), offset);
    if (bytesRead === 0) throw new StaleHistorySnapshotError();
    let segmentStart = 0;
    const finish = (end: number) => {
      const current = token!;
      current.parts.push(chunk.subarray(segmentStart, end));
      const bytes = current.parts.length === 1 ? current.parts[0]! : Buffer.concat(current.parts);
      const value: unknown = JSON.parse(bytes.toString('utf8'));
      if (current.kind === 'key') {
        if (typeof value !== 'string' || keys.has(value)) throw invalid();
        key = value;
        keys.add(key);
        state = 'colon';
      } else {
        visit(key, value, {start: current.start, end: offset + end}, current.kind);
        state = current.kind === 'item' ? 'afterItem' : 'afterValue';
      }
      token = undefined;
    };
    for (let i = 0; i < bytesRead; i++) {
      const byte = chunk[i]!;
      if (token) {
        if (token.quoted) {
          if (token.escape) token.escape = false;
          else if (byte === 92) token.escape = true;
          else if (byte === 34) {
            token.quoted = false;
            if (token.depth === 0) finish(i + 1);
          }
          continue;
        }
        if (token.primitive) {
          if (byte !== 44 && byte !== 93 && byte !== 125 && !whitespace(byte)) continue;
          finish(i);
        } else {
          if (byte === 34) token.quoted = true;
          if (byte === 91 || byte === 123) token.depth++;
          if (byte === 93 || byte === 125) {
            token.depth--;
            if (token.depth === 0) finish(i + 1);
          }
          continue;
        }
      }
      if (whitespace(byte)) continue;
      switch (state as State) {
        case 'object':
          if (byte !== 123) throw invalid();
          state = 'firstKey';
          continue;
        case 'firstKey':
          if (byte === 125) { state = 'done'; continue; }
          // falls through
        case 'key':
          if (byte !== 34) throw invalid();
          break;
        case 'colon':
          if (byte !== 58) throw invalid();
          state = 'value';
          continue;
        case 'value':
          if (byte === 91 && indexedArrays.has(key!)) {
            visit(key!, undefined, {start: offset + i, end: offset + i + 1}, 'array');
            state = 'firstItem';
            continue;
          }
          break;
        case 'firstItem':
          if (byte === 93) { state = 'afterValue'; continue; }
          break;
        case 'item':
          break;
        case 'afterItem':
          if (byte === 93) { state = 'afterValue'; continue; }
          if (byte !== 44) throw invalid();
          state = 'item';
          continue;
        case 'afterValue':
          if (byte === 125) { state = 'done'; continue; }
          if (byte !== 44) throw invalid();
          state = 'key';
          continue;
        case 'done':
          throw invalid();
      }
      if (byte === 44 || byte === 93 || byte === 125) throw invalid();
      segmentStart = i;
      token = {start: offset + i, kind: state === 'key' || state === 'firstKey' ? 'key' : state === 'value' ? 'field' : 'item',
        parts: [], quoted: byte === 34, escape: false, depth: byte === 91 || byte === 123 ? 1 : 0,
        primitive: byte !== 34 && byte !== 91 && byte !== 123};
    }
    if (token) token.parts.push(chunk.subarray(segmentStart, bytesRead));
    offset += bytesRead;
  }
  if (state !== 'done') throw invalid();
}

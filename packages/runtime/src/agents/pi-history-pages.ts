import { createHash } from 'node:crypto';
import type { BigIntStats } from 'node:fs';
import { open, stat, type FileHandle } from 'node:fs/promises';
import {
  historyRowFocusId, historyToolCallFocusIds, historyWindowResult, readHistoryJsonlLines, resolveHistoryWindow, StaleHistorySnapshotError,
  type HistoryWindowOptions,
} from '../history-cursor.js';
import { assertPiTranscriptHeader, externalPiMessage, parsePiEntry, piHistoryMessageId } from './external-pi-history.js';
import { collectPiInputSources, piInputEvent, piUserInput, piUserInputNote } from './pi-user-input.js';

type Source = {file: string; nativeId: string; backend: 'pi' | 'pi-rpc'; scope: string};
type Position = {start: number; length: number; id: string; parentId: string | null};
type Input = {deliveryId: string};
type Entry = Position & {
  event: ReturnType<typeof piInputEvent<Input>>;
  messageId: string | undefined;
  focus: string | undefined;
  calls: string[];
};
type Row = {id: string; entry: Position; kind: 'note' | 'message'; input?: Position};
type Index = {
  rows: Row[];
  exact: Map<string, number>;
  focus: Map<string, number>;
  calls: Map<string, number>;
  duplicateIds: Set<string>;
  imageAliases: Map<string, number>;
};

function fingerprint(info: BigIntStats): string {
  if (!info.isFile() || info.size > BigInt(Number.MAX_SAFE_INTEGER)) throw new Error('Invalid Pi transcript file');
  return [info.dev, info.ino, info.size, info.mtimeNs, info.ctimeNs, info.birthtimeNs].join(':');
}

function parse(text: string): unknown {
  // The original reader also recognizes bare CR as a line delimiter. Use
  // that reader for such files instead of building incompatible offsets.
  if (text.slice(0, text.endsWith('\r') ? -1 : undefined).includes('\r')) throw new Error('Unsupported Pi transcript line endings');
  return JSON.parse(text);
}

async function buildIndex(
  file: FileHandle, size: number, source: Source, read: (size: number) => void, signal?: AbortSignal,
): Promise<Index> {
  const byId = new Map<string, Entry>();
  let header = false;
  let leaf: string | null = null;
  for await (const {start, raw} of readHistoryJsonlLines(file, size, {onRead: read, ...(signal && {signal})})) {
    const text = raw.toString('utf8');
    if (!text.trim()) continue;
    const value = parse(text);
    if (!header) {
      assertPiTranscriptHeader(value, source.nativeId);
      header = true;
      continue;
    }
    const entry = parsePiEntry(value);
    if (byId.has(entry.id)) throw new Error('Invalid Pi transcript entry');
    const message = externalPiMessage(entry, undefined, source.backend);
    const event = piInputEvent(entry, ({deliveryId}) => ({deliveryId}));
    byId.set(entry.id, {
      id: entry.id, parentId: entry.parentId, start, length: raw.length, event,
      messageId: message && piHistoryMessageId(entry.id, message),
      focus: historyRowFocusId(message), calls: historyToolCallFocusIds(message),
    });
    leaf = entry.id;
  }
  if (!header) throw new Error('Missing Pi transcript header');

  const branch: Entry[] = [];
  const visited = new Set<string>();
  while (leaf !== null) {
    const entry = byId.get(leaf);
    if (!entry || visited.has(leaf)) throw new Error('Invalid Pi transcript branch');
    visited.add(leaf);
    branch.push(entry);
    leaf = entry.parentId;
  }
  branch.reverse();
  const {users, notes} = collectPiInputSources(branch);
  const rows: Row[] = [];
  const exact = new Map<string, number>();
  const focus = new Map<string, number>();
  const calls = new Map<string, number>();
  const duplicateIds = new Set<string>();
  const imageAliases = new Map<string, number>();
  function add(row: Row, alias?: string, toolIds: string[] = []) {
    if (exact.has(row.id)) duplicateIds.add(row.id);
    else exact.set(row.id, rows.length);
    if (alias !== undefined && !focus.has(alias)) focus.set(alias, rows.length);
    for (const id of toolIds) if (!calls.has(id)) calls.set(id, rows.length);
    rows.push(row);
  }
  for (const entry of branch) {
    const note = notes.get(entry.id);
    const input = users.get(entry.id);
    if (note) add({id: note.deliveryId, entry, kind: 'note'}, note.deliveryId);
    if (entry.messageId !== undefined) {
      if (source.backend === 'pi' && input) {
        const alias = input.input.deliveryId;
        imageAliases.set(alias, imageAliases.has(alias) ? -1 : rows.length);
      }
      add({
        id: entry.messageId, entry, kind: 'message', ...(input && {input: byId.get(input.entryId)!}),
      }, input?.input.deliveryId ?? entry.focus, entry.calls);
    }
  }
  return {rows, exact, focus, calls, duplicateIds, imageAliases};
}

/** Indexes one verified Pi journal version. No message bodies are retained between requests. */
export class PiHistoryPages {
  private cache: {stamp: string; index: Promise<Index>} | undefined;

  constructor(private readonly recordBytes: (phase: 'index' | 'page', size: number) => void = () => {}) {}

  page(source: Source, options: HistoryWindowOptions, signal?: AbortSignal) {
    return this.read(source, options.snapshot, signal, async (index, readRow, stamp) => {
      const {startIndex, endIndex} = resolveHistoryWindow(index.rows.length, options,
        id => index.exact.get(id) ?? -1,
        id => index.exact.get(id) ?? index.focus.get(id) ?? index.calls.get(id) ?? -1,
        position => index.rows[position]?.id);
      const history: unknown[] = [];
      const ids: string[] = [];
      for (const row of index.rows.slice(startIndex, endIndex)) {
        history.push(await readRow(row));
        ids.push(row.id);
      }
      return historyWindowResult(history, ids, index.rows.length, startIndex, stamp);
    });
  }

  imageRow(source: Source, id: string, signal?: AbortSignal) {
    return this.read(source, undefined, signal, async (index, readRow) => {
      const exact = index.exact.get(id);
      const alias = index.imageAliases.get(id);
      if (index.duplicateIds.has(id) || alias === -1
        || (exact !== undefined && alias !== undefined && exact !== alias)) return;
      const position = exact ?? alias;
      if (position !== undefined) return readRow(index.rows[position]!);
      return;
    });
  }

  private async read<T>(
    source: Source, snapshot: string | undefined, signal: AbortSignal | undefined,
    select: (index: Index, readRow: (row: Row) => Promise<unknown>, stamp: string) => Promise<T>,
  ): Promise<T> {
    signal?.throwIfAborted();
    const file = await open(source.file, 'r');
    try {
      const info = await file.stat({bigint: true});
      const stamp = createHash('sha256').update(JSON.stringify([
        source.scope, source.backend, source.nativeId, source.file, fingerprint(info),
      ])).digest('base64url');
      if (snapshot !== undefined && snapshot !== stamp) throw new StaleHistorySnapshotError();
      const cached = this.cache?.stamp === stamp ? this.cache : undefined;
      const index = await (cached ?? (this.cache = {
        stamp, index: buildIndex(file, Number(info.size), source, size => this.recordBytes('index', size), signal),
      })).index.catch(error => {
        if (this.cache?.stamp === stamp) this.cache = undefined;
        throw error;
      });

      async function check() {
        const before = fingerprint(info);
        const after = await file.stat({bigint: true});
        const current = await stat(source.file, {bigint: true});
        if (fingerprint(after) !== before || fingerprint(current) !== before) throw new StaleHistorySnapshotError();
        signal?.throwIfAborted();
      }
      await check();
      const readEntry = async (position: Position) => {
        signal?.throwIfAborted();
        const bytes = Buffer.allocUnsafe(position.length);
        let offset = 0;
        while (offset < bytes.length) {
          const {bytesRead} = await file.read(bytes, offset, bytes.length - offset, position.start + offset);
          this.recordBytes('page', bytesRead);
          if (!bytesRead) throw new StaleHistorySnapshotError();
          offset += bytesRead;
        }
        const entry = parsePiEntry(parse(bytes.toString('utf8')));
        if (entry.id !== position.id || entry.parentId !== position.parentId) throw new StaleHistorySnapshotError();
        return entry;
      };
      const readRow = async (row: Row) => {
        const entry = await readEntry(row.entry);
        const marker = row.kind === 'note' ? entry : row.input ? await readEntry(row.input) : undefined;
        const input = marker?.type === 'custom' ? piUserInput(marker.data) : undefined;
        const message = row.kind === 'note'
          ? input && piUserInputNote(entry, input, source.backend)
          : externalPiMessage(entry, input, source.backend);
        if (!message || (row.kind === 'note' ? input?.deliveryId : piHistoryMessageId(entry.id, message)) !== row.id)
          throw new StaleHistorySnapshotError();
        return message;
      };
      const result = await select(index, readRow, stamp);
      await check();
      return result;
    } finally {
      await file.close();
    }
  }
}

import { createHash, randomUUID } from 'node:crypto';
import { lstat, mkdir, rename, rm, writeFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { DatabaseSync } from 'node:sqlite';
import { historyRowFocusId, historyToolCallFocusIds, historyWindowResult, resolveHistoryWindow, StaleHistorySnapshotError } from '../history-cursor.js';
import { claudeMetadataPath, readClaudeMetadata } from './claude-metadata.js';
import { ClaudeProjection } from './claude-projection.js';
import { resolveClaudeTranscript } from './claude-transcript-source.js';
import { readSessionMessages } from './claude.js';
import { savedHistoryFileVersion, type SavedHistorySource } from './saved-history-index.js';

interface ClaudeSavedHistoryOptions {
  cacheDirectory: string;
  metadataDirectory: string;
  /** Desktop record ID and pinned profile reference. */
  scope: string;
  sessionId: string;
  cwd: string;
  env: NodeJS.ProcessEnv;
  /** An imported conversation must continue to resolve to its original path. */
  nativePath?: string;
  snapshot?: string;
  signal?: AbortSignal;
  assertCurrent?: () => void;
}

const version = 'claude-0.3.317-projection-1';
const building = new Map<string, { directory: string; pending: Promise<void> }>();
const hash = (fields: unknown[]) => createHash('sha256').update(JSON.stringify(fields)).digest('base64url');

export function claudeHistoryCacheDirectory(dataDirectory: string, workspaceId: string): string {
  return join(dataDirectory, 'cache', 'claude-history-pages', hash([workspaceId]));
}

async function assertCacheDirectory(directory: string): Promise<void> {
  const parent = dirname(directory);
  for (const path of [dirname(parent), parent, directory]) {
    const info = await lstat(path).catch(error => {
      if ((error as NodeJS.ErrnoException).code !== 'ENOENT') throw error;
    });
    if (info && !info.isDirectory()) throw new Error('Claude history cache path is not a local directory');
  }
}

/** The caller has blocked new history reads for the workspace. */
export async function removeClaudeHistoryCache(dataDirectory: string, workspaceId: string): Promise<void> {
  const directory = claudeHistoryCacheDirectory(dataDirectory, workspaceId);
  await Promise.allSettled([...building.values()].filter(build => build.directory === directory).map(build => build.pending));
  try { await lstat(directory); }
  catch (error) {
    if (['ENOENT', 'ENOTDIR'].includes((error as NodeJS.ErrnoException).code ?? '')) return;
    throw error;
  }
  await assertCacheDirectory(directory);
  await rm(directory, { recursive: true, force: true });
}

function readIndex<T>(path: string, snapshot: string, read: (db: DatabaseSync, total: number) => T): T {
  const db = new DatabaseSync(path, { readOnly: true });
  try {
    db.exec('PRAGMA trusted_schema=OFF');
    const generation = db.prepare('SELECT snapshot, total FROM generation').get();
    if (generation?.snapshot !== snapshot || typeof generation.total !== 'number' || !Number.isSafeInteger(generation.total) || generation.total < 0)
      throw new StaleHistorySnapshotError();
    return read(db, generation.total);
  } finally {
    db.close();
  }
}

async function buildIndex(path: string, snapshot: string, options: ClaudeSavedHistoryOptions, validate: () => Promise<void>): Promise<void> {
  const { cacheDirectory, metadataDirectory, sessionId, cwd, env, signal } = options;
  const metadata = readClaudeMetadata(metadataDirectory, sessionId);
  const messages = await readSessionMessages(sessionId, cwd, env, signal, options.nativePath);
  if (!messages.length || messages.some(message => message.session_id !== sessionId))
    throw new Error('Claude SDK did not return history with the expected native session identity');

  const projection = new ClaudeProjection(() => metadata);
  for (const message of messages) projection.put(message);
  const { history, historyMessageIds } = projection.snapshot();
  await validate();

  const temporary = `${path}.${randomUUID()}.tmp`;
  await mkdir(cacheDirectory, { recursive: true, mode: 0o700 });
  await assertCacheDirectory(cacheDirectory);
  await writeFile(temporary, '', { flag: 'wx', mode: 0o600 });
  try {
    const db = new DatabaseSync(temporary);
    try {
      // Only the private staging file is writable. An interrupted build can
      // be discarded in its entirety, so it needs no separate recovery journal.
      db.exec(`PRAGMA journal_mode=OFF; BEGIN;
        CREATE TABLE generation(snapshot TEXT NOT NULL, total INTEGER NOT NULL) STRICT;
        CREATE TABLE messages(position INTEGER PRIMARY KEY, id TEXT NOT NULL UNIQUE, focus TEXT, body TEXT NOT NULL) STRICT;
        CREATE INDEX message_focus ON messages(focus, position);
        CREATE TABLE tool_focus(id TEXT PRIMARY KEY, position INTEGER NOT NULL) STRICT;`);
      const row = db.prepare('INSERT INTO messages VALUES (?, ?, ?, ?)');
      const tool = db.prepare('INSERT OR IGNORE INTO tool_focus VALUES (?, ?)');
      for (const [position, message] of history.entries()) {
        signal?.throwIfAborted();
        row.run(position, historyMessageIds[position]!, historyRowFocusId(message) ?? null, JSON.stringify(message));
        for (const id of historyToolCallFocusIds(message)) tool.run(id, position);
      }
      db.prepare('INSERT INTO generation VALUES (?, ?)').run(snapshot, history.length);
      db.exec('COMMIT');
    } finally {
      db.close();
    }
    await validate();
    await rename(temporary, path);
  } finally {
    await rm(temporary, { force: true });
  }
}

/**
 * The SDK slices after reconstructing its branch. Hive also merges assistant
 * fragments by API message ID, including across SDK windows. Use the existing
 * full projection once per native generation, then seek to display rows in
 * the derived index. The native transcript continues to own the history.
 */
export async function openClaudeSavedHistory(options: ClaudeSavedHistoryOptions): Promise<SavedHistorySource | undefined> {
  const { cacheDirectory, metadataDirectory, scope, sessionId, cwd, env, signal, assertCurrent } = options;
  const unavailable = () => {
    signal?.throwIfAborted();
    if (options.snapshot !== undefined) throw new StaleHistorySnapshotError();
    return undefined;
  };
  signal?.throwIfAborted();
  const native = await resolveClaudeTranscript(sessionId, cwd, env);
  if (!native || (options.nativePath && options.nativePath !== native)) return unavailable();
  const metadata = claudeMetadataPath(metadataDirectory, sessionId);
  const sourceVersion = async () => hash([version, scope, native, metadata, env.CLAUDE_CODE_DISABLE_PRECOMPACT_SKIP,
    await savedHistoryFileVersion(native), await savedHistoryFileVersion(metadata, true)]);
  const snapshot = await sourceVersion().catch(() => undefined);
  if (!snapshot || (options.snapshot !== undefined && options.snapshot !== snapshot)) return unavailable();
  const validate = async () => {
    signal?.throwIfAborted();
    const resolved = await resolveClaudeTranscript(sessionId, cwd, env).catch(() => undefined);
    if (resolved !== native || await sourceVersion().catch(() => undefined) !== snapshot) throw new StaleHistorySnapshotError();
    signal?.throwIfAborted();
    assertCurrent?.();
  };

  // The .index file is reconstructed from the native transcript. It belongs
  // to the desktop's disposable cache alongside temporary build files.
  const path = join(cacheDirectory, `${hash([scope, sessionId])}.index`);
  try { await assertCacheDirectory(cacheDirectory); }
  catch { return unavailable(); }
  const ready = async () => {
    try {
      if (!(await lstat(path)).isFile()) return false;
      return readIndex(path, snapshot, () => true);
    } catch { return false; }
  };
  if (!await ready()) {
    assertCurrent?.();
    const buildKey = JSON.stringify([path, snapshot]);
    const build = building.get(buildKey) ?? {directory: cacheDirectory, pending: buildIndex(path, snapshot, options, validate)};
    building.set(buildKey, build);
    try { await build.pending; }
    catch (error) {
      if (error instanceof StaleHistorySnapshotError) throw error;
      return unavailable();
    } finally {
      if (building.get(buildKey) === build) building.delete(buildKey);
    }
  }
  await validate();
  const access = async <T>(read: (db: DatabaseSync, total: number) => T): Promise<T> => {
    await validate();
    await assertCacheDirectory(cacheDirectory);
    const result = readIndex(path, snapshot, read);
    await validate();
    return result;
  };

  return {
    snapshot, validate,
    async page(window) {
      if (window.snapshot !== undefined && window.snapshot !== snapshot) throw new StaleHistorySnapshotError();
      return access((db, total) => {
        const byId = db.prepare('SELECT position FROM messages WHERE id = ?');
        const byFocus = db.prepare('SELECT position FROM messages WHERE focus = ? ORDER BY position LIMIT 1');
        const byTool = db.prepare('SELECT position FROM tool_focus WHERE id = ?');
        const at = db.prepare('SELECT id FROM messages WHERE position = ?');
        const position = (row: {position?: unknown} | undefined) => typeof row?.position === 'number' ? row.position : undefined;
        const findId = (id: string) => position(byId.get(id)) ?? -1;
        const findFocus = (id: string) => position(byId.get(id)) ?? position(byFocus.get(id)) ?? position(byTool.get(id)) ?? -1;
        const { startIndex, endIndex } = resolveHistoryWindow(total, window, findId, findFocus, index => at.get(index)?.id as string | undefined);
        const rows = db.prepare('SELECT id, body FROM messages WHERE position >= ? AND position < ? ORDER BY position').all(startIndex, endIndex);
        if (rows.length !== endIndex - startIndex) throw new StaleHistorySnapshotError();
        return historyWindowResult(rows.map(row => JSON.parse(row.body as string) as unknown),
          rows.map(row => row.id as string), total, startIndex, snapshot);
      });
    },
    async byId(id) {
      return access(db => {
        const row = db.prepare('SELECT body FROM messages WHERE id = ?').get(id);
        return row && JSON.parse(row.body as string) as unknown;
      });
    },
  };
}

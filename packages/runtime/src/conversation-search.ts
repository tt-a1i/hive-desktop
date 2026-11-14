import { createReadStream } from 'node:fs';
import { stat } from 'node:fs/promises';
import { createInterface } from 'node:readline';
import { getSessionMessages } from '@anthropic-ai/claude-agent-sdk';
import type { SessionEntry } from '@earendil-works/pi-coding-agent';
import type { ChatRecord } from './chat-index.js';
import { externalPiBranch, externalPiMessage } from './agents/external-pi-history.js';
import { piUserInputNote, piUserInputSources } from './agents/pi-user-input.js';
import { readClaudeMetadata } from './agents/claude-metadata.js';
import { ClaudeProjection } from './agents/claude-projection.js';
import { codexMetadataFile, readCodexMetadata } from './agents/codex-metadata.js';
import { normalizeCodexItem, type CodexItem } from './agents/codex-history.js';
import { readFile } from 'node:fs/promises';

export type ConversationSearchMatch = { messageId: string; snippet: string };
export type ConversationSearchResult = { matches: ConversationSearchMatch[]; complete: boolean; warning?: string };
export type ConversationSearchOptions = {
  workspacePath: string;
  codexDirectory: string;
  claudeDirectory: string;
  /** A live snapshot supplies native message IDs; delivery IDs are normalized here. */
  liveHistory?: { history: readonly unknown[]; historyMessageIds: readonly string[]; activeMessage?: { messageId: string; message: unknown } };
};
type Row = { id: string; message: unknown };
// Cache only successfully parsed durable snapshots. A small LRU bounds retained
// transcripts; every lookup stats all dependent files before using a snapshot.
const rowCache = new Map<string, { fingerprint: string; rows: Row[]; bytes: number; complete: boolean }>();
const CACHE_LIMIT = 24;
const CACHE_BYTES = 32 * 1024 * 1024;
async function fingerprint(paths: string[]): Promise<string> {
  return JSON.stringify(await Promise.all(paths.map(async path => {
    try {
      const info = await stat(path, { bigint: true });
      return [path, String(info.dev), String(info.ino), String(info.size), String(info.mtimeNs), String(info.ctimeNs)];
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code === 'ENOENT') return [path, 'missing'];
      throw error;
    }
  })));
}
async function cachedRows(key: string, paths: string[], load: () => Promise<{ rows: Row[]; complete: boolean }>): Promise<{ rows: Row[]; complete: boolean }> {
  const before = await fingerprint(paths);
  const cached = rowCache.get(key);
  if (cached?.fingerprint === before) {
    rowCache.delete(key);
    rowCache.set(key, cached);
    return { rows: cached.rows, complete: cached.complete };
  }
  rowCache.delete(key);
  const loaded = await load();
  if (before !== await fingerprint(paths)) throw new Error('Conversation history changed while being read; retry search');
  // Retain only searchable text, never raw images/provider payloads.
  const compact = loaded.rows.map(row => ({ id: desktopMessageId(row.id, row.message), message: { role: 'assistant', content: messageText(row.message) } }));
  const bytes = compact.reduce((sum, row) => sum + Buffer.byteLength(row.message.content, 'utf8') + Buffer.byteLength(row.id, 'utf8'), 0);
  if (bytes <= CACHE_BYTES) {
    rowCache.set(key, { fingerprint: before, rows: compact, bytes, complete: loaded.complete });
    let total = [...rowCache.values()].reduce((sum, item) => sum + item.bytes, 0);
    while (rowCache.size > CACHE_LIMIT || total > CACHE_BYTES) {
      const oldest = rowCache.keys().next().value!;
      total -= rowCache.get(oldest)!.bytes;
      rowCache.delete(oldest);
    }
  }
  return { rows: compact, complete: loaded.complete };
}
function object(value: unknown): value is Record<string, unknown> { return value !== null && typeof value === 'object' && !Array.isArray(value); }
function fold(value: string): string { return value.normalize('NFC').toLocaleLowerCase(); }
// Match conversation.rs's serde presentation contract before replacing the ID.
function desktopPresentation(value: unknown): Record<string, unknown> | undefined {
  if (!object(value) || value.version !== 1 || typeof value.deliveryId !== 'string' || !value.deliveryId ||
      typeof value.displayText !== 'string' || !['user', 'dispatch', 'report', 'cancel', 'recovery', 'system', 'unknown'].includes(value.origin as string)) return;
  for (const key of ['dispatchId', 'workerName', 'workerId']) if (value[key] != null && typeof value[key] !== 'string') return;
  if (value.artifacts !== undefined && (!Array.isArray(value.artifacts) || !value.artifacts.every(item => typeof item === 'string'))) return;
  if (value.attachments !== undefined && (!Array.isArray(value.attachments) || !value.attachments.every(item => {
    if (!object(item) || !['id', 'workspaceId', 'name', 'mimeType'].every(key => typeof item[key] === 'string') ||
        typeof item.size !== 'number' || !Number.isSafeInteger(item.size) || item.size < 0) return false;
    return item.projectFile == null || (object(item.projectFile) && typeof item.projectFile.relativePath === 'string' && typeof item.projectFile.sha256 === 'string');
  }))) return;
  return value;
}
function desktopMessageId(id: string, message: unknown): string {
  if (!object(message)) return id;
  if (message.role === 'toolResult' && typeof message.toolCallId === 'string') return message.toolCallId;
  if (message.role === 'user' || (message.role === 'custom' && message.customType === 'hive-delivery')) {
    const presentation = desktopPresentation(message.presentation);
    if (presentation) return presentation.deliveryId as string;
  }
  return id;
}

// Search user-visible conversation fields only. Do not search raw provider metadata,
// base64 images, hidden startup instructions, or arbitrary object serialization.
function messageText(value: unknown): string {
  if (!object(value)) return '';
  const presentation = desktopPresentation(value.presentation);
  if (value.role === 'custom' && !presentation) return '';
  const parts: string[] = [];
  if (presentation && typeof presentation.displayText === 'string') parts.push(presentation.displayText);
  else {
    const content = value.content;
    if (typeof content === 'string') parts.push(content);
    else if (Array.isArray(content)) for (const block of content) {
      if (!object(block)) continue;
      if ((block.type === 'text' || block.type === 'input_text' || block.type === 'output_text') && typeof block.text === 'string') parts.push(block.text);
      if (block.type === 'thinking' && typeof block.thinking === 'string') parts.push(block.thinking);
    }
  }
  return parts.join('\n');
}
function find(rows: readonly Row[], terms: string[]): ConversationSearchMatch[] {
  const matches: ConversationSearchMatch[] = [];
  for (const row of rows) {
    const text = messageText(row.message);
    const folded = fold(text);
    if (!terms.every(term => folded.includes(term))) continue;
    // Locate using code points; normalization can change offsets, so the snippet is
    // informative rather than an offset into the original message.
    const position = Array.from(folded.slice(0, Math.max(0, folded.indexOf(terms[0]!)))).length;
    const start = Math.max(0, position - 70);
    const slice = Array.from(text).slice(start, start + 220).join('').replace(/\s+/gu, ' ').trim();
    matches.push({ messageId: row.id, snippet: `${start ? '…' : ''}${slice}${Array.from(text).length > start + 220 ? '…' : ''}` });
  }
  return matches;
}

async function piRows(file: string, backend: 'pi' | 'pi-rpc'): Promise<Row[]> {
  const before = await stat(file);
  const entries: SessionEntry[] = [];
  const seen = new Set<string>();
  let header = false;
  const lines = createInterface({ input: createReadStream(file, { encoding: 'utf8' }), crlfDelay: Infinity });
  for await (const raw of lines) {
    if (!raw.trim()) continue;
    let parsed: unknown;
    try { parsed = JSON.parse(raw); } catch { throw new Error('Malformed Pi transcript'); }
    if (!object(parsed)) throw new Error('Invalid Pi transcript entry');
    if (!header) {
      if (parsed.type !== 'session' || typeof parsed.id !== 'string') throw new Error('Invalid Pi transcript header');
      header = true;
      continue;
    }
    if (typeof parsed.type !== 'string' || typeof parsed.id !== 'string' || !parsed.id ||
        !(parsed.parentId === null || typeof parsed.parentId === 'string') || seen.has(parsed.id)) throw new Error('Invalid Pi transcript entry');
    seen.add(parsed.id);
    entries.push(parsed as unknown as SessionEntry);
  }
  if (!header) throw new Error('Missing Pi transcript header');
  const after = await stat(file);
  if (before.ino !== after.ino || before.size !== after.size || before.mtimeMs !== after.mtimeMs || before.ctimeMs !== after.ctimeMs) throw new Error('Pi transcript changed while being read; retry search');
  const branch = externalPiBranch(entries, entries.at(-1)?.id ?? null);
  const sources = piUserInputSources(branch);
  const rows: Row[] = [];
  for (const entry of branch) {
    const note = sources.notes.get(entry.id);
    if (note) rows.push({ id: note.deliveryId, message: piUserInputNote(entry, note, backend) });
    const message = externalPiMessage(entry, sources.users.get(entry.id), backend);
    if (message) rows.push({ id: entry.type === 'message' && entry.message.role === 'toolResult' && typeof entry.message.toolCallId === 'string' ? entry.message.toolCallId : entry.id, message });
  }
  return rows;
}

async function codexRows(record: ChatRecord, directory: string): Promise<{ rows: Row[]; complete: boolean }> {
  if (!record.codexThreadId) return { rows: [], complete: true };
  const data: unknown = JSON.parse(await readFile(`${codexMetadataFile(directory, record.codexThreadId)}.display.json`, 'utf8'));
  if (!object(data) || data.version !== 1 || !Array.isArray(data.entries)) throw new Error('Invalid Codex display journal');
  const metadata = readCodexMetadata(directory, record.codexThreadId);
  const complete = data.completeHistory === true && Array.isArray(data.searchEntries);
  const entries: unknown[] = complete ? data.searchEntries as unknown[] : data.entries;
  const seenIds = new Set<string>();
  const rows = entries.map((entry: unknown) => {
    if (!object(entry) || typeof entry.turnId !== 'string' || !object(entry.item) || typeof entry.item.id !== 'string' || typeof entry.item.type !== 'string') throw new Error('Invalid Codex display journal entry');
    const item = entry.item as CodexItem;
    if (complete && (!item.id || !entry.turnId || seenIds.has(item.id) || item.status === 'inProgress' || item.type === 'hiveProjectionNotice')) throw new Error('Invalid complete Codex history snapshot');
    seenIds.add(item.id);
    const delivery = metadata.deliveries.find(d => d.turnId === entry.turnId && d.itemId === item.id)
      ?? metadata.deliveries.find(d => d.turnId === entry.turnId && d.itemId === undefined && item.type === 'userMessage');
    return { id: item.id, message: normalizeCodexItem(item, delivery) };
  });
  return { rows, complete };
}

/** Reads fresh persisted history each call: appended or replaced files cannot leave a stale cache. */
export async function searchConversation(record: ChatRecord, query: string, options: ConversationSearchOptions): Promise<ConversationSearchResult> {
  const terms = fold(query.trim()).split(/\s+/u).filter(Boolean);
  if (!terms.length) return { matches: [], complete: true };
  let rows: Row[] = [];
  let warning: string | undefined;
  let codexSynced = false;
  try {
    if (record.backend === 'pi' || record.backend === 'pi-rpc') {
      if (record.sessionFile) {
        const file = record.sessionFile;
        rows = (await cachedRows(`${record.backend}:${file}`, [file], async () => ({ rows: await piRows(file, record.backend as 'pi' | 'pi-rpc'), complete: true }))).rows;
      }
      else if (record.piSessionId || record.externalPiSessionId) warning = 'Pi transcript path is unavailable';
    } else if (record.backend === 'claude') {
      if (record.nativeclaudeSessionId) {
        const sessionId = record.nativeclaudeSessionId;
        const metadata = readClaudeMetadata(options.claudeDirectory, sessionId);
        const projection = new ClaudeProjection(() => metadata);
        const messages = await getSessionMessages(sessionId, { dir: options.workspacePath });
        for (const message of messages) projection.put(message);
        const snapshot = projection.snapshot();
        rows = snapshot.history.map((message, i) => ({ id: desktopMessageId(snapshot.historyMessageIds[i]!, message), message }));
        // The SDK returns [] for both missing and empty sessions and does not expose
        // transcript parse errors. Its result alone cannot prove completeness.
        warning = 'Claude history completeness cannot be verified from the SDK';
      }
    } else if (record.codexThreadId) {
      const metadata = codexMetadataFile(options.codexDirectory, record.codexThreadId);
      const loaded = await cachedRows(`codex:${metadata}`, [metadata, `${metadata}.display.json`], () => codexRows(record, options.codexDirectory));
      rows = loaded.rows;
      codexSynced = loaded.complete;
      if (!loaded.complete) warning = 'Codex display journal may omit native history';
    }
  } catch (error) {
    warning = `Conversation history could not be read: ${error instanceof Error ? error.message : String(error)}`;
  }
  const live = options.liveHistory;
  if (live) {
    if (live.history.length !== live.historyMessageIds.length) {
      warning = 'Live conversation history has mismatched message IDs';
    } else {
      // Pi snapshots are the current native branch; merging older on-disk rows
      // could expose an abandoned branch after an in-memory branch switch.
      const byId = new Map((record.backend === 'pi' || record.backend === 'pi-rpc' ? [] : rows).map(row => [row.id, row]));
      // A synchronized native Codex snapshot is authoritative. Its display
      // journal can contain older items; never reintroduce them by merging.
      if (!codexSynced) live.history.forEach((message, i) => {
        const id = desktopMessageId(live.historyMessageIds[i]!, message);
        byId.set(id, { id, message });
      });
      if (live.activeMessage) {
        const id = desktopMessageId(live.activeMessage.messageId, live.activeMessage.message);
        byId.set(id, { id, message: live.activeMessage.message });
      }
      rows = [...byId.values()];
    }
  }
  return { matches: find(rows, terms), complete: !warning, ...(warning ? { warning } : {}) };
}

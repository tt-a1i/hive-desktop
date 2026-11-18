import { AcpHistory } from './agents/acp-history.js';
import { createReadStream } from 'node:fs';
import { stat } from 'node:fs/promises';
import { createInterface } from 'node:readline';
import type { SessionMessage } from '@anthropic-ai/claude-agent-sdk';
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
  /** Runtime currently processing a turn; some providers emit tool updates outside snapshots. */
  liveBusy?: boolean;
  codexDirectory: string;
  claudeDirectory: string;
  /** Reads the native transcript through the session's pinned profile. */
  readClaudeHistory?: () => Promise<SessionMessage[]>;
  /** A live snapshot supplies native message IDs; delivery IDs are normalized here. */
  liveHistory?: { history: readonly unknown[]; historyMessageIds: readonly string[]; activeMessage?: { messageId: string; message: unknown } };
  /** Conversation lists only show one excerpt per conversation. */
  firstMatchOnly?: boolean;
};
type Row = { id: string; message: unknown };
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
async function freshRows(paths: string[], load: () => Promise<{ rows: Row[]; complete: boolean }>): Promise<{ rows: Row[]; complete: boolean; truncated: boolean; unresolved: boolean }> {
  const before = await fingerprint(paths);
  const loaded = await load();
  if (before !== await fingerprint(paths)) throw new Error('Conversation history changed while being read; retry search');
  // Do not cache by stat metadata: same-size rewrites can preserve timestamps.
  // Every search parses the durable source again; raw provider payloads are not retained.
  const projected = searchableRows(loaded.rows);
  const compact = projected.rows.map(row => ({ id: row.id, message: { role: 'assistant', content: row.text } }));
  return { rows: compact, complete: loaded.complete, truncated: projected.truncated, unresolved: projected.unresolved };
}
function object(value: unknown): value is Record<string, unknown> { return value !== null && typeof value === 'object' && !Array.isArray(value); }
export function foldSearchText(value: string): string { return value.normalize('NFC').toLocaleLowerCase(); }
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

// Bound traversal before serializing the explicitly displayed tool input. Never
// traverse provider envelopes, image data or arbitrary hidden metadata.
function boundedInput(value: unknown, state: { truncated: boolean }): string {
  const parts: string[] = [];
  let budget = 32_768;
  let nodes = 1_000;
  const append = (text: string) => {
    if (budget <= 0) { if (text.length) state.truncated = true; return; }
    let piece = '';
    for (const character of text) {
      if (budget === 0) { state.truncated = true; break; }
      piece += character;
      budget--;
    }
    parts.push(piece);
  };
  const visit = (item: unknown, depth: number) => {
    if (item == null) return;
    if (--nodes < 0 || budget <= 0 || depth > 8) { state.truncated = true; return; }
    if (typeof item === 'string') { append(item); return; }
    if (typeof item === 'number' || typeof item === 'boolean') { append(String(item)); return; }
    if (Array.isArray(item)) {
      for (const child of item) {
        if (nodes <= 0 || budget <= 0) { state.truncated = true; break; }
        visit(child, depth + 1);
      }
      return;
    }
    if (object(item)) for (const key in item) {
      if (!Object.hasOwn(item, key)) continue;
      if (nodes <= 0 || budget <= 0) { state.truncated = true; break; }
      append(key);
      visit(item[key], depth + 1);
    }
  };
  visit(value, 0);
  return parts.join(' ');
}
function codexFallbackOutput(value: Record<string, unknown>): boolean {
  const raw = value.rawBackendItem;
  if (value.role !== 'toolResult' || object(value.source) || !object(raw) || typeof raw.type !== 'string') return false;
  if (typeof raw.aggregatedOutput === 'string') return false;
  if (raw.type === 'dynamicToolCall' && Array.isArray(raw.contentItems) && raw.contentItems.every(part => object(part) && part.type === 'inputText' && typeof part.text === 'string')) return false;
  if (raw.type === 'mcpToolCall' && (raw.error != null || raw.result != null)) return false;
  return true;
}
// Search visible fields only. Some Codex normalized tool outputs are fallback
// JSON of the whole native item, which must not enter the search index.
function messageText(value: unknown, state: { truncated: boolean }): string {
  if (!object(value)) return '';
  const presentation = desktopPresentation(value.presentation);
  if (value.role === 'custom' && !presentation) return '';
  const parts: string[] = [];
  if (presentation && typeof presentation.displayText === 'string') parts.push(presentation.displayText);
  else if (!codexFallbackOutput(value)) {
    const content = value.content;
    if (typeof content === 'string') parts.push(content);
    else if (Array.isArray(content)) for (const block of content) {
      if (!object(block)) continue;
      if ((block.type === 'text' || block.type === 'input_text' || block.type === 'output_text') && typeof block.text === 'string') parts.push(block.text);
      if (block.type === 'thinking' && typeof block.thinking === 'string') parts.push(block.thinking);
    }
  }
  if (value.role === 'toolResult') {
    if (typeof value.toolName === 'string') parts.push(value.toolName);
    const input = boundedInput(value.toolInput, state);
    if (input) parts.push(input);
    if (object(value.details) && typeof value.details.patch === 'string') parts.push(value.details.patch);
  }
  return parts.join('\n');
}
// Assistant tool calls carry inputs separately from the result rendered by the
// desktop. Attach those fields to that result's actual ID, not the assistant ID.
function searchableRows(rows: readonly Row[]): { rows: { id: string; text: string }[]; truncated: boolean; unresolved: boolean } {
  const state = { truncated: false };
  const calls = new Map<string, { name?: string; input?: unknown }>();
  for (const row of rows) {
    const message = row.message;
    if (!object(message) || message.role !== 'assistant' || !Array.isArray(message.content)) continue;
    for (const block of message.content) if (object(block) && block.type === 'toolCall' && typeof block.id === 'string') {
      calls.set(block.id, { ...(typeof block.name === 'string' ? {name:block.name} : {}), ...(block.arguments !== undefined ? {input:block.arguments} : {}) });
    }
  }
  const results = new Set<string>();
  const projected = rows.map(row => {
    const message = row.message;
    const id = desktopMessageId(row.id, message);
    let text = messageText(message, state);
    if (object(message) && message.role === 'toolResult') {
      results.add(id);
      const call = calls.get(id);
      const extra = [call?.name ?? '', message.toolInput == null ? boundedInput(call?.input, state) : ''].filter(Boolean).join('\n');
      if (extra) text = [text, extra].filter(Boolean).join('\n');
    }
    return {id, text};
  });
  let unresolved = false;
  for (const [id, call] of calls) {
    if (results.has(id)) continue;
    unresolved = true;
    const text = [call.name ?? '', boundedInput(call.input, state)].filter(Boolean).join('\n');
    if (text) projected.push({ id, text });
  }
  return { rows: projected, truncated: state.truncated, unresolved };
}
const maxSnippetLength = 220;

function splitsSurrogatePair(text: string, index: number): boolean {
  const high = text.charCodeAt(index - 1);
  const low = text.charCodeAt(index);
  return high >= 0xd800 && high <= 0xdbff && low >= 0xdc00 && low <= 0xdfff;
}

function snippetWindow(text: string, position: number, context: number) {
  let start = Math.min(position, text.length);
  if (splitsSurrogatePair(text, start)) start--;
  for (let i = 0; i < context && start > 0; i++) {
    start--;
    if (splitsSurrogatePair(text, start)) start--;
  }
  let end = start;
  for (let i = 0; i < maxSnippetLength && end < text.length; i++) {
    end++;
    if (splitsSurrogatePair(text, end)) end++;
  }
  return { excerpt: text.slice(start, end), hasBefore: start > 0, hasAfter: end < text.length };
}

function find(rows: readonly { id: string; text: string }[], first: string, rest: string[], firstMatchOnly = false): ConversationSearchMatch[] {
  const matches: ConversationSearchMatch[] = [];
  // Keep the hit near the left edge for narrow sidebar rows. Leave enough room
  // for a full-length query.
  const context = Math.max(0, Math.min(12, maxSnippetLength - [...first].length));
  for (const row of rows) {
    const normalized = row.text.normalize('NFC');
    const folded = normalized.toLocaleLowerCase();
    const position = folded.indexOf(first);
    if (position < 0 || !rest.every(term => folded.includes(term))) continue;
    const display = snippetWindow(normalized, position, context);
    // Locale-dependent lowercasing can change the length before the hit. If
    // the display window lost the hit, use the window in the matched string.
    const {excerpt, hasBefore, hasAfter} = display.excerpt.toLocaleLowerCase().includes(first)
      ? display : snippetWindow(folded, position, context);
    const slice = excerpt.replace(/\s+/gu, ' ').trim();
    matches.push({ messageId: row.id, snippet: `${hasBefore ? '…' : ''}${slice}${hasAfter ? '…' : ''}` });
    if (firstMatchOnly) break;
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
  const complete = data.completeHistory === true && data.searchVersion === 2 && Array.isArray(data.searchEntries);
  const entries: unknown[] = complete ? data.searchEntries as unknown[] : data.entries;
  const seenIds = new Set<string>();
  const rows = entries.map((entry: unknown) => {
    if (!object(entry) || typeof entry.turnId !== 'string' || !object(entry.item) || typeof entry.item.id !== 'string' || typeof entry.item.type !== 'string') throw new Error('Invalid Codex display journal entry');
    const item = entry.item as CodexItem;
    if (complete && (!item.id || !entry.turnId || seenIds.has(item.id) || item.status === 'inProgress' || item.type === 'hiveProjectionNotice')) throw new Error('Invalid complete Codex history snapshot');
    seenIds.add(item.id);
    const delivery = [...(metadata.inheritedDeliveries ?? []), ...metadata.deliveries].find(d => d.turnId === entry.turnId && d.itemId === item.id)
      ?? metadata.deliveries.find(d => d.turnId === entry.turnId && d.itemId === undefined && item.type === 'userMessage');
    return { id: item.id, message: normalizeCodexItem(item, delivery, metadata.inheritedSources?.find(source => source.itemId === item.id)?.sourceSessionId) };
  });
  return { rows, complete };
}

/** Reads fresh persisted history each call: appended or replaced files cannot leave a stale cache. */
export async function searchConversation(record: ChatRecord, query: string, options: ConversationSearchOptions): Promise<ConversationSearchResult> {
  const [first, ...rest] = foldSearchText(query.trim()).split(/\s+/u);
  if (!first) return { matches: [], complete: true };
  let rows: Row[] = [];
  let warning: string | undefined;
  let codexSynced = false;
  let truncated = false;
  let unresolved = false;
  try {
    if(record.backend==='acp'){
      if(record.sessionFile){const transcript=await AcpHistory.load(record.sessionFile);if(!transcript||transcript.state.sessionId!==record.acpSessionId)throw new Error('ACP transcript identity mismatch');rows=transcript.state.history.map((message,index)=>({id:desktopMessageId(transcript.state.historyIds[index]!,message),message}));warning='ACP local transcript may differ from native history; no agent was started for search';}
    } else if (record.backend === 'pi' || record.backend === 'pi-rpc') {
      if (record.sessionFile) {
        const file = record.sessionFile;
        const loaded = await freshRows([file], async () => ({ rows: await piRows(file, record.backend as 'pi' | 'pi-rpc'), complete: true }));
        rows = loaded.rows;
        truncated = loaded.truncated;
        unresolved = loaded.unresolved;
      }
      else if (record.piSessionId || record.externalPiSessionId) warning = 'Pi transcript path is unavailable';
    } else if (record.backend === 'claude') {
      if (record.nativeclaudeSessionId) {
        const sessionId = record.nativeclaudeSessionId;
        const metadata = readClaudeMetadata(options.claudeDirectory, sessionId);
        const projection = new ClaudeProjection(() => metadata);
        if (!options.readClaudeHistory) throw new Error('The saved Claude profile is unavailable for history search');
        const messages = await options.readClaudeHistory();
        for (const message of messages) projection.put(message);
        const snapshot = projection.snapshot();
        rows = snapshot.history.map((message, i) => ({ id: desktopMessageId(snapshot.historyMessageIds[i]!, message), message }));
        // The SDK returns [] for both missing and empty sessions and does not expose
        // transcript parse errors. Its result alone cannot prove completeness.
        warning = 'Claude history completeness cannot be verified from the SDK';
      }
    } else if (record.codexThreadId) {
      const metadata = codexMetadataFile(options.codexDirectory, record.codexThreadId);
      const loaded = await freshRows([metadata, `${metadata}.display.json`], () => codexRows(record, options.codexDirectory));
      rows = loaded.rows;
      codexSynced = loaded.complete;
      truncated = loaded.truncated;
      unresolved = loaded.unresolved;
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
      if (record.backend === 'pi' || record.backend === 'pi-rpc') { truncated = false; unresolved = false; }
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
  const projected = searchableRows(rows);
  if ((truncated || projected.truncated) && !warning) warning = 'Some tool inputs exceed the search limit';
  if ((unresolved || projected.unresolved || (options.liveBusy && record.backend !== 'codex')) && !warning) warning = 'Live or unfinished tool output may be missing from search';
  return { matches: find(projected.rows, first, rest, options.firstMatchOnly), complete: !warning, ...(warning ? { warning } : {}) };
}

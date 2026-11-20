import { createHash } from 'node:crypto';
import { MAX_HISTORY_PAGE_LIMIT, readHistoryJsonlLines, type HistoryWindowOptions } from '../history-cursor.js';
import { openIndexedHistory, savedHistoryFileVersion, type IndexedHistoryRow } from './saved-history-index.js';

export const importedCodexHistoryWarning = 'This preview shows text from the saved Codex transcript. Continue the conversation to load its native history.';

type RolloutRecord = Record<string, unknown>;
function object(value: unknown): RolloutRecord | undefined {
  return value && typeof value === 'object' && !Array.isArray(value) ? value as RolloutRecord : undefined;
}
export function importedCodexMessage(row: RolloutRecord) {
  const event = object(row.payload);
  if (row.type !== 'event_msg' || !event) return;
  const timestamp = typeof row.timestamp === 'string' ? Date.parse(row.timestamp) : NaN;
  const time = Number.isFinite(timestamp) ? {timestamp} : {};
  if ((event.type === 'user_message' || event.type === 'agent_message') && typeof event.message === 'string') {
    return {role:event.type === 'user_message' ? 'user' : 'assistant', content:event.message, ...time};
  }
  if (event.type !== 'item_completed') return;
  const item = object(event.item);
  if (!item || !Array.isArray(item.content) || (item.type !== 'UserMessage' && item.type !== 'AgentMessage')) return;
  const content = item.content.map(object).filter(part => part?.type === 'text' || part?.type === 'Text')
    .flatMap(part => typeof part?.text === 'string' ? [part.text] : []).join('\n');
  return {role:item.type === 'UserMessage' ? 'user' : 'assistant', content, ...time};
}

type Turn = {id?: string; explicit: boolean; rows: IndexedHistoryRow[]; nativeItems: Map<string, number>};
export async function openImportedCodexHistory(path: string, threadId: string,
    {cacheKey, signal, snapshot}: {cacheKey: string; signal?: AbortSignal; snapshot?: string}) {
  const version = await savedHistoryFileVersion(path);
  return openIndexedHistory({
    cacheKey:JSON.stringify(['imported-codex-text', cacheKey, threadId]), versions:[{file:path, version}],
    ...(signal && {signal}), ...(snapshot !== undefined && {snapshot}),
    async build(file) {
      const info = await file.stat();
      if (!info.isFile()) throw new Error('Invalid imported Codex transcript');
      const turns: Turn[] = [];
      const named = new Map<string, Turn>();
      let current: Turn | undefined;
      let header = false;
      const begin = (id?: string, explicit = false) => {
        const turn: Turn = {...(id && {id}), explicit, rows:[], nativeItems:new Map()};
        turns.push(turn);
        if (id) named.set(id, turn);
        return turn;
      };
      for await (const {start, raw} of readHistoryJsonlLines(file, info.size, signal ? {signal} : {})) {
        if (raw.length === 0) continue;
        let row: RolloutRecord | undefined;
        try { row = object(JSON.parse(raw.toString('utf8'))); }
        catch { continue; } // Match the native reader's tolerance of malformed rollout rows.
        if (!row) continue;
        const event = object(row.payload);
        if (!header) {
          if (row.type !== 'session_meta' || event?.id !== threadId) throw new Error('Imported Codex thread identity does not match');
          header = true;
          continue;
        }
        if (row.type !== 'event_msg' || !event) continue;
        const turnId = typeof event.turn_id === 'string' && event.turn_id ? event.turn_id : undefined;
        if (event.type === 'task_started' || event.type === 'turn_started') {
          current = (turnId && named.get(turnId)) || begin(turnId, true);
          current.explicit = true;
          continue;
        }
        if (event.type === 'task_complete' || event.type === 'turn_complete' || event.type === 'turn_aborted') {
          if (!turnId || current?.id === turnId) current = undefined;
          continue;
        }
        if (event.type === 'thread_rolled_back') {
          if (typeof event.num_turns !== 'number' || !Number.isSafeInteger(event.num_turns) || event.num_turns < 0)
            throw new Error('Cannot establish the imported Codex history after an invalid rollback marker');
          const remaining = Math.max(0, turns.length - event.num_turns);
          for (const removed of turns.splice(remaining)) {
            if (removed.id) named.delete(removed.id);
          }
          current = undefined;
          continue;
        }
        const message = importedCodexMessage(row);
        if (!message) continue;
        // Legacy rollouts use compatibility text events. Paginated rollouts
        // persist completed turn items instead (Codex rollout persistence
        // policy). Older legacy turns also lack an explicit start event.
        if (!turnId && message.role === 'user' && !current?.explicit) current = begin();
        const turn = turnId ? named.get(turnId) ?? begin(turnId) : current ?? (current = begin());
        if (!message.content.trim()) continue;
        const nativeId = event.type === 'item_completed' ? object(event.item)?.id : undefined;
        const id = typeof nativeId === 'string' && nativeId ? nativeId
          : `imported:${createHash('sha256').update(`${threadId}:${start}`).digest('hex')}`;
        const rowSpan = {start, end:start + raw.length, id};
        const index = turn.nativeItems.get(id);
        if (index !== undefined) turn.rows[index] = rowSpan;
        else {
          if (nativeId) turn.nativeItems.set(id, turn.rows.length);
          turn.rows.push(rowSpan);
        }
      }
      if (!header) throw new Error('Imported Codex thread has no saved identity');
      return {rows:turns.flatMap(turn => turn.rows), focus:new Map<string, number>(), toolFocus:new Map<string, number>()};
    },
    project(row) {
      const message = object(row);
      const result = message && importedCodexMessage(message);
      if (!result) throw new Error('Imported Codex message no longer matches its saved row');
      return result;
    },
  });
}

/** Preserve the full-history contract for callers that do not request pages. */
export async function readImportedCodexHistory(path: string, threadId: string, cacheKey: string, signal: AbortSignal) {
  const source = await openImportedCodexHistory(path, threadId, {cacheKey, signal});
  const pages: Awaited<ReturnType<typeof source.page>>[] = [];
  let beforeIndex: number | undefined;
  do {
    const options: HistoryWindowOptions = {limit:MAX_HISTORY_PAGE_LIMIT, snapshot:source.snapshot, ...(beforeIndex !== undefined && {beforeIndex})};
    const page = await source.page(options);
    pages.unshift(page);
    beforeIndex = page.historyPage.nextBeforeIndex;
  } while (beforeIndex !== undefined);
  return {history:pages.flatMap(page => page.history), historyMessageIds:pages.flatMap(page => page.historyMessageIds)};
}

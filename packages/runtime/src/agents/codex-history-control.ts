import { supportsCodexDeferredContinuation } from './codex-native-version.js';
import { resolve } from 'node:path';
import { CodexTransport } from './codex-transport.js';
import type { CodexItem, CodexTurn } from './codex-history.js';

export type CodexHistoryThread = { id: string; path: string; cwd: string; historyMode: 'paginated'; forkedFromId?: string | null; [key: string]: unknown };
export type CodexHistory = { thread: CodexHistoryThread; turns: CodexTurn[] };
export class CodexHistoryUncertainError extends Error {
  constructor(readonly operation: 'fork' | 'revert', cause: unknown) {
    super(`Codex ${operation} outcome is uncertain; do not retry automatically`, { cause });
    this.name = 'CodexHistoryUncertainError';
  }
}
export type CodexHistoryForkOptions = {
  threadId: string; cwd: string; lastTurnId?: string; beforeFirstTurnId?: string;
  onNativeRequested: () => void | Promise<void>;
  onNativeKnown: (threadId: string) => void | Promise<void>;
};

/** Read-only history and native branch creation. Never opens or executes a turn. */
export async function openCodexHistoryControl(options: { cwd: string; command?: string; signal?: AbortSignal }) {
  const transport = await CodexTransport.start({ cwd: options.cwd, ...(options.command && { command: options.command }), ...(options.signal && { signal: options.signal }) });
  if (!supportsCodexDeferredContinuation(transport.initializeResult)) {
    await transport.close();
    throw new Error('Unsupported Codex app-server version for history operations');
  }
  const read = async (threadId: string): Promise<CodexHistory> => {
    if (!threadId) throw new Error('Missing Codex thread ID');
    const response = await transport.sendRequest<{ thread: CodexHistoryThread }>('thread/read', { threadId, includeTurns: false });
    const thread = response?.thread;
    if (!thread || thread.id !== threadId || thread.historyMode !== 'paginated' || typeof thread.path !== 'string' || !thread.path || typeof thread.cwd !== 'string' || !thread.cwd) throw new Error('Unsupported or invalid Codex thread history');
    const turns: CodexTurn[] = [];
    const turnIds = new Set<string>(), itemIds = new Set<string>(), cursors = new Set<string>();
    let cursor: string | null = null;
    do {
      const page: { data: (CodexTurn & {itemsView?: string})[]; nextCursor?: string | null } = await transport.sendRequest('thread/turns/list', { threadId, cursor, sortDirection: 'asc', itemsView: 'full', limit: 1 });
      if (!page || !Array.isArray(page.data) || (page.nextCursor != null && typeof page.nextCursor !== 'string')) throw new Error('Invalid Codex history page');
      for (const turn of page.data) {
        if (!turn || typeof turn.id !== 'string' || !turn.id || turnIds.has(turn.id) || !['completed', 'interrupted', 'failed'].includes(turn.status) || turn.itemsView !== 'full' || !Array.isArray(turn.items)) throw new Error('Incomplete or duplicate Codex turn');
        turnIds.add(turn.id);
        for (const item of turn.items as CodexItem[]) {
          if (!item || typeof item.id !== 'string' || !item.id || typeof item.type !== 'string' || itemIds.has(item.id) || item.status === 'inProgress') throw new Error('Incomplete or duplicate Codex item');
          itemIds.add(item.id);
        }
        turns.push(turn);
      }
      cursor = page.nextCursor ?? null;
      if (cursor !== null) {
        if (!cursor || cursors.has(cursor)) throw new Error('Repeated Codex history cursor');
        cursors.add(cursor);
      }
    } while (cursor !== null);
    return { thread, turns };
  };
  return {
    read,
    async fork(input: CodexHistoryForkOptions): Promise<CodexHistory> {
      if (input.lastTurnId && input.beforeFirstTurnId) throw new Error('Conflicting Codex history boundaries');
      const source = await read(input.threadId);
      const index = input.lastTurnId ? source.turns.findIndex(turn => turn.id === input.lastTurnId) : source.turns.length - 1;
      if (input.lastTurnId && index < 0) throw new Error('Codex fork boundary not found');
      if (input.beforeFirstTurnId && source.turns[0]?.id !== input.beforeFirstTurnId) throw new Error('Codex first turn boundary changed');
      const expected = input.beforeFirstTurnId ? [] : source.turns.slice(0, index + 1);
      await input.onNativeRequested();
      let response: { thread: CodexHistoryThread };
      try {
        response = await transport.sendRequest('thread/fork', { threadId: input.threadId, cwd: input.cwd, excludeTurns: true, deferGoalContinuation: true, ...(input.lastTurnId && { lastTurnId: input.lastTurnId }) });
      } catch (error) { throw new CodexHistoryUncertainError('fork', error); }
      const childId = response?.thread?.id;
      if (typeof childId !== 'string' || !childId || childId === input.threadId) throw new CodexHistoryUncertainError('fork', new Error('Missing or invalid child identity'));
      await input.onNativeKnown(childId);
      if (input.beforeFirstTurnId) {
        try { await transport.sendRequest('thread/revert', { threadId: childId, beforeTurnId: input.beforeFirstTurnId }); }
        catch (error) { throw new CodexHistoryUncertainError('revert', error); }
      }
      const child = await read(childId);
      if (child.thread.forkedFromId !== input.threadId || resolve(child.thread.cwd) !== resolve(input.cwd) || child.thread.path === source.thread.path || JSON.stringify(child.turns) !== JSON.stringify(expected)) throw new Error('Codex child identity or history prefix mismatch');
      return child;
    },
    close: () => transport.close(),
  };
}

import { existsSync } from 'node:fs';
import type { ChatRecord } from './chat-index.js';
import { readJsonlTailWindow } from './history-cursor.js';

export interface BackendUsageBreakdown {
  backend: 'pi' | 'codex' | 'claude' | 'pi-rpc' | 'acp';
  totalSessions: number;
  activeLast7Days: number;
  worktreeSessions: number;
  archivedSessions: number;
  inputTokens: number;
  outputTokens: number;
  cacheReadTokens: number;
}

export interface LocalUsageSummary {
  checkedAt: number;
  totalSessions: number;
  activeLast7Days: number;
  activeLast30Days: number;
  worktreeSessions: number;
  archivedSessions: number;
  sampledTranscripts: number;
  totalInputTokens: number;
  totalOutputTokens: number;
  totalCacheReadTokens: number;
  backends: BackendUsageBreakdown[];
}

const BACKEND_ORDER: readonly BackendUsageBreakdown['backend'][] = [
  'pi',
  'codex',
  'claude',
  'pi-rpc',
  'acp',
];

const DAY_MS = 24 * 60 * 60 * 1000;
const MAX_SAMPLED_FILES = 60;

/**
 * Purely local, read-only multi-backend activity & token usage summary.
 * Never starts any agent process or issues any model/network request.
 */
export async function collectLocalUsageSummary(
  sessions: readonly ChatRecord[],
  now = Date.now(),
): Promise<LocalUsageSummary> {
  const cutoff7d = now - 7 * DAY_MS;
  const cutoff30d = now - 30 * DAY_MS;

  const map = new Map<BackendUsageBreakdown['backend'], BackendUsageBreakdown>();
  for (const backend of BACKEND_ORDER) {
    map.set(backend, {
      backend,
      totalSessions: 0,
      activeLast7Days: 0,
      worktreeSessions: 0,
      archivedSessions: 0,
      inputTokens: 0,
      outputTokens: 0,
      cacheReadTokens: 0,
    });
  }

  let activeLast7Days = 0;
  let activeLast30Days = 0;
  let worktreeSessions = 0;
  let archivedSessions = 0;

  for (const s of sessions) {
    const entry = map.get(s.backend);
    if (!entry) continue;
    entry.totalSessions++;
    if (s.updatedAt >= cutoff7d) {
      entry.activeLast7Days++;
      activeLast7Days++;
    }
    if (s.updatedAt >= cutoff30d) {
      activeLast30Days++;
    }
    if (s.execution?.kind === 'worktree') {
      entry.worktreeSessions++;
      worktreeSessions++;
    }
    if (s.archived) {
      entry.archivedSessions++;
      archivedSessions++;
    }
  }

  // Sample recent local Pi/Pi-RPC transcripts using bounded reverse chunks (max 64 KiB each)
  // to sum recorded token usage without loading huge transcripts into memory.
  const recentPi = [...sessions]
    .filter(s => (s.backend === 'pi' || s.backend === 'pi-rpc') && !!s.sessionFile)
    .sort((a, b) => b.updatedAt - a.updatedAt)
    .slice(0, MAX_SAMPLED_FILES);

  let sampledTranscripts = 0;
  let totalInputTokens = 0;
  let totalOutputTokens = 0;
  let totalCacheReadTokens = 0;

  for (const s of recentPi) {
    if (!s.sessionFile || !existsSync(s.sessionFile)) continue;
    try {
      const tail = await readJsonlTailWindow(s.sessionFile, { maxLines: 160, chunkBytes: 65_536 });
      sampledTranscripts++;
      const entry = map.get(s.backend)!;
      for (const line of tail.lines) {
        if (!line.includes('"usage"')) continue;
        let parsed: Record<string, unknown>;
        try {
          parsed = JSON.parse(line) as Record<string, unknown>;
        } catch {
          continue;
        }
        const msg = parsed.message as Record<string, unknown> | undefined;
        const usage = (msg?.usage ?? parsed.usage) as Record<string, unknown> | undefined;
        if (!usage || typeof usage !== 'object') continue;
        const input = Number(usage.input ?? usage.inputTokens ?? usage.input_tokens ?? 0);
        const output = Number(usage.output ?? usage.outputTokens ?? usage.output_tokens ?? 0);
        const cacheRead = Number(usage.cacheRead ?? usage.cacheReadTokens ?? usage.cache_read_input_tokens ?? 0);
        if (Number.isFinite(input) && input > 0) {
          entry.inputTokens += input;
          totalInputTokens += input;
        }
        if (Number.isFinite(output) && output > 0) {
          entry.outputTokens += output;
          totalOutputTokens += output;
        }
        if (Number.isFinite(cacheRead) && cacheRead > 0) {
          entry.cacheReadTokens += cacheRead;
          totalCacheReadTokens += cacheRead;
        }
      }
    } catch {
      // Skip unreadable files cleanly
    }
  }

  return {
    checkedAt: now,
    totalSessions: sessions.length,
    activeLast7Days,
    activeLast30Days,
    worktreeSessions,
    archivedSessions,
    sampledTranscripts,
    totalInputTokens,
    totalOutputTokens,
    totalCacheReadTokens,
    backends: BACKEND_ORDER.map(b => map.get(b)!),
  };
}

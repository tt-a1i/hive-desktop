import assert from 'node:assert/strict';

export type ProcessObserverAttempt = {
  attempt: number;
  elapsedMs: number;
  status: 'ready' | 'timeout' | 'error';
  error?: string;
  failure?: { code: unknown; killed: unknown; signal: unknown };
};

/** Warm the observer before launching owned processes; never retry cleanup observations. */
export async function prepareProcessObserver(
  read: () => Promise<readonly { ProcessId: number }[]>,
  ownPid: number,
  observed: (attempt: ProcessObserverAttempt) => void,
): Promise<void> {
  for (let attempt = 1; attempt <= 2; attempt++) {
    const started = Date.now();
    try {
      const rows = await read();
      assert.ok(rows.some(row => row.ProcessId === ownPid), 'CIM process observer must see the check process before CLI controls start');
    } catch (error) {
      const details = (error && typeof error === 'object' ? error : {}) as Record<string, unknown>;
      // Match the observed execFile deadline, not CIM errors, malformed output or maxBuffer failures.
      const timeout = details.code === null && details.killed === true && details.signal === 'SIGTERM';
      observed({ attempt, elapsedMs: Date.now() - started, status: timeout ? 'timeout' : 'error', error: String(error),
        failure: { code: details.code ?? null, killed: details.killed ?? null, signal: details.signal ?? null } });
      if (timeout && attempt === 1) continue;
      throw error;
    }
    observed({ attempt, elapsedMs: Date.now() - started, status: 'ready' });
    return;
  }
}

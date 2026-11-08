import type { CodexTransport } from './codex-transport.js';

/** RPC success only acknowledges enqueueing; changed settings are applied by a later native event. */
export async function updateCodexSettings(
  transport: CodexTransport, threadId: string, settings: { effort?: string; model?: string }, signal: AbortSignal,
) {
  let timer: ReturnType<typeof setTimeout> | undefined;
  let unsubscribe = () => {};
  let rejectApplied!: (error: unknown) => void;
  const aborted = () => rejectApplied(signal.reason);
  const applied = new Promise<{ model: string }>((resolve, reject) => {
    rejectApplied = reject;
    unsubscribe = transport.onNotification(event => {
      const params = event.params as { threadId?: string; threadSettings?: { model?: unknown; effort?: unknown } } | null;
      if (event.method !== 'thread/settings/updated' || params?.threadId !== threadId) return;
      if (!params.threadSettings || typeof params.threadSettings.model !== 'string') {
        reject(new Error('Codex returned invalid applied thread settings'));
      } else resolve({ model: params.threadSettings.model });
    });
    timer = setTimeout(() => reject(new Error('Codex settings application timed out')), 30_000);
    signal.addEventListener('abort', aborted, { once: true });
    if (signal.aborted) aborted();
  });
  // Application/transport failure can arrive before the RPC acknowledgement.
  void applied.catch(() => {});
  try {
    signal.throwIfAborted();
    await transport.sendRequest('thread/settings/update', { threadId, ...settings });
    return await applied;
  } finally {
    clearTimeout(timer);
    unsubscribe();
    signal.removeEventListener('abort', aborted);
  }
}

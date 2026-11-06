import { randomUUID } from 'node:crypto';
import { createAgentSession, SessionManager } from '@earendil-works/pi-coding-agent';
import type { PiSessionHandle, PiSessionOptions } from './types.js';

/** Uses Pi's normal resource loader, user settings, credentials and tool defaults. */
export async function createPiSession(options: PiSessionOptions): Promise<PiSessionHandle> {
  const manager = options.sessionFile
    ? SessionManager.open(options.sessionFile, options.sessionDir)
    : SessionManager.create(options.cwd, options.sessionDir);
  const { session } = await createAgentSession({
    cwd: options.cwd,
    ...(options.agentDir !== undefined && { agentDir: options.agentDir }),
    sessionManager: manager,
    ...(options.customTools !== undefined && { customTools: options.customTools }),
  });
  const sessionId = manager.getSessionId();
  let messageId: string | undefined;
  let activeMessage: { messageId: string; message: unknown } | undefined;
  let activeHistoryIndex: number | undefined;
  const messageIds = new WeakMap<object, string>();
  const idForMessage = (message: (typeof session.messages)[number]): string => {
    if (message.role === 'toolResult') return message.toolCallId;
    let id = messageIds.get(message);
    if (!id) {
      id = randomUUID();
      messageIds.set(message, id);
    }
    return id;
  };
  let active: { cancelled: boolean; done: Promise<void>; finalError: string | undefined; sdkAborted: boolean } | undefined;
  let disposed = false;
  const unsubscribe = session.subscribe((event) => {
    switch (event.type) {
      case 'agent_start':
        // Pi awaits agent_start before the first model request. Preflight itself is not cancellable.
        if (active?.cancelled) void session.abort();
        break;
      case 'message_start':
        if (event.message.role === 'assistant') {
          messageId = idForMessage(event.message);
          activeMessage = { messageId, message: event.message };
          activeHistoryIndex = session.messages.length;
        }
        break;
      case 'message_update': {
        const update = event.assistantMessageEvent;
        if (event.message.role === 'assistant') {
          messageId ??= idForMessage(event.message);
          messageIds.set(event.message, messageId);
          activeMessage = { messageId, message: event.message };
        }
        if (update.type === 'text_delta' || update.type === 'thinking_delta') {
          messageId ??= randomUUID();
          options.onEvent({ type: 'assistant_delta', sessionId, messageId,
            kind: update.type === 'text_delta' ? 'text' : 'thinking',
            contentIndex: update.contentIndex, delta: update.delta });
        }
        break;
      }
      case 'message_end':
        if (event.message.role === 'assistant') {
          const finalId = messageId ?? idForMessage(event.message);
          messageIds.set(event.message, finalId);
          activeMessage = undefined;
          activeHistoryIndex = undefined;
          options.onEvent({ type: 'message_end', sessionId, messageId: finalId, message: event.message });
          if (active) {
            if (event.message.stopReason === 'error') active.finalError = event.message.errorMessage ?? 'Pi request failed';
            else active.finalError = undefined;
          }
          messageId = undefined;
        }
        break;
      case 'tool_execution_start':
        options.onEvent({ type: 'tool_start', sessionId, toolCallId: event.toolCallId, toolName: event.toolName, args: event.args });
        break;
      case 'tool_execution_update':
        options.onEvent({ type: 'tool_update', sessionId, toolCallId: event.toolCallId, toolName: event.toolName, result: event.partialResult });
        break;
      case 'tool_execution_end':
        options.onEvent({ type: 'tool_end', sessionId, toolCallId: event.toolCallId, toolName: event.toolName, result: event.result, isError: event.isError });
        break;
      case 'agent_settled':
        if (active) active.sdkAborted ||= event.aborted;
        break;
    }
  });
  const setModel = async (provider: string, id: string) => {
    const model = session.modelRuntime.getModel(provider, id);
    if (!model) throw new Error(`Unknown Pi model: ${provider}/${id}`);
    await session.setModel(model, { persist: false });
  };
  try {
    if (options.model) await setModel(options.model.provider, options.model.id);
  } catch (error) {
    unsubscribe();
    session.dispose();
    throw error;
  }
  return {
    sessionId,
    get sessionFile() { return manager.getSessionFile(); },
    history: () => session.messages,
    snapshot() {
      const history = [...session.messages];
      const streaming = session.state.streamingMessage;
      if (!activeMessage && streaming?.role === 'assistant') {
        messageId = idForMessage(streaming);
        activeMessage = { messageId, message: streaming };
        activeHistoryIndex = history.length;
      }
      let current = activeMessage;
      // Pi appends message_end to state before dispatching it to subscribers.
      // Associate that exact newly appended assistant with its live ID during that interval.
      const ended = activeHistoryIndex === undefined ? undefined : history[activeHistoryIndex];
      if (current && ended?.role === 'assistant') {
        messageIds.set(ended, current.messageId);
        current = undefined;
      } else if (current && streaming?.role === 'assistant') {
        current = { messageId: current.messageId, message: streaming };
      }
      return { history, historyMessageIds: history.map(idForMessage),
        ...(current !== undefined && { activeMessage: current }) };
    },
    prompt(text) {
      if (disposed) return Promise.reject(new Error('Pi session is disposed'));
      if (active) return Promise.reject(new Error('Pi session is already processing a prompt'));
      const run = { cancelled: false, sdkAborted: false, done: Promise.resolve(), finalError: undefined as string | undefined };
      active = run;
      run.done = Promise.resolve().then(async () => {
        try {
          await session.prompt(text);
          if (run.finalError && !run.cancelled) options.onEvent({ type: 'error', sessionId, message: run.finalError });
        } catch (error) {
          if (!run.cancelled) options.onEvent({ type: 'error', sessionId, message: error instanceof Error ? error.message : String(error) });
          throw error;
        } finally {
          active = undefined;
          options.onEvent({ type: 'settled', sessionId, aborted: run.cancelled || run.sdkAborted });
        }
      });
      return run.done;
    },
    async abort() {
      const run = active;
      if (!run) return;
      run.cancelled = true;
      await session.abort();
      // Covers input/auth preflight, for which Pi has no cancellation signal.
      await run.done.catch(() => {});
    },
    dispose() {
      if (active) throw new Error('Abort and await the active Pi prompt before disposal');
      if (disposed) return;
      disposed = true;
      unsubscribe();
      session.dispose();
    },
    currentModel() {
      const model = session.model;
      return model && { provider: model.provider, id: model.id, name: model.name };
    },
    listModels() {
      const available = new Set(session.modelRuntime.getAvailableSnapshot().map(model => `${model.provider}\0${model.id}`));
      return session.modelRuntime.getModels().map(model => ({ provider: model.provider, id: model.id,
        name: model.name, available: available.has(`${model.provider}\0${model.id}`) }));
    },
    setModel,
  };
}

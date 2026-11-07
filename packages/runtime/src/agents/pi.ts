import { randomUUID } from 'node:crypto';
import { createAgentSession } from '@earendil-works/pi-coding-agent';
import { createDesktopSessionManager } from './pi-session-manager.js';
import { isDeliveryPresentation, type DeliveryPresentation } from '../delivery-presentation.js';
import type { PiSessionHandle, PiSessionOptions } from './types.js';

type DeliveryMetadata = DeliveryPresentation & { deliveryId: string };

function deliveryMetadata(message: unknown): DeliveryMetadata | undefined {
  if (typeof message !== 'object' || message === null || !('role' in message) || message.role !== 'custom'
    || !('customType' in message) || message.customType !== 'hive-delivery' || !('details' in message)) return;
  const details = message.details;
  if (!isDeliveryPresentation(details) || !('deliveryId' in details) || typeof details.deliveryId !== 'string'
    || details.deliveryId.length === 0) return;
  return { version: 1, origin: details.origin, displayText: details.displayText, deliveryId: details.deliveryId,
    ...(details.dispatchId !== undefined && { dispatchId: details.dispatchId }),
    ...(details.workerName !== undefined && { workerName: details.workerName }),
    ...(details.workerId !== undefined && { workerId: details.workerId }),
    ...(details.artifacts !== undefined && { artifacts: details.artifacts }) };
}

function presentedMessage<T>(message: T): T | (T & { presentation: DeliveryMetadata }) {
  const presentation = deliveryMetadata(message);
  return presentation ? { ...message, presentation } : message;
}

/** Uses Pi's normal resource loader, user settings, credentials and tool defaults. */
export async function createPiSession(options: PiSessionOptions): Promise<PiSessionHandle> {
  const manager = createDesktopSessionManager(options.cwd, options.sessionDir, options.sessionFile);
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
    const delivery = deliveryMetadata(message);
    if (delivery) return delivery.deliveryId;
    let id = messageIds.get(message);
    if (!id) {
      id = randomUUID();
      messageIds.set(message, id);
    }
    return id;
  };
  let active: { cancelled: boolean; done: Promise<void>; finalError: string | undefined; sdkAborted: boolean; deliveryId: string | undefined; deliveryReceived: boolean } | undefined;
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
        if (event.message.role === 'user' || (event.message.role === 'custom' && event.message.customType === 'hive-delivery')) {
          const delivery = deliveryMetadata(event.message);
          if (active && delivery && delivery.deliveryId === active.deliveryId) active.deliveryReceived = true;
          options.onEvent({ type: 'message_end', sessionId, messageId: idForMessage(event.message), message: presentedMessage(event.message) });
        }
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
    hasContext: customType => session.messages.some(message => message.role === 'custom' && message.customType === customType),
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
      return { history: history.map(presentedMessage), historyMessageIds: history.map(idForMessage),
        ...(current !== undefined && { activeMessage: current }) };
    },
    async addContext(text, customType = 'hive-context') {
      if (disposed) throw new Error('Pi session is disposed');
      if (active) throw new Error('Pi session is already processing a prompt');
      await session.sendCustomMessage({ customType, content: text, display: false }, { triggerTurn: false });
    },
    completedDeliveryIds() {
      return manager.getEntries().flatMap((entry) => {
        if (entry.type !== 'custom' || entry.customType !== 'hive-delivery-completed') return [];
        const data = entry.data;
        return typeof data === 'object' && data !== null && 'id' in data && typeof data.id === 'string' ? [data.id] : [];
      });
    },
    prompt(text, promptOptions) {
      if (disposed) return Promise.reject(new Error('Pi session is disposed'));
      if (active) return Promise.reject(new Error('Pi session is already processing a prompt'));
      const run = { cancelled: false, sdkAborted: false, done: Promise.resolve(), finalError: undefined as string | undefined, deliveryId: promptOptions?.deliveryId, deliveryReceived: false };
      active = run;
      run.done = Promise.resolve().then(async () => {
        try {
          if (promptOptions?.deliveryId) {
            const presentation: DeliveryPresentation = isDeliveryPresentation(promptOptions.presentation)
              ? promptOptions.presentation : { version: 1, origin: 'unknown', displayText: text };
            await session.sendCustomMessage({ customType: 'hive-delivery', content: text, display: false,
              details: { ...presentation, deliveryId: promptOptions.deliveryId } }, { triggerTurn: true });
          } else if (promptOptions?.automated) await session.sendUserMessage(text);
          else await session.prompt(text);
          if (run.finalError && !run.cancelled && !promptOptions?.deliveryId) options.onEvent({ type: 'error', sessionId, message: run.finalError });
          if (promptOptions?.deliveryId) {
            if (run.cancelled || run.sdkAborted) throw new Error('Pi delivery was aborted');
            if (run.finalError) throw new Error(run.finalError);
            if (!run.deliveryReceived) throw new Error('Pi delivery was not added to the conversation');
            manager.appendCustomEntry('hive-delivery-completed', { id: promptOptions.deliveryId });
          }
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

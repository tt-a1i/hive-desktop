import { PI_USER_INPUT, PI_USER_INPUT_RESULT, piUserInputSources, piUserInputNote, piUserPrompt, type PiUserInput } from './pi-user-input.js';
import { piUsage } from '../agent-usage.js';
import { promptImages, imageMessage } from '../prompt-images.js';
import { randomUUID } from 'node:crypto';
import { createAgentSession } from '@earendil-works/pi-coding-agent';
import { isUnsetPiModel } from '../chat-index.js';
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
    ...(details.attachments !== undefined && { attachments: structuredClone(details.attachments) }),
    ...(details.sourceText !== undefined && { sourceText: details.sourceText }),
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
    const delivery = userInputs.get(message) ?? deliveryMetadata(message);
    if (delivery) return delivery.deliveryId;
    let id = messageIds.get(message);
    if (!id) {
      id = randomUUID();
      messageIds.set(message, id);
    }
    return id;
  };
  const userInputs = new WeakMap<object, PiUserInput>();
  let active: { nativeInput: PiUserInput | undefined; handled: boolean; cancelled: boolean; done: Promise<void>; finalError: string | undefined; sdkAborted: boolean; deliveryId: string | undefined; deliveryReceived: boolean } | undefined;
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
          if (event.message.role === 'user' && active?.nativeInput && !active.deliveryReceived) {
            userInputs.set(event.message, active.nativeInput);
            active.deliveryReceived = true;
          }
          const delivery = userInputs.get(event.message) ?? deliveryMetadata(event.message);
          if (active && delivery && delivery.deliveryId === active.deliveryId) active.deliveryReceived = true;
          options.onEvent({ type: 'message_end', sessionId, messageId: idForMessage(event.message), message: userInputs.has(event.message) ? { ...event.message, presentation: userInputs.get(event.message) } : presentedMessage(event.message) });
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
  const unsubscribeErrors = session.extensionRunner.onError(error => {
    if (active) active.finalError = error.error;
    options.onEvent({ type: 'error', sessionId, message: error.error });
  });
  let savedModelUnavailable = false;
  const setModel = async (provider: string, id: string) => {
    const model = session.modelRuntime.getModel(provider, id);
    if (!model) throw new Error(`Unknown Pi model: ${provider}/${id}`);
    await session.setModel(model, { persist: false });
    savedModelUnavailable = false;
  };
  try {
    if (options.model) {
      const { provider, id } = options.model;
      // History stays readable when a provider is removed or signed out. Keep
      // the saved choice until the user selects a replacement.
      if (!session.modelRuntime.getModel(provider, id) || !await session.modelRuntime.checkAuth(provider)) {
        savedModelUnavailable = true;
      } else if (session.model?.provider !== provider || session.model?.id !== id) {
        await setModel(provider, id);
      }
    }
  } catch (error) {
    unsubscribe();
    unsubscribeErrors();
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
      const sources = piUserInputSources(manager.getBranch());
      let index = 0;
      const displayed: unknown[] = [];
      const displayIds: string[] = [];
      for (const projected of manager.buildSessionProjection().entries) {
        const note = sources.notes.get(projected.sourceEntry.id);
        if (note && !(active?.nativeInput?.deliveryId === note.deliveryId && active.deliveryReceived)) {
          displayed.push(piUserInputNote(projected.sourceEntry, note, 'pi'));
          displayIds.push(note.deliveryId);
        }
        for (const projectedMessage of projected.messages) {
          const original = history[index++] ?? projectedMessage;
          const input = sources.users.get(projected.sourceEntry.id);
          if (input && original.role === 'user') {
            userInputs.set(original, input);
            displayed.push({ ...original, presentation: input, rawBackendItem: projected.sourceEntry,
              source: { backend: 'pi', entryId: projected.sourceEntry.id } });
          } else displayed.push(presentedMessage(original));
          displayIds.push(idForMessage(original));
        }
      }
      for (const original of history.slice(index)) {
        const input = userInputs.get(original);
        displayed.push(input ? { ...original, presentation: input } : presentedMessage(original));
        displayIds.push(idForMessage(original));
      }
      return { history: displayed, historyMessageIds: displayIds,
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
      let images: ReturnType<typeof promptImages>;
      try { images = promptImages(promptOptions?.images); } catch (error) { return Promise.reject(error); }
      if (images.length && !session.model?.input.includes('image')) return Promise.reject(new Error('当前 Pi 模型不支持图片输入。'));
      if (disposed) return Promise.reject(new Error('Pi session is disposed'));
      if (savedModelUnavailable) return Promise.reject(new Error('原先的 Pi 模型不可用，请重新选择。'));
      if (active) return Promise.reject(new Error('Pi session is already processing a prompt'));
      const run = { nativeInput: undefined as PiUserInput | undefined, handled: false, cancelled: false, sdkAborted: false, done: Promise.resolve(), finalError: undefined as string | undefined, deliveryId: promptOptions?.deliveryId, deliveryReceived: false };
      active = run;
      run.done = Promise.resolve().then(async () => {
        try {
          if (promptOptions?.deliveryId) {
            const presentation: DeliveryPresentation = isDeliveryPresentation(promptOptions.presentation)
              ? promptOptions.presentation : { version: 1, origin: 'unknown', displayText: text };
            if (presentation.origin === 'user') {
              const input: PiUserInput = { ...presentation, deliveryId: promptOptions.deliveryId, rawText: text };
              const native = piUserPrompt(input);
              const entryId = manager.appendCustomEntry(PI_USER_INPUT, input);
              run.nativeInput = input;
              const entry = manager.getEntry(entryId)!;
              options.onEvent({ type: 'message_end', sessionId, messageId: input.deliveryId, message: piUserInputNote(entry, input, 'pi') });
              let disposition: string = 'failed';
              try {
                if (native.context.trim()) await session.sendCustomMessage({ customType: 'hive-user-input-context', content: native.context,
                  display: false, details: { deliveryId: input.deliveryId } }, { triggerTurn: false });
                await session.prompt(native.text, { images, source: 'interactive', preflightResult(value) {
                  disposition = value;
                  run.handled = value === 'handled';
                } });
              } finally { manager.appendCustomEntry(PI_USER_INPUT_RESULT, { deliveryId: input.deliveryId, disposition }); }
            } else {
              await session.sendCustomMessage({ customType: 'hive-delivery', content: imageMessage(text,images), display: false,
                details: { ...presentation, deliveryId: promptOptions.deliveryId } }, { triggerTurn: true });
            }
          } else if (promptOptions?.automated) await session.sendUserMessage(imageMessage(text,images));
          else await session.prompt(text, {images});
          if (run.finalError && !run.cancelled && !promptOptions?.deliveryId) options.onEvent({ type: 'error', sessionId, message: run.finalError });
          if (promptOptions?.deliveryId) {
            if (run.cancelled || run.sdkAborted) throw new Error('Pi delivery was aborted');
            if (run.finalError) throw new Error(run.finalError);
            if (!run.deliveryReceived && !run.handled) throw new Error('Pi delivery was not added to the conversation');
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
      unsubscribeErrors();
      session.dispose();
    },
    currentModel() {
      const model = session.model;
      return model && !savedModelUnavailable && !isUnsetPiModel(model) ? { provider: model.provider, id: model.id, name: model.name } : undefined;
    },
    async refreshModels() {
      await session.modelRuntime.refresh({ allowNetwork: false });
      const selected = session.model;
      const current = selected && session.modelRuntime.getModel(selected.provider, selected.id);
      const available = current && session.modelRuntime.getAvailableSnapshot().some(model => model.provider === current.provider && model.id === current.id);
      savedModelUnavailable = !!selected && !isUnsetPiModel(selected) && !available;
      const configError = session.modelRuntime.getError();
      if (configError) throw new Error(configError);
      if (current && available) await session.setModel(current, { persist: false });
    },
    supportsImages: () => session.model?.input.includes('image') ?? false,
    listModels() {
      const available = new Set(session.modelRuntime.getAvailableSnapshot().map(model => `${model.provider}\0${model.id}`));
      return session.modelRuntime.getModels().map(model => ({ provider: model.provider, id: model.id,
        name: model.name, available: available.has(`${model.provider}\0${model.id}`) }));
    },
    usage: () => piUsage(session.getSessionStats(),"内置 Pi 原生会话"),
    reasoning: () => ({ level: session.thinkingLevel, levels: !savedModelUnavailable && session.model ? session.getAvailableThinkingLevels() : [] }),
    setReasoning(level) {
      if (disposed) throw new Error('Pi session is disposed');
      if (active) throw new Error('Pi session is already processing a prompt');
      const levels = !savedModelUnavailable && session.model ? session.getAvailableThinkingLevels() : [];
      const selected = levels.find(candidate => candidate === level);
      if (!selected) throw new TypeError('当前模型不支持此推理强度。');
      session.setThinkingLevel(selected, { persist: false });
      return { level: session.thinkingLevel, levels: session.getAvailableThinkingLevels() };
    },
    setModel,
  };
}

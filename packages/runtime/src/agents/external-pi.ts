import { piUserInputSources, piUserInputNote, piUserPrompt, type PiUserInput } from './pi-user-input.js';
import { piUsage } from '../agent-usage.js';
import { promptImages } from '../prompt-images.js';
import { randomUUID } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import type { SessionEntry } from '@earendil-works/pi-coding-agent';
import { isDeliveryPresentation, type DeliveryPresentation } from '../delivery-presentation.js';
import { assertExternalPiCompletion } from './external-pi-completion.js';
import { applyExternalPiUpdate, externalPiBranch, externalPiMessage } from './external-pi-history.js';
import { ExternalPiInteractions } from './external-pi-interactions.js';
import { ExternalPiTransport, type ExternalPiRecord } from './external-pi-transport.js';
import type { PiEvent, PiSessionHandle } from './types.js';

export interface ExternalPiTeamBinding { workspaceId: string; agentId: string; token: string; hivePort: string; cwd: string }
export interface ExternalPiSessionOptions {
  cwd: string;
  sessionDir: string;
  sessionFile?: string;
  model?: { provider: string; id: string };
  command?: string;
  onEvent: (event: PiEvent) => void;
  onUnexpectedExit?: (error: Error) => void;
  teamBinding?: () => ExternalPiTeamBinding | undefined;
}
export interface ExternalPiSessionHandle extends PiSessionHandle { readonly teamToolName: string; dispose(): Promise<void> }
type Model = { provider: string; id: string; name: string; input?: string[] };
type State = { sessionId: string; sessionFile: string; model?: Model; isStreaming: boolean };
type Run = { cancelled: boolean; aborted: boolean; started: boolean; error?: Error; lastAssistant?: Record<string, unknown>;
  terminal: Promise<void>; resolve(): void; reject(error: Error): void; done: Promise<void> };

/** External Pi owns model routing, credentials, configuration and the durable transcript. */
export async function createExternalPiSession(options: ExternalPiSessionOptions): Promise<ExternalPiSessionHandle> {
  let state: State | undefined;
  let branch: SessionEntry[] = [];
  let history: Record<string, unknown>[] = [];
  let ids: string[] = [];
  const liveIds = new Map<string, string>();
  let models: Model[] = [];
  let activeMessage: { messageId: string; message: unknown } | undefined;
  let active: Run | undefined;
  let initialized = false;
  let disposed = false;
  let failure: Error | undefined;
  let eventWork = Promise.resolve();
  let transport: ExternalPiTransport;
  const emit = (event: PiEvent) => options.onEvent(event);
  const interactions = new ExternalPiInteractions(() => state!.sessionId, emit, id => transport.expireUiRequest(id));
  const refresh = async () => {
    const result = await transport.request<{ entries: SessionEntry[]; leafId: string | null }>('get_entries');
    branch = externalPiBranch(result.entries, result.leafId);
    const sources = piUserInputSources(branch);
    const presented = branch.flatMap(entry => {
      const note = sources.notes.get(entry.id);
      const message: Record<string, unknown> | undefined = note ? piUserInputNote(entry, note, 'pi-rpc') : externalPiMessage(entry, sources.users.get(entry.id));
      return message ? [{ entry, message }] : [];
    });
    history = presented.map(item => item.message); ids = presented.map(item => item.message.role === 'toolResult' && typeof item.message.toolCallId === 'string'
      ? item.message.toolCallId : liveIds.get(item.entry.id) ?? item.entry.id);
  };
  const ensureOpen = () => { if (disposed || failure) throw failure ?? new Error('External Pi session is disposed'); };
  const onEvent = (event: ExternalPiRecord) => {
    if (!state) return;
    const sessionId = state.sessionId;
    const run = active;
    if (event.type === 'agent_start' && run) {
      run.started = true;
      if (run.cancelled) void transport.request('abort').catch(error => run.reject(asError(error)));
    } else if (event.type === 'message_start' && isObject(event.message) && event.message.role === 'assistant') {
      activeMessage = { messageId: randomUUID(), message: event.message };
    } else if (event.type === 'message_update' && isObject(event.assistantMessageEvent)) {
      activeMessage ??= { messageId: randomUUID(), message: { role: 'assistant', content: [] } };
      activeMessage.message = applyExternalPiUpdate(activeMessage.message as Record<string, unknown>, event);
      const update = event.assistantMessageEvent;
      if ((update.type === 'text_delta' || update.type === 'thinking_delta') && typeof update.delta === 'string' && typeof update.contentIndex === 'number') {
        emit({ type: 'assistant_delta', sessionId, messageId: activeMessage.messageId, kind: update.type === 'text_delta' ? 'text' : 'thinking', contentIndex: update.contentIndex, delta: update.delta });
      }
    } else if (event.type === 'message_end' && isObject(event.message)) {
      const message = event.message;
      const liveId = message.role === 'assistant' ? activeMessage?.messageId : undefined;
      if (message.role === 'assistant') activeMessage = undefined;
      if (run && message.role === 'assistant') { run.lastAssistant = message; }
      eventWork = eventWork.then(async () => {
        await refresh();
        const index = history.findLastIndex(candidate => sameMessage(candidate, message));
        if (index !== -1 && liveId) {
          const source = history[index]!.source as { entryId: string };
          liveIds.set(source.entryId, liveId); ids[index] = liveId;
        }
        if (activeMessage?.messageId === liveId) activeMessage = undefined;
        if (index !== -1) emit({ type: 'message_end', sessionId, messageId: ids[index]!, message: history[index] });
      }).catch(error => { if (run) { run.error = asError(error); run.reject(run.error); } });
    } else if (event.type === 'tool_execution_start') emit({ type: 'tool_start', sessionId, toolCallId: String(event.toolCallId), toolName: String(event.toolName), args: event.args });
    else if (event.type === 'tool_execution_update') emit({ type: 'tool_update', sessionId, toolCallId: String(event.toolCallId), toolName: String(event.toolName), result: event.partialResult });
    else if (event.type === 'tool_execution_end') emit({ type: 'tool_end', sessionId, toolCallId: String(event.toolCallId), toolName: String(event.toolName), result: event.result, isError: event.isError === true });
    else if (event.type === 'agent_settled' && run) { run.aborted = event.aborted === true; run.resolve(); }
    else if (event.type === 'extension_error' || event.type === 'hive_extension_ui_unsupported') {
      const error = new Error(String(event.error ?? event.message ?? 'External Pi extension failed'));
      emit({ type: 'error', sessionId, message: error.message });
      if (run) { run.error = error; if (!run.started) run.reject(error); }
    } else if (event.type === 'extension_ui_request' && event.method === 'notify') {
      if (event.notifyType === 'error') emit({ type: 'error', sessionId, message: String(event.message) });
    }
  };
  transport = await ExternalPiTransport.start({ cwd: options.cwd, sessionDir: options.sessionDir,
    ...(options.sessionFile && { sessionFile: options.sessionFile }), ...(options.command && { command: options.command }), ...(options.model && { model: options.model }),
    extensionPath: fileURLToPath(new URL(import.meta.url.endsWith('.ts') ? './external-pi-extension.ts' : './external-pi-extension.js', import.meta.url)), onEvent,
    onDialogRequest(request, write) {
      if (!initialized) throw new Error('External Pi dialog arrived before the chat session was initialized');
      interactions.request(request, write);
    },
    onClose(error) {
      if (disposed) return;
      interactions.clear('disconnected');
      failure = error; active?.reject(error);
      if (initialized) options.onUnexpectedExit?.(error);
    },
  });
  const setModel = async (provider: string, id: string) => {
    ensureOpen();
    if (active) throw new Error('External Pi session is already processing a prompt');
    const model = await transport.request<Model>('set_model', { provider, modelId: id });
    if (state) state.model = {provider:model.provider,id:model.id,name:model.name,...(model.input && {input:[...model.input]})};
  };
  try {
    state = await transport.request<State>('get_state');
    if (options.model && (state.model?.provider !== options.model.provider || state.model?.id !== options.model.id)) await setModel(options.model.provider, options.model.id);
    models = (await transport.request<{ models: Model[] }>('get_available_models')).models;
    await refresh(); initialized = true;
  } catch (error) { disposed = true; await transport.close(); throw error; }
  const reasoning = async () => {
    ensureOpen();
    const current = await transport.request<State & { thinkingLevel: string }>('get_state');
    const { levels } = await transport.request<{ levels: string[] }>('get_available_thinking_levels');
    if (typeof current.thinkingLevel !== 'string' || !Array.isArray(levels) || !levels.every(level => typeof level === 'string')) throw new Error('外部 Pi 未返回有效的推理强度信息。');
    return { level: current.thinkingLevel, levels: current.model ? levels : [] };
  };
  return {
    sessionId: state.sessionId,
    teamToolName: transport.teamToolName,
    get sessionFile() { return state!.sessionFile; },
    history: () => history,
    interactions: () => interactions.snapshot(),
    respondInteraction: (id, response) => { ensureOpen(); return interactions.respond(id, response); },
    hasContext: customType => history.some(message => message.role === 'custom' && message.customType === customType),
    snapshot: () => ({ history: [...history], historyMessageIds: [...ids], ...(activeMessage && { activeMessage: { ...activeMessage } }) }),
    completedDeliveryIds: () => branch.flatMap(entry => entry.type === 'custom' && entry.customType === 'hive-delivery-completed'
      && isObject(entry.data) && typeof entry.data.id === 'string' ? [entry.data.id] : []),
    async addContext(text, customType = 'hive-context') {
      ensureOpen(); if (active) throw new Error('External Pi session is already processing a prompt');
      await transport.bridge({ action: 'context', text, customType });
      await eventWork; await refresh();
    },
    prompt(text, promptOptions) {
      let images: ReturnType<typeof promptImages>;
      try { images = promptImages(promptOptions?.images); } catch (error) { return Promise.reject(error); }
      try { ensureOpen(); } catch (error) { return Promise.reject(error); }
      if (images.length && !state?.model?.input?.includes('image')) return Promise.reject(new Error('当前外部 Pi 模型未提供图片输入能力。'));
      if (active) return Promise.reject(new Error('External Pi session is already processing a prompt'));
      let resolve!: () => void; let reject!: (error: Error) => void;
      const terminal = new Promise<void>((yes, no) => { resolve = yes; reject = no; });
      void terminal.catch(() => {});
      const run: Run = { cancelled: false, aborted: false, started: false, terminal, resolve, reject, done: Promise.resolve() };
      active = run;
      run.done = Promise.resolve().then(async () => {
        try {
          await transport.bridge({ action: 'bind', binding: options.teamBinding?.() ?? null });
          if (run.cancelled) return;
          const before = new Set(branch.map(entry => entry.id));
          let handledUser = false;
          if (promptOptions?.deliveryId) {
            const presentation: DeliveryPresentation = isDeliveryPresentation(promptOptions.presentation) ? promptOptions.presentation : { version: 1, origin: 'unknown', displayText: text };
            if (presentation.origin === 'user') {
              const input: PiUserInput = { ...presentation, deliveryId: promptOptions.deliveryId, rawText: text };
              const native = piUserPrompt(input);
              await transport.bridge({ action: 'begin-user-input', input });
              await refresh();
              const note = history.find(message => isObject(message.presentation) && message.presentation.deliveryId === input.deliveryId);
              if (note) emit({ type: 'message_end', sessionId: state!.sessionId, messageId: input.deliveryId, message: note });
              let disposition = 'failed';
              try {
                if (run.cancelled) throw new Error('External Pi delivery was aborted');
                if (run.error) throw run.error;
                const result = await transport.request<{disposition:string}>('prompt', { message: native.text, ...(images.length && {images}) });
                disposition = result.disposition;
                handledUser = disposition === 'handled' && !run.started;
                if (!handledUser) await terminal;
              } finally { await transport.bridge({ action: 'end-user-input', deliveryId: input.deliveryId, disposition }); }
            } else {
              await transport.bridge({ action: 'delivery', text, ...(images.length && {images}), details: { ...presentation, deliveryId: promptOptions.deliveryId } });
              await terminal;
            }
          } else if (promptOptions?.automated) {
            await transport.bridge({ action: 'user', text, ...(images.length && {images}) }); await terminal;
          } else {
            const disposition = await transport.request<{ disposition: string }>('prompt', { message: text, ...(images.length && {images}) });
            if (disposition.disposition !== 'handled' || run.started) await terminal;
          }
          await eventWork; await refresh();
          if (run.cancelled || run.aborted) { if (promptOptions?.deliveryId) throw new Error('External Pi delivery was aborted'); return; }
          if (run.error) throw run.error;
          if (run.lastAssistant?.stopReason === 'error') throw new Error(String(run.lastAssistant.errorMessage ?? 'External Pi request failed'));
          if (promptOptions?.deliveryId && handledUser) {
            await transport.bridge({ action: 'complete-user-input', deliveryId: promptOptions.deliveryId });
            await refresh();
          } else if (promptOptions?.deliveryId) {
            const terminalEntry = branch.findLast(entry => !before.has(entry.id) && entry.type === 'message' && entry.message.role === 'assistant');
            if (!terminalEntry || terminalEntry.type !== 'message' || !run.lastAssistant || !sameMessage(terminalEntry.message as unknown as Record<string, unknown>, run.lastAssistant)) throw new Error('External Pi delivery terminal assistant is missing');
            assertExternalPiCompletion(branch, promptOptions.deliveryId, terminalEntry.id);
            await transport.bridge({ action: 'complete', deliveryId: promptOptions.deliveryId, assistantEntryId: terminalEntry.id });
            await refresh();
          }
        } catch (error) {
          if (!run.cancelled) emit({ type: 'error', sessionId: state!.sessionId, message: asError(error).message });
          throw error;
        } finally {
          interactions.clear('turn-ended'); transport.clearUiRequests();
          active = undefined; activeMessage = undefined;
          emit({ type: 'settled', sessionId: state!.sessionId, aborted: run.cancelled || run.aborted });
        }
      });
      return run.done;
    },
    async abort() {
      const run = active; if (!run) { await interactions.cancel('aborted'); transport.clearUiRequests(); return; }
      run.cancelled = true;
      try { await interactions.cancel('aborted'); transport.clearUiRequests(); await transport.request('clear_queue'); await transport.request('abort'); }
      catch (error) { if (!failure) throw error; }
      await run.done.catch(() => {});
    },
    async dispose() {
      if (active) throw new Error('Abort and await the active external Pi prompt before disposal');
      if (disposed) return;
      disposed = true; interactions.clear('disposed'); await transport.close();
    },
    supportsImages:()=>state?.model?.input?.includes('image')??false,
    currentModel: () => state?.model && ({ provider: state.model.provider, id: state.model.id, name: state.model.name }),
    listModels: () => models.map(model => ({ provider: model.provider, id: model.id, name: model.name, available: true })),
    async usage(){ensureOpen();return piUsage(await transport.request("get_session_stats"),"外部 Pi 原生会话");},
    reasoning,
    async setReasoning(level) {
      ensureOpen();
      if (active) throw new Error('External Pi session is already processing a prompt');
      const previous = await reasoning();
      if (!previous.levels.includes(level)) throw new TypeError('当前模型不支持此推理强度。');
      await transport.request('set_thinking_level', { level });
      return reasoning();
    },
    setModel,
  };
}
function isObject(value: unknown): value is Record<string, unknown> { return !!value && typeof value === 'object' && !Array.isArray(value); }
function sameMessage(a: Record<string, unknown>, b: Record<string, unknown>): boolean {
  return a.role === b.role && (a.role === 'custom' ? a.customType === b.customType && JSON.stringify(a.details) === JSON.stringify(b.details) : a.timestamp === b.timestamp)
    && JSON.stringify(a.content) === JSON.stringify(b.content);
}
function asError(value: unknown): Error { return value instanceof Error ? value : new Error(String(value)); }

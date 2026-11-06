import type { TSchema } from 'typebox';
import { Value } from 'typebox/value';
import { isDeliveryPresentation, type DeliveryPresentation } from '../delivery-presentation.js';
import { CodexTransport, type CodexServerRequest } from './codex-transport.js';
import { mkdirSync } from 'node:fs';
import { CodexProjection } from './codex-projection.js';
import { CodexEvents } from './codex-events.js';
import { readCodexMetadata, writeCodexMetadata } from './codex-metadata.js';
import type { CodexDelivery, CodexItem, CodexMetadata, CodexThreadResponse, CodexTurn } from './codex-history.js';
import type { PiEvent, PiSessionHandle } from './types.js';

export interface CodexTool {
  name: string;
  description: string;
  parameters: TSchema;
  execute(id: string, args: unknown, signal?: AbortSignal): Promise<{ content: ({ type: 'text'; text: string } | { type: 'image'; mimeType: string; data: string })[] }>;
}
export interface CodexSessionOptions {
  cwd: string;
  sessionDir: string;
  threadId?: string;
  command?: string;
  model?: { provider: string; id: string };
  customTools?: CodexTool[];
  onEvent: (event: PiEvent) => void;
  /** Persist the durable backend reference before delivery completion is acknowledged. */
  onMaterialized?: (threadId: string) => void | Promise<void>;
  /** Lifecycle notification only; callback must schedule cleanup rather than await it here. */
  onUnexpectedExit?: (error: Error) => void;
  /** Explicit UI/policy response; absence rejects approvals and user-input requests. */
  onServerRequest?: (request: CodexServerRequest) => Promise<unknown>;
}
export interface CodexSessionHandle extends PiSessionHandle {
  readonly threadId: string;
  readonly materialized: boolean;
  hasContext(customType: string): boolean;
  dispose(): Promise<void>;
}
type Model = { id: string; model: string; displayName: string; hidden: boolean };
type Run = { tools: Set<Promise<unknown>>; terminalComplete: boolean; turnId?: string; cancelled: boolean; controller: AbortController; delivery?: { id: string; presentation: DeliveryPresentation };
  terminal: Promise<CodexTurn>; resolve: (turn: CodexTurn) => void; reject: (error: Error) => void; done: Promise<void>; error?: Error };

/** Codex 0.166 app-server public JSON-RPC; no terminal scraping or Pi model intermediary. */
export async function createCodexSession(options: CodexSessionOptions): Promise<CodexSessionHandle> {
  let active: Run | undefined;
  let disposed = false;
  let initialized = false;
  let materialized = Boolean(options.threadId);
  let materializationNotified = materialized;
  let threadId = options.threadId ?? '';
  let metadata: CodexMetadata = options.threadId ? readCodexMetadata(options.sessionDir, options.threadId)
    : { version: 1, context: [], deliveries: [], completed: [] };
  let response: CodexThreadResponse;
  let events: CodexEvents | undefined;
  let projection: CodexProjection | undefined;
  const emit = (event: PiEvent) => options.onEvent(event);
  const persist = (next: CodexMetadata) => {
    writeCodexMetadata(options.sessionDir, threadId, next);
    metadata = next;
  };
  const rememberDelivery = (turnId: string) => {
    const delivery = active?.delivery;
    if (delivery && !metadata.deliveries.some(entry => entry.turnId === turnId)) {
      persist({ ...metadata, deliveries: [...metadata.deliveries, { turnId, deliveryId: delivery.id, presentation: delivery.presentation }] });
    }
  };
  const transport = await CodexTransport.start({ cwd: options.cwd,
    ...(options.command && { command: options.command }),
    onClose(info) {
      if (active) { active.controller.abort(); active.terminalComplete = true; active.reject(info.error); }
      if (initialized && !disposed) options.onUnexpectedExit?.(info.error);
    },
    async onRequest(request) {
      const params = request.params as Record<string, unknown>;
      if (request.method === 'item/tool/call' && params.threadId === threadId) {
        const tool = options.customTools?.find(candidate => candidate.name === params.tool);
        if (!tool) throw new Error(`Unknown Hive Codex tool: ${String(params.tool)}`);
        try {
          if (!Value.Check(tool.parameters, params.arguments)) throw new Error(`Invalid arguments for ${tool.name}`);
          const run = active;
          if (!run) throw new Error('Codex requested a Hive tool outside an active turn');
          const execution = tool.execute(String(params.callId), params.arguments, run.controller.signal);
          run.tools.add(execution);
          let result: Awaited<ReturnType<CodexTool['execute']>>;
          try { result = await execution; } finally { run.tools.delete(execution); }
          return { success: true, contentItems: result.content.map(part => {
            if (part.type === 'text') return { type: 'inputText', text: part.text };
            if (part.type === 'image') return { type: 'inputImage', imageUrl: `data:${part.mimeType};base64,${part.data}` };
            throw new Error('Unsupported Codex dynamic tool output');
          }) };
        } catch (error) {
          return { success: false, contentItems: [{ type: 'inputText', text: asError(error).message }] };
        }
      }
      if (options.onServerRequest) return options.onServerRequest(request);
      const error = new Error(`Codex requires unsupported interaction: ${request.method}`);
      emit({ type: 'error', sessionId: threadId, message: error.message });
      if (active) {
        active.error = error;
        active.controller.abort();
        if (active.turnId) void transport.sendRequest('turn/interrupt', { threadId, turnId: active.turnId }).catch(active.reject);
      }
      throw error;
    },
    onNotification(notification) {
      const params = notification.params as Record<string, unknown>;
      if (!params || params.threadId !== threadId) return;
      try {
        if (notification.method === 'turn/started' && active) active.turnId = (params.turn as CodexTurn).id;
        if (notification.method === 'item/started' || notification.method === 'item/completed') {
          const turnId = String(params.turnId);
          rememberDelivery(turnId);
          if (notification.method === 'item/completed') projection?.put(params.item as CodexItem, turnId);
          events?.item(params.item as CodexItem, turnId, notification.method === 'item/completed');
        } else if (notification.method === 'turn/completed' && active) {
          const turn = params.turn as CodexTurn;
          if (!active.turnId || active.turnId === turn.id) {
            active.terminalComplete = true;
            if (turn.status === 'interrupted') {
              for (const item of events?.interruptedItems() ?? []) {
                projection?.put(item, turn.id);
                events?.item(item, turn.id, true);
              }
            }
            projection?.complete(turn.id);
            active.resolve(turn);
          }
        } else events?.delta(notification.method, params);
      } catch (error) {
        if (active) {
          active.error = asError(error); active.controller.abort();
          if (active.terminalComplete) active.reject(active.error);
          else if (active.turnId) void transport.sendRequest('turn/interrupt', { threadId, turnId: active.turnId }).catch(active.reject);
        }
      }
    },
  });
  const resume = async (extra: Record<string, unknown> = {}) => {
    response = await transport.sendRequest<CodexThreadResponse>('thread/resume', { threadId, ...extra });
  };
  const hydrate = async () => {
    const read = await transport.sendRequest<CodexThreadResponse>('thread/read', { threadId, includeTurns: true });
    if (read.thread.historyMode !== 'legacy') throw new Error('Codex paginated history is unsupported by this installed app-server');
    projection!.recover(read.thread.turns);
    events!.restore(projection!.current());
  };
  const models: Model[] = [];
  try {
    let cursor: string | null = null;
    do {
      const page: { data: Model[]; nextCursor: string | null } = await transport.sendRequest('model/list', { cursor, includeHidden: true });
      models.push(...page.data); cursor = page.nextCursor;
    } while (cursor);
    const modelParams = options.model ? { model: options.model.id, modelProvider: options.model.provider } : {};
    if (options.threadId) await resume(modelParams);
    else response = await transport.sendRequest<CodexThreadResponse>('thread/start', { cwd: options.cwd, historyMode: 'legacy', allowProviderModelFallback: false, ...modelParams,
      dynamicTools: options.customTools?.map(tool => ({ type: 'function', name: tool.name, description: tool.description, inputSchema: tool.parameters })) ?? [] });
    if (options.model && (response!.model !== options.model.id || response!.modelProvider !== options.model.provider)) throw new Error('Codex did not select the requested model/provider');
    threadId = response!.thread.id;
    persist(metadata);
    mkdirSync(options.sessionDir, { recursive: true, mode: 0o700 });
    projection = new CodexProjection(options.sessionDir, threadId);
    events = new CodexEvents(threadId, emit, id => metadata.deliveries.find(entry => entry.turnId === id));
    if (response!.thread.historyMode !== 'legacy') throw new Error('Codex paginated history is unsupported by this installed app-server');
    projection.recover(response!.thread.turns);
    events.restore(projection.current());
  } catch (error) { await transport.close(); throw error; }
  const notifyMaterialized = async () => {
    if (materialized && !materializationNotified) {
      await options.onMaterialized?.(threadId);
      materializationNotified = true;
    }
  };
  const ensureIdle = () => {
    if (disposed) throw new Error('Codex session is disposed');
    if (active) throw new Error('Codex session is already processing a prompt');
  };
  initialized = true;
  return {
    sessionId: threadId, threadId,
    get materialized() { return materialized; },
    get sessionFile() { return response.thread.path ?? undefined; },
    history: () => events!.snapshot().history,
    snapshot: () => events!.snapshot(),
    completedDeliveryIds: () => [...metadata.completed],
    hasContext: customType => metadata.context.some(entry => entry.customType === customType),
    async addContext(text, customType = 'hive-context') {
      ensureIdle();
      const next = { ...metadata, context: [...metadata.context, { text, customType }] };
      // Application context enters only the next real turn; no model request or synthetic user message.
      persist(next);
    },
    prompt(text, promptOptions) {
      try { ensureIdle(); } catch (error) { return Promise.reject(error); }
      let resolve!: Run['resolve']; let reject!: Run['reject'];
      const terminal = new Promise<CodexTurn>((yes, no) => { resolve = yes; reject = no; });
      // Requests can fail before the turn is created; keep terminal rejection observed.
      void terminal.catch(() => {});
      const run: Run = { tools: new Set(), terminalComplete: false, cancelled: false, controller: new AbortController(), terminal, resolve, reject, done: Promise.resolve(),
        ...(promptOptions?.deliveryId && { delivery: { id: promptOptions.deliveryId,
          presentation: isDeliveryPresentation(promptOptions.presentation) ? promptOptions.presentation : { version: 1, origin: 'unknown', displayText: text } } }) };
      active = run;
      run.done = Promise.resolve().then(async () => {
        try {
          await notifyMaterialized();
          const started = await transport.sendRequest<{ turn: CodexTurn }>('turn/start', { threadId,
            input: [{ type: 'text', text, text_elements: [] }],
            ...(metadata.context.length > 0 && { additionalContext: { 'hive-context': { kind: 'application',
              value: metadata.context.map(entry => entry.text).join('\n\n') } } }) });
          run.turnId = started.turn.id;
          materialized = true;
          await notifyMaterialized();
          rememberDelivery(run.turnId);
          if ((run.cancelled || run.error) && !run.terminalComplete) await transport.sendRequest('turn/interrupt', { threadId, turnId: run.turnId });
          const turn = await run.terminal;
          await hydrate();
          if (run.error) throw run.error;
          if (turn.status === 'failed') throw new Error(turn.error?.message ?? 'Codex turn failed');
          if (turn.status === 'interrupted' || run.cancelled) {
            run.cancelled = true;
            if (run.delivery) throw new Error('Codex delivery was aborted');
          } else if (turn.status !== 'completed') throw new Error(`Unexpected Codex terminal status: ${turn.status}`);
          else if (run.delivery) persist({ ...metadata, completed: [...new Set([...metadata.completed, run.delivery.id])] });
        } catch (error) {
          if (!run.terminalComplete && run.turnId) {
            run.controller.abort();
            await transport.sendRequest('turn/interrupt', { threadId, turnId: run.turnId });
            await run.terminal.catch(() => {});
          }
          if (!run.cancelled) emit({ type: 'error', sessionId: threadId, message: asError(error).message });
          throw error;
        } finally {
          await Promise.allSettled([...run.tools]);
          active = undefined;
          emit({ type: 'settled', sessionId: threadId, aborted: run.cancelled });
        }
      });
      return run.done;
    },
    async abort() {
      const run = active;
      if (!run) return;
      run.cancelled = true; run.controller.abort();
      if (run.turnId && !run.terminalComplete) await transport.sendRequest('turn/interrupt', { threadId, turnId: run.turnId });
      await run.done.catch(() => {});
    },
    dispose() {
      if (active) throw new Error('Abort and await the active Codex prompt before disposal');
      disposed = true;
      return transport.close();
    },
    currentModel: () => ({ provider: response.modelProvider, id: response.model,
      name: models.find(model => model.model === response.model)?.displayName ?? response.model }),
    listModels: () => models.map(model => ({ provider: response.modelProvider, id: model.model, name: model.displayName, available: true })),
    async setModel(provider, id) {
      ensureIdle();
      if (provider !== response.modelProvider) throw new Error(`Codex provider switching is unsupported: ${provider}`);
      if (!models.some(model => model.model === id)) throw new Error(`Unknown Codex model: ${id}`);
      await transport.sendRequest('thread/settings/update', { threadId, model: id });
      response = { ...response, model: id };
    },
  };
}
function asError(error: unknown): Error { return error instanceof Error ? error : new Error(String(error)); }

import { codexUsage } from '../agent-usage.js';
import { promptImages } from '../prompt-images.js';
import type { TSchema } from 'typebox';
import { Value } from 'typebox/value';
import { isDeliveryPresentation, type DeliveryPresentation } from '../delivery-presentation.js';
import { CodexTransport, type CodexServerRequest } from './codex-transport.js';
import { mkdirSync } from 'node:fs';
import { CodexInteractions } from './codex-interactions.js';
import { CodexProjection } from './codex-projection.js';
import { CodexEvents } from './codex-events.js';
import { readCodexMetadata, writeCodexMetadata } from './codex-metadata.js';
import type { CodexDelivery, CodexItem, CodexMetadata, CodexThreadResponse, CodexTurn } from './codex-history.js';
import { isCodexToolItem } from './codex-history.js';
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
type Model = { id: string; model: string; displayName: string; hidden: boolean; supportedReasoningEfforts?: {reasoningEffort:string;description:string}[] };
type Run = { tools: Set<Promise<unknown>>; terminalComplete: boolean; turnId?: string; cancelled: boolean; controller: AbortController; delivery?: { id: string; presentation: DeliveryPresentation };
  terminal: Promise<CodexTurn>; resolve: (turn: CodexTurn) => void; reject: (error: Error) => void; done: Promise<void>; error?: Error };

/** Codex 0.166 app-server public JSON-RPC; no terminal scraping or Pi model intermediary. */
export async function createCodexSession(options: CodexSessionOptions): Promise<CodexSessionHandle> {
  let active: Run | undefined;
  let nativeUsage:unknown;
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
  let transport!: CodexTransport;
  const interactions = new CodexInteractions(() => threadId, emit, id => transport?.cancelServerRequest(id));
  transport = await CodexTransport.start({ cwd: options.cwd,
    ...(options.command && { command: options.command }),
    onClose(info) {
      interactions.clear('disconnected');
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
      if (interactions.supports(request.method) && params.threadId === threadId && active && active.turnId === params.turnId && !active.terminalComplete) {
        try { return interactions.request(request); }
        catch (error) {
          active.error = asError(error);
          active.controller.abort();
          void transport.sendRequest('turn/interrupt', { threadId, turnId: active.turnId }).catch(active.reject);
          throw error;
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
        if (notification.method === 'thread/tokenUsage/updated') {nativeUsage=structuredClone(params.tokenUsage);return;}
        if (notification.method === 'serverRequest/resolved') {
          if (typeof params.requestId === 'number' || typeof params.requestId === 'string') interactions.resolved(params.requestId);
          return;
        }
        if (notification.method === 'turn/started' && active) active.turnId = (params.turn as CodexTurn).id;
        if (notification.method === 'item/started' || notification.method === 'item/completed') {
          const turnId = String(params.turnId);
          if (active?.turnId === turnId && !active.terminalComplete) interactions.item(turnId, params.item);
          rememberDelivery(turnId);
          if (notification.method === 'item/completed' || isCodexToolItem(params.item as CodexItem)) projection?.put(params.item as CodexItem, turnId);
          events?.item(params.item as CodexItem, turnId, notification.method === 'item/completed');
        } else if (notification.method === 'turn/completed' && active) {
          const turn = params.turn as CodexTurn;
          if (!active.turnId || active.turnId === turn.id) {
            active.terminalComplete = true;
            interactions.endTurn(turn.id, turn.status === 'interrupted' ? 'aborted' : 'turn-ended');
            if (turn.status === 'interrupted' || turn.status === 'failed') {
              for (const item of events?.interruptedItems(turn.id) ?? []) {
                projection?.put(item, turn.id);
                events?.item(item, turn.id, true);
              }
            } else if (turn.status === 'completed') {
              // A model turn may finish while a native background command remains active.
              for (const item of events?.unfinishedTools(turn.id) ?? []) projection?.put(item, turn.id);
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
  const recoverHistory = async (thread: CodexThreadResponse['thread']) => {
    if (thread.historyMode === 'legacy') { projection!.recover(thread.turns); return; }
    if (thread.historyMode !== 'paginated') throw new Error('Unsupported Codex history contract');
    const turns: CodexTurn[] = [];
    let cursor: string | null = null;
    do {
      const page: {data: (CodexTurn & {itemsView?: string})[]; nextCursor?: string | null} = await transport.sendRequest('thread/turns/list', {threadId, cursor, sortDirection: 'asc', itemsView: 'full', limit: 100});
      if (page.data.some(turn => turn.itemsView !== 'full')) throw new Error('Codex did not return complete turn items');
      turns.push(...page.data); cursor = page.nextCursor ?? null;
    } while (cursor);
    projection!.recover(turns, true);
  };
  const hydrate = async () => {
    const read = await transport.sendRequest<CodexThreadResponse>('thread/read', { threadId, includeTurns: false });
    if (read.thread.historyMode === 'legacy') {
      const full = await transport.sendRequest<CodexThreadResponse>('thread/read', { threadId, includeTurns: true });
      await recoverHistory(full.thread);
    } else await recoverHistory(read.thread);
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
    else response = await transport.sendRequest<CodexThreadResponse>('thread/start', { cwd: options.cwd, allowProviderModelFallback: false, ...modelParams,
      dynamicTools: options.customTools?.map(tool => ({ type: 'function', name: tool.name, description: tool.description, inputSchema: tool.parameters })) ?? [] });
    if (options.model && (response!.model !== options.model.id || response!.modelProvider !== options.model.provider)) throw new Error('Codex did not select the requested model/provider');
    threadId = response!.thread.id;
    persist(metadata);
    mkdirSync(options.sessionDir, { recursive: true, mode: 0o700 });
    projection = new CodexProjection(options.sessionDir, threadId);
    events = new CodexEvents(threadId, emit, id => metadata.deliveries.find(entry => entry.turnId === id));
    if (materialized) await recoverHistory(response!.thread);
    else if (!['legacy', 'paginated'].includes(response!.thread.historyMode)) throw new Error('Unsupported Codex history contract');
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
  const reasoning = async () => {
    if (disposed) throw new Error('Codex session is disposed');
    const read = await transport.sendRequest<{thread:{model?:string;reasoningEffort?:string|null}}>('thread/read', {threadId,includeTurns:false});
    const level = read.thread.reasoningEffort;
    if (level !== null && level !== undefined && typeof level !== 'string') throw new Error('Codex 未返回有效的推理强度。');
    const model = models.find(model => model.model === (read.thread.model ?? response.model));
    return {level:level ?? 'native',levels:model?.supportedReasoningEfforts?.map(option=>option.reasoningEffort) ?? [],description:'从原生 Codex 线程读取；档位由当前模型提供。'};
  };
  return {
    sessionId: threadId, threadId,
    get materialized() { return materialized; },
    get sessionFile() { return response.thread.path ?? undefined; },
    history: () => events!.snapshot().history,
    snapshot: () => events!.snapshot(),
    usage:()=>codexUsage(nativeUsage),
    interactions: () => interactions.snapshot(),
    respondInteraction: (id, response) => interactions.respond(id, response),
    completedDeliveryIds: () => [...metadata.completed],
    hasContext: customType => metadata.context.some(entry => entry.customType === customType),
    async addContext(text, customType = 'hive-context') {
      ensureIdle();
      const next = { ...metadata, context: [...metadata.context, { text, customType }] };
      // Application context enters only the next real turn; no model request or synthetic user message.
      persist(next);
    },
    prompt(text, promptOptions) {
      let images: ReturnType<typeof promptImages>;
      try { images = promptImages(promptOptions?.images); } catch (error) { return Promise.reject(error); }
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
            input: [...(text ? [{ type: 'text', text, text_elements: [] }] : []), ...images.map(image=>({type:'image',url:`data:${image.mimeType};base64,${image.data}`}))],
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
          if (run.turnId) interactions.endTurn(run.turnId);
          active = undefined;
          emit({ type: 'settled', sessionId: threadId, aborted: run.cancelled });
        }
      });
      return run.done;
    },
    async abort() {
      interactions.clear('aborted');
      const run = active;
      if (!run) return;
      run.cancelled = true; run.controller.abort();
      if (run.turnId && !run.terminalComplete) await transport.sendRequest('turn/interrupt', { threadId, turnId: run.turnId });
      await run.done.catch(() => {});
    },
    dispose() {
      if (active) throw new Error('Abort and await the active Codex prompt before disposal');
      interactions.clear('disposed');
      disposed = true;
      return transport.close();
    },
    reasoning,
    async setReasoning(level) {
      ensureIdle();
      if (!(await reasoning()).levels.includes(level)) throw new TypeError('当前 Codex 模型不支持此推理强度。');
      ensureIdle();
      await transport.sendRequest('thread/settings/update',{threadId,effort:level});
      return reasoning();
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

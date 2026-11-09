import { codexUsage } from '../agent-usage.js';
import { promptImages } from '../prompt-images.js';
import type { TSchema } from 'typebox';
import { Value } from 'typebox/value';
import { isDeliveryPresentation, type DeliveryPresentation } from '../delivery-presentation.js';
import { CodexTransport, CodexRpcError, type CodexServerRequest } from './codex-transport.js';
import { updateCodexSettings } from './codex-settings.js';
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
type InFlightDelivery = { id: string; text: string; presentation: DeliveryPresentation; itemId?: string; accepted?: boolean };
type Run = { appended: InFlightDelivery[]; requests: Set<Promise<unknown>>; tools: Set<Promise<unknown>>; nativeTools: Map<string, { done: Promise<void>; complete(): void }>; interrupting?: Promise<void>; terminalComplete: boolean; turnId?: string; cancelled: boolean; controller: AbortController; delivery?: InFlightDelivery;
  terminal: Promise<CodexTurn>; resolve: (turn: CodexTurn) => void; reject: (error: Error) => void; done: Promise<void>; error?: Error };

/** Codex 0.166 app-server public JSON-RPC; no terminal scraping or Pi model intermediary. */
export async function createCodexSession(options: CodexSessionOptions): Promise<CodexSessionHandle> {
  let active: Run | undefined;
  const lifetime = new AbortController();
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
  const rememberDelivery = (turnId: string, item?: CodexItem) => {
    const run = active;
    if (!run || run.turnId !== turnId) return;
    const primary = run.delivery;
    if (primary && !metadata.deliveries.some(entry => entry.turnId === turnId && entry.deliveryId === primary.id)) {
      persist({ ...metadata, deliveries: [...metadata.deliveries, { turnId, deliveryId: primary.id, presentation: primary.presentation }] });
    }
    if (item?.type !== 'userMessage' || !Array.isArray(item.content)) return;
    const text = codexInputText(item);
    const delivery = [primary, ...run.appended.filter(entry => entry.accepted)].find(entry => entry && !entry.itemId && entry.text === text);
    if (!delivery || metadata.deliveries.some(entry => entry.turnId === turnId && entry.itemId === item.id)) return;
    const next = { turnId, itemId: item.id, deliveryId: delivery.id, presentation: delivery.presentation };
    persist({ ...metadata, deliveries: [...metadata.deliveries.filter(entry => entry.deliveryId !== delivery.id || entry.turnId !== turnId), next] });
    delivery.itemId = item.id;
  };
  let transport!: CodexTransport;
  const interactions = new CodexInteractions(() => threadId, emit, id => transport?.cancelServerRequest(id));
  const interrupt = async (run: Run) => {
    if (!run.turnId || run.terminalComplete) return;
    return run.interrupting ??= (async () => {
      try { await transport.sendRequest('turn/interrupt', { threadId, turnId: run.turnId }); }
      catch (error) {
        // A real terminal notification can win the race with an RPC rejection.
        if (run.terminalComplete && !run.error) return;
        run.error ??= asError(error);
        run.controller.abort();
        // No native terminal was confirmed. Release the owned connection and
        // prompt before allowing another session to replace this one.
        try { await transport.close(); }
        finally { run.terminalComplete = true; run.reject(run.error); }
        throw run.error;
      }
    })();
  };
  transport = await CodexTransport.start({ cwd: options.cwd,
    ...(options.command && { command: options.command }),
    onClose(info) {
      lifetime.abort(info.error);
      interactions.clear('disconnected');
      const error = active?.error ?? info.error;
      if (active) { active.error = error; active.controller.abort(); active.terminalComplete = true; active.reject(error); }
      if (initialized && !disposed) options.onUnexpectedExit?.(error);
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
          let complete!: () => void;
          const done = new Promise<void>(resolve => { complete = resolve; });
          run.nativeTools.set(String(params.callId), { done, complete });
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
          void interrupt(active).catch(active.reject);
          throw error;
        }
      }
      if (options.onServerRequest) return options.onServerRequest(request);
      const error = new Error(`Codex requires unsupported interaction: ${request.method}`);
      emit({ type: 'error', sessionId: threadId, message: error.message });
      if (active) {
        active.error = error;
        active.controller.abort();
        if (active.turnId) void interrupt(active).catch(active.reject);
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
          rememberDelivery(turnId, params.item as CodexItem);
          if (notification.method === 'item/completed' || isCodexToolItem(params.item as CodexItem)) projection?.put(params.item as CodexItem, turnId);
          const item = params.item as CodexItem;
          const awaitingSource = item.type === 'userMessage' && active?.appended.some(entry =>
            !entry.accepted && entry.text === codexInputText(item));
          // A raw native user row and its later Hive delivery use different UI
          // IDs. Wait for association instead of briefly emitting both rows.
          if (!awaitingSource) events?.item(item, turnId, notification.method === 'item/completed');
          if (notification.method === 'item/completed' && active?.turnId === turnId) {
            const id = (params.item as CodexItem).id;
            active.nativeTools.get(id)?.complete();
            active.nativeTools.delete(id);
          }
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
          else if (active.turnId) void interrupt(active).catch(active.reject);
        }
      }
    },
  });
  const resume = async (extra: Record<string, unknown> = {}) => {
    response = await transport.sendRequest<CodexThreadResponse>('thread/resume', { threadId, ...extra });
  };
  const recoverHistory = async (thread: CodexThreadResponse['thread']) => {
    if (thread.historyMode === 'legacy') { projection!.recover(thread.turns); return thread.turns; }
    if (thread.historyMode !== 'paginated') throw new Error('Unsupported Codex history contract');
    const turns: CodexTurn[] = [];
    let cursor: string | null = null;
    do {
      const page: {data: (CodexTurn & {itemsView?: string})[]; nextCursor?: string | null} = await transport.sendRequest('thread/turns/list', {threadId, cursor, sortDirection: 'asc', itemsView: 'full', limit: 100});
      if (page.data.some(turn => turn.itemsView !== 'full')) throw new Error('Codex did not return complete turn items');
      turns.push(...page.data); cursor = page.nextCursor ?? null;
    } while (cursor);
    projection!.recover(turns, true);
    return turns;
  };
  const hydrate = async () => {
    const read = await transport.sendRequest<CodexThreadResponse>('thread/read', { threadId, includeTurns: false });
    let turns: CodexTurn[];
    if (read.thread.historyMode === 'legacy') {
      const full = await transport.sendRequest<CodexThreadResponse>('thread/read', { threadId, includeTurns: true });
      turns = await recoverHistory(full.thread);
    } else turns = await recoverHistory(read.thread);
    events!.restore(projection!.current());
    return turns;
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
    events = new CodexEvents(threadId, emit, (id, itemId) => metadata.deliveries.find(entry => entry.turnId === id && entry.itemId === itemId)
      ?? metadata.deliveries.find(entry => entry.turnId === id && entry.itemId === undefined));
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
      const run: Run = { appended: [], requests: new Set(), tools: new Set(), nativeTools: new Map(), terminalComplete: false, cancelled: false, controller: new AbortController(), terminal, resolve, reject, done: Promise.resolve(),
        ...(promptOptions?.deliveryId && { delivery: { id: promptOptions.deliveryId, text,
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
          if ((run.cancelled || run.error) && !run.terminalComplete) await interrupt(run);
          const turn = await run.terminal;
          // Interrupted/failed turns must not wait for an outstanding steer ACK.
          // Stop will close this owned transport; the durable notification stays.
          if (turn.status === 'completed' && !run.cancelled && !run.error) await Promise.allSettled([...run.requests]);
          const nativeTurns = await hydrate();
          for (const entry of projection!.current()) if (entry.turnId === turn.id) rememberDelivery(turn.id, entry.item);
          if (run.error) throw run.error;
          if (turn.status === 'failed') throw new Error(turn.error?.message ?? 'Codex turn failed');
          if (turn.status === 'interrupted' || run.cancelled) {
            run.cancelled = true;
            if (run.delivery) throw new Error('Codex delivery was aborted');
          } else if (turn.status !== 'completed') throw new Error(`Unexpected Codex terminal status: ${turn.status}`);
          else {
            const native = nativeTurns.find(entry => entry.id === turn.id && entry.status === 'completed');
            const consumed = run.appended.filter(entry => entry.accepted && entry.itemId
              && native?.items.some(item => item.id === entry.itemId && item.type === 'userMessage')).map(entry => entry.id);
            persist({ ...metadata, completed: [...new Set([...metadata.completed, ...(run.delivery ? [run.delivery.id] : []), ...consumed])] });
          }
        } catch (error) {
          if (!run.terminalComplete && run.turnId) {
            run.controller.abort();
            await interrupt(run);
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
    async steer(text, input) {
      const run = active;
      if (disposed || !run?.turnId || run.terminalComplete || run.cancelled || run.error
        || run.appended.some(entry => entry.text === text)) return false;
      const delivery: InFlightDelivery = { id: input.deliveryId, text, presentation: input.presentation };
      run.appended.push(delivery);
      const request = transport.sendRequest<{turnId: string}>('turn/steer', {
        threadId, expectedTurnId: run.turnId, input: [{type: 'text', text, text_elements: []}],
      }).then(response => {
        if (response.turnId !== run.turnId) throw new Error('Codex steered a different native turn');
        delivery.accepted = true;
        // Native item notifications may precede the RPC acknowledgement. Bind
        // only accepted inputs, then refresh that same stable display item.
        try {
          for (const entry of projection!.current()) if (entry.turnId === run.turnId) rememberDelivery(entry.turnId, entry.item);
          const bound = projection!.current().find(entry => entry.item.id === delivery.itemId);
          if (bound) events!.item(bound.item, bound.turnId, true);
        } catch (error) {
          run.error = asError(error); run.controller.abort();
          if (!run.terminalComplete) void interrupt(run).catch(run.reject);
          throw error;
        }
      });
      run.requests.add(request);
      let failure: unknown, endedBeforeRejection = false;
      try { await request; }
      catch (error) { failure = error; endedBeforeRejection = run.terminalComplete && !run.error; }
      finally { run.requests.delete(request); }
      // The primary delivery reports failed/aborted turns. Appended inputs remain
      // in their spool and must be eligible for a later explicit resume.
      try { await run.done; } catch { return false; }
      if (failure) {
        if (endedBeforeRejection && failure instanceof CodexRpcError && failure.code === -32600) return false;
        throw failure;
      }
      if (!metadata.completed.includes(delivery.id)) throw new Error('Codex did not confirm consumption of the appended delivery');
      return true;
    },
    async abort(stopOptions) {
      interactions.clear('aborted');
      const run = active;
      if (!run) return;
      if (stopOptions?.settleTools && run.nativeTools.size) {
        // A workflow can retire this member as soon as report commits, before
        // its CLI response reaches Codex. Wait for item/completed, not a sleep
        // or a fabricated success. Use the owned CLI's normal shutdown budget.
        let timer: ReturnType<typeof setTimeout> | undefined;
        try {
          await Promise.race([
            Promise.all([...run.nativeTools.values()].map(tool => tool.done)),
            run.terminal.catch(() => {}),
            new Promise<void>(resolve => { timer = setTimeout(resolve, 1500); }),
          ]);
        } finally { clearTimeout(timer); }
      }
      run.cancelled = true; run.controller.abort();
      try { await interrupt(run); }
      finally { await run.done.catch(() => {}); }
      if (run.error) throw run.error;
    },
    dispose() {
      if (active) throw new Error('Abort and await the active Codex prompt before disposal');
      interactions.clear('disposed');
      disposed = true;
      lifetime.abort(new Error('Codex session is disposed'));
      return transport.close();
    },
    reasoning,
    async setReasoning(level) {
      ensureIdle();
      const current = await reasoning();
      if (!current.levels.includes(level)) throw new TypeError('当前 Codex 模型不支持此推理强度。');
      ensureIdle();
      if (current.level === level) return current;
      await updateCodexSettings(transport, threadId, { effort: level }, lifetime.signal);
      return reasoning();
    },
    currentModel: () => ({ provider: response.modelProvider, id: response.model,
      name: models.find(model => model.model === response.model)?.displayName ?? response.model }),
    listModels: () => models.map(model => ({ provider: response.modelProvider, id: model.model, name: model.displayName, available: true })),
    async setModel(provider, id) {
      ensureIdle();
      if (provider !== response.modelProvider) throw new Error(`Codex provider switching is unsupported: ${provider}`);
      if (!models.some(model => model.model === id)) throw new Error(`Unknown Codex model: ${id}`);
      if (response.model === id) return;
      const applied = await updateCodexSettings(transport, threadId, { model: id }, lifetime.signal);
      if (applied.model !== id) throw new Error('Codex did not select the requested model');
      response = { ...response, model: id };
    },
  };
}
function codexInputText(item: CodexItem): string | undefined {
  return Array.isArray(item.content) ? item.content.filter(part => part.type === 'text').map(part => part.text).join('') : undefined;
}
function asError(error: unknown): Error { return error instanceof Error ? error : new Error(String(error)); }

import { randomUUID } from 'node:crypto';
import { join } from 'node:path';
import { AcpTransport, AcpRpcError } from './acp-transport.js';
import { AcpHistory } from './acp-history.js';
import { acpInitialize, acpPermission, acpSession, acpUpdate, object } from './acp-protocol.js';
import { createAcpTeamServer } from './acp-team-stdio.js';
import type { AgentInteraction, PiEvent, PiSessionHandle } from './types.js';
export interface AcpTeamBinding {
    workspaceId: string;
    agentId: string;
    token: string;
    hivePort: string;
    cwd: string;
}
export interface AcpSessionOptions {
    command: string;
    args?: string[];
    env?: NodeJS.ProcessEnv;
    cwd: string;
    sessionDir: string;
    sessionFile?: string;
    signal?: AbortSignal;
    requestTimeoutMs?: number;
    cancelTimeoutMs?: number;
    onEvent: (event: PiEvent) => void;
    onUnexpectedExit?: (error: Error) => void;
    teamBinding?: () => AcpTeamBinding | undefined;
}
export interface AcpSessionHandle extends PiSessionHandle {
    readonly nativeResume: boolean;
    readonly readOnly: boolean;
    teamConnected(): boolean;
    uncertainDeliveryIds(): readonly string[];
    dispose(): Promise<void>;
}
type Permission = {
    interaction: AgentInteraction;
    options: string[];
    resolve: (value: unknown) => void;
};
/** ACP v1 only. Prompt completion consumes a Hive delivery, never reports a Hive task. */
export async function createAcpSession(options: AcpSessionOptions): Promise<AcpSessionHandle> {
    const file = options.sessionFile ?? join(options.sessionDir, `acp-${randomUUID()}.json`);
    let store = await AcpHistory.load(file);
    const restoring = !!store;
    if (options.sessionFile && !store)
        throw new Error('ACP transcript requested for restore is missing; refusing to create a replacement session');
    let sessionId = store?.state.sessionId ?? '';
    let disposed = false;
    let failure: Error | undefined;
    let ready = false;
    let loading = false;
    let readOnly = false;
    let active: {
        cancelled: boolean;
        done: Promise<void>;
    } | undefined;
    let transport: AcpTransport | undefined;
    const binding = options.teamBinding?.();
    const team = binding ? await createAcpTeamServer(binding) : undefined;
    const permissions = new Map<string, Permission>();
    let work = Promise.resolve();
    const earlyUpdates: unknown[] = [];
    let replay: unknown[] = [];
    let replayIds: string[] = [];
    let assistantId = '';
    let assistant: Record<string, unknown> | undefined;
    const tools = new Map<string, Record<string, unknown>>();
    const redact = (value: unknown): unknown => { if (typeof value === 'string')
        return binding?.token ? value.replaceAll(binding.token, '[redacted]') : value; if (Array.isArray(value))
        return value.map(redact); if (object(value))
        return Object.fromEntries(Object.entries(value).map(([k, v]) => [k, redact(v)])); return value; };
    const emit = (event: PiEvent) => { try {
        options.onEvent(redact(event) as PiEvent);
    }
    catch { /* UI errors do not approve or consume input. */ } };
    const clearPermissions = (reason: 'aborted' | 'turn-ended' | 'disconnected' | 'disposed') => { for (const [id, p] of permissions) {
        p.resolve({ outcome: { outcome: 'cancelled' } });
        emit({ type: 'interaction_resolved', sessionId, interactionId: id, reason });
    } permissions.clear(); };
    const fail = (error: unknown) => { if (failure || disposed)
        return; failure = new Error(String(redact(error instanceof Error ? error.message : String(error)))); clearPermissions('disconnected'); void transport?.close(100).catch(() => { }); if (ready && !active)
        options.onUnexpectedExit?.(failure); };
    const persist = async (change: Parameters<AcpHistory['write']>[0]) => { try {
        await store!.write(s => { change(s); Object.assign(s, redact(s)); });
    }
    catch (error) {
        fail(error);
        throw error;
    } };
    const put = async (message: unknown, id: string) => {
        if (loading) {
            const i = replayIds.indexOf(id);
            if (i < 0) {
                replayIds.push(id);
                replay.push(structuredClone(message));
            }
            else
                replay[i] = structuredClone(message);
            return;
        }
        await persist(state => { const i = state.historyIds.indexOf(id); if (i < 0) {
            state.historyIds.push(id);
            state.history.push(structuredClone(message));
        }
        else
            state.history[i] = structuredClone(message); });
    };
    const update = async (params: unknown) => {
        const parsed = acpUpdate.safeParse(params);
        if (!parsed.success)
            throw new Error('Invalid ACP session update');
        if (parsed.data.sessionId !== sessionId)
            throw new Error('ACP update belongs to another session');
        const u = parsed.data.update;
        const kind = u.sessionUpdate;
        if (kind === 'agent_message_chunk' || kind === 'agent_thought_chunk' || kind === 'user_message_chunk') {
            if (!object(u.content) || u.content.type !== 'text' || typeof u.content.text !== 'string')
                return;
            if (kind === 'user_message_chunk') {
                await put({ role: 'user', content: [u.content], timestamp: Date.now() }, randomUUID());
                assistant = undefined;
                return;
            }
            if (!assistant) {
                assistantId = randomUUID();
                assistant = { role: 'assistant', content: [], timestamp: Date.now() };
            }
            const content = assistant.content as Record<string, unknown>[];
            const type = kind === 'agent_thought_chunk' ? 'thinking' : 'text';
            let block = content.at(-1);
            if (!block || block.type !== type) {
                block = type === 'text' ? { type, text: '' } : { type, thinking: '' };
                content.push(block);
            }
            const field = type === 'text' ? 'text' : 'thinking';
            block[field] = String(block[field]) + u.content.text;
            await put(assistant, assistantId);
            if (!loading)
                emit({ type: 'assistant_delta', sessionId, messageId: assistantId, kind: type, contentIndex: content.length - 1, delta: u.content.text });
        }
        else if (kind === 'tool_call' || kind === 'tool_call_update') {
            if (typeof u.toolCallId !== 'string')
                throw new Error('Invalid ACP tool call');
            const old = tools.get(u.toolCallId);
            const tool = { ...old, ...u };
            tools.set(u.toolCallId, tool);
            const name = typeof tool.title === 'string' ? tool.title : 'ACP tool';
            await put({ role: 'toolResult', toolCallId: u.toolCallId, toolName: name, content: tool.content ?? [], details: tool, isError: tool.status === 'failed', timestamp: Date.now() }, `tool-${u.toolCallId}`);
            if (!loading) {
                if (!old)
                    emit({ type: 'tool_start', sessionId, toolCallId: u.toolCallId, toolName: name, args: tool.rawInput ?? {} });
                if (tool.status === 'completed' || tool.status === 'failed')
                    emit({ type: 'tool_end', sessionId, toolCallId: u.toolCallId, toolName: name, result: tool, isError: tool.status === 'failed' });
                else
                    emit({ type: 'tool_update', sessionId, toolCallId: u.toolCallId, toolName: name, result: tool });
            }
            assistant = undefined;
        }
    };
    try {
        transport = await AcpTransport.start({ command: options.command, ...(options.args && { args: options.args }), env: { ...options.env, ...team?.childEnv }, cwd: options.cwd, ...(options.signal && { signal: options.signal }), ...(options.requestTimeoutMs && { requestTimeoutMs: options.requestTimeoutMs }), onClose: info => { if (!disposed && !readOnly)
                fail(info.error); }, onNotification: n => { if (n.method === 'session/update') {
                if (!sessionId) {
                    earlyUpdates.push(n.params);
                    return;
                }
                work = work.then(() => update(n.params)).catch(fail);
            } }, onRequest: async (r) => {
                if (r.method !== 'session/request_permission')
                    throw new AcpRpcError(-32601, `Unsupported client method: ${r.method}`);
                const p = acpPermission.safeParse(r.params);
                if (!p.success)
                    throw new AcpRpcError(-32602, 'Invalid permission request');
                if (p.data.sessionId !== sessionId || !active || loading)
                    throw new AcpRpcError(-32602, 'Permission does not belong to an active session turn');
                const id = randomUUID();
                const interaction = { id, backend: 'acp', method: 'session/request_permission', nativeRequestId: r.id, sessionId, state: 'pending', params: p.data } as unknown as AgentInteraction;
                return await new Promise(resolve => { permissions.set(id, { interaction, options: p.data.options.map(o => o.optionId), resolve }); emit({ type: 'interaction', sessionId, interaction }); });
            } });
        const init = acpInitialize.parse(transport.initializeResult);
        const nativeResume = init.agentCapabilities?.loadSession === true;
        if (restoring && !nativeResume) {
            readOnly = true;
            await transport.close();
            await team?.dispose();
        }
        else if (restoring) {
            loading = true;
            await transport.sendRequest('session/load', { sessionId, cwd: options.cwd, mcpServers: team ? [team.server] : [] });
            await work;
            if (failure)
                throw failure;
            await persist(s => { s.history = replay; s.historyIds = replayIds; });
            loading = false;
        }
        else {
            const result = acpSession.parse(await transport.sendRequest('session/new', { cwd: options.cwd, mcpServers: team ? [team.server] : [] }));
            sessionId = result.sessionId;
            store = new AcpHistory(file, { version: 1, sessionId, history: [], historyIds: [], contexts: [], deliveries: {} });
            await persist(() => { });
            for (const params of earlyUpdates)
                work = work.then(() => update(params)).catch(fail);
            await work;
            if (failure)
                throw failure;
        }
        ready = true;
        const check = () => { if (disposed || failure)
            throw failure ?? new Error('ACP session is disposed'); if (readOnly)
            throw new Error('ACP agent does not support native session restore; transcript is read-only'); };
        const validate = (id: string, response: unknown) => { const p = permissions.get(id); if (!p)
            throw new Error('Permission request is no longer pending'); if (!object(response) || typeof response.optionId !== 'string' || !p.options.includes(response.optionId))
            throw new Error('Choose a native ACP permission optionId'); return { p, optionId: response.optionId }; };
        return { sessionId, sessionFile: file, nativeResume, readOnly, teamConnected: () => team?.connected() ?? false, uncertainDeliveryIds: () => Object.keys(store!.state.deliveries).filter(id => store!.state.deliveries[id] === 'sent'),
            history: () => structuredClone(store!.state.history), snapshot: () => ({ history: structuredClone(store!.state.history), historyMessageIds: [...store!.state.historyIds] }), hasContext: type => store!.state.contexts.some(c => c.customType === type) || store!.state.history.some(m => object(m) && m.customType === type),
            completedDeliveryIds: () => Object.keys(store!.state.deliveries).filter(id => store!.state.deliveries[id] === 'completed'), supportsImages: () => false, currentModel: () => undefined, listModels: () => [], async setModel() { throw new Error('ACP model selection is not implemented'); },
            interactions: () => [...permissions.values()].map(p => p.interaction), validateInteractionResponse: (id, response) => { validate(id, response); }, respondInteraction(id, response) { check(); const { p, optionId } = validate(id, response); permissions.delete(id); p.resolve({ outcome: { outcome: 'selected', optionId } }); emit({ type: 'interaction_resolved', sessionId, interactionId: id, reason: 'resolved' }); },
            async addContext(text, customType = 'hive-context') { check(); if (active)
                throw new Error('ACP turn is active'); await persist(s => { s.contexts.push({ text, customType }); s.history.push({ role: 'custom', customType, content: text, display: false, timestamp: Date.now() }); s.historyIds.push(randomUUID()); }); },
            prompt(text, promptOptions) {
                try {
                    check();
                    if (active)
                        throw new Error('ACP turn is active');
                    if (promptOptions?.images?.length)
                        throw new Error('ACP image input is not implemented');
                    if (binding && !team?.connected())
                        throw new Error('ACP agent has not connected the Hive MCP team bridge');
                    const id = promptOptions?.deliveryId;
                    if (id && store!.state.deliveries[id] === 'completed')
                        return Promise.resolve();
                    if (id && store!.state.deliveries[id] === 'sent')
                        throw new Error('ACP delivery outcome is uncertain; inspect the native session before explicitly retrying as a new delivery');
                }
                catch (error) {
                    return Promise.reject(error);
                }
                const run = { cancelled: false, done: Promise.resolve() };
                active = run;
                assistant = undefined;
                tools.clear();
                run.done = (async () => {
                    try {
                        const context = store!.state.contexts.map(c => c.text).join('\n\n');
                        const deliveryId = promptOptions?.deliveryId;
                        const userId = deliveryId ?? randomUUID();
                        await persist(s => { (s.inputs ??= []).push({ id: userId, text, ...(promptOptions?.presentation && { presentation: promptOptions.presentation }) }); if (deliveryId)
                            s.deliveries[deliveryId] = 'sent'; s.contexts = []; s.historyIds.push(userId); s.history.push({ role: 'user', content: [{ type: 'text', text }], timestamp: Date.now(), ...(promptOptions?.presentation && { presentation: { ...promptOptions.presentation, ...(deliveryId && { deliveryId }) } }) }); });
                        emit({ type: 'message_end', sessionId, messageId: userId, message: store!.state.history.at(-1) });
                        if (run.cancelled)
                            throw new Error('ACP turn cancelled before send');
                        const result = await transport!.sendRequest('session/prompt', { sessionId, prompt: [{ type: 'text', text: context ? `${context}\n\n${text}` : text }] });
                        await work;
                        if (failure)
                            throw failure;
                        if (run.cancelled || !object(result) || result.stopReason === 'cancelled')
                            throw new Error('ACP prompt was cancelled; delivery outcome is uncertain');
                        if (!['end_turn', 'max_tokens', 'max_turn_requests', 'refusal'].includes(String(result.stopReason)))
                            throw new Error('Invalid ACP stop reason');
                        if (assistant)
                            emit({ type: 'message_end', sessionId, messageId: assistantId, message: assistant });
                        if (deliveryId)
                            await persist(s => { s.deliveries[deliveryId] = 'completed'; });
                    }
                    catch (error) {
                        fail(error);
                        emit({ type: 'error', sessionId, message: error instanceof Error ? error.message : String(error) });
                        throw failure ?? error;
                    }
                    finally {
                        clearPermissions('turn-ended');
                        active = undefined;
                        emit({ type: 'settled', sessionId, aborted: run.cancelled });
                        if (failure && !disposed)
                            options.onUnexpectedExit?.(failure);
                    }
                })();
                return run.done;
            },
            async abort() { if (!active)
                return; const run = active; run.cancelled = true; clearPermissions('aborted'); try {
                await transport!.notify('session/cancel', { sessionId });
            }
            catch (error) {
                fail(error);
            } let timer: NodeJS.Timeout | undefined; try {
                await Promise.race([run.done.catch(() => { }), new Promise<void>(resolve => { timer = setTimeout(() => { fail(new Error('ACP cancellation timed out; delivery outcome is uncertain')); resolve(); }, options.cancelTimeoutMs ?? 1500); })]);
            }
            finally {
                clearTimeout(timer);
            } await run.done.catch(() => { }); },
            async dispose() { if (disposed)
                return; disposed = true; clearPermissions('disposed'); await transport!.close(); await active?.done.catch(() => { }); await team?.dispose(); },
        };
    }
    catch (error) {
        disposed = true;
        clearPermissions('disposed');
        await transport?.close();
        await team?.dispose();
        throw new Error(String(redact(error instanceof Error ? error.message : String(error))));
    }
}

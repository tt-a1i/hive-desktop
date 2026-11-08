import { readClaudeReasoning } from './claude-reasoning.js';
import { promptImages } from '../prompt-images.js';
import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process';
import { randomUUID, type UUID } from 'node:crypto';
import { accessSync, constants } from 'node:fs';
import { delimiter, extname, isAbsolute, join, resolve } from 'node:path';
import { getSessionMessages, query, type CanUseTool, type ModelInfo, type OnElicitation, type OnUserDialog,
  type Query, type EffortLevel, type SDKResultMessage, type SessionMessage } from '@anthropic-ai/claude-agent-sdk';
import { isDeliveryPresentation } from '../delivery-presentation.js';
import { ClaudeInteractions } from './claude-interactions.js';
import { ClaudeInput, claudeDeadline } from './claude-input.js';
import { claudeCompletionAssistant } from './claude-completion.js';
import type { projectClaudeMessage } from './claude-history.js';
import { ClaudeProjection } from './claude-projection.js';
import { readClaudeMetadata, writeClaudeMetadata, type ClaudeMetadata } from './claude-metadata.js';
import { createClaudeTeamServer, type ClaudeTeamBinding } from './claude-team.js';
import type { PiEvent, PiSessionHandle } from './types.js';
export interface ClaudeSessionOptions {
  cwd: string; sessionDir: string; sessionId?: string; command?: string;
  model?: { provider: string; id: string }; onEvent: (event: PiEvent) => void;
  teamBinding?: () => ClaudeTeamBinding | undefined;
  onUnexpectedExit?: (error: Error) => void;
  canUseTool?: CanUseTool; onUserDialog?: OnUserDialog; supportedDialogKinds?: string[]; onElicitation?: OnElicitation;
}
export interface ClaudeSessionHandle extends PiSessionHandle {
  readonly teamToolName: string; readonly processId: number | undefined;
  catalog(): { commands: string[]; models: string[]; mcpServers: { name: string; status: string }[] };
  dispose(): Promise<void>;
}
type Run = { uuid: UUID; cancelled: boolean; controller: AbortController; tools: Set<Promise<unknown>>;
  terminal: Promise<SDKResultMessage>; resolve(value: SDKResultMessage): void; reject(error: Error): void; done: Promise<void>; assistants: Set<string>; error?: Error };

/** Launches the user's original Claude Code with the official SDK; never handles its credentials. */
export async function createClaudeSession(options: ClaudeSessionOptions): Promise<ClaudeSessionHandle> {
  if (options.model && options.model.provider !== 'claude') throw new Error('Unsupported Claude model provider');
  const executable = resolveClaudeExecutable(options.command ?? process.env.HIVE_CLAUDE_COMMAND ?? 'claude');
  const sessionId = options.sessionId ?? randomUUID();
  let metadata = readClaudeMetadata(options.sessionDir, sessionId);
  let closeTask: Promise<void> | undefined;
  let disposed = false; let ready = false; let failure: Error | undefined;
  let active: Run | undefined;
  let child: ChildProcessWithoutNullStreams | undefined;
  let processClosed!: () => void;
  const exited = new Promise<void>(resolve => { processClosed = resolve; });
  const input = new ClaudeInput();
  const controller = new AbortController();
  const projection = new ClaudeProjection(() => metadata);
  const toolNames = new Map<string, string>();
  let models: ModelInfo[] = [];
  let currentModel = options.model?.id;
  let catalogs = { commands: [] as string[], models: [] as string[], mcpServers: [] as { name: string; status: string }[] };
  const emit = (event: PiEvent) => options.onEvent(event);
  const interactions = new ClaudeInteractions(sessionId, () => active && ({ id: active.uuid, signal: active.controller.signal }), emit);
  const persist = (next: ClaudeMetadata) => { writeClaudeMetadata(options.sessionDir, sessionId, next); metadata = next; };
  const fail = (error: Error) => {
    if (disposed || failure) return;
    failure = error; interactions.clear('disconnected'); active?.controller.abort(); active?.reject(error);
    if (ready) options.onUnexpectedExit?.(error);
  };
  const check = () => { if (disposed || failure) throw failure ?? new Error('Claude session is disposed'); };
  const present = (message: Parameters<typeof projectClaudeMessage>[0], notify: boolean) => {
    for (const item of projection.put(message)) {
      if (item.message.role === 'assistant' && Array.isArray(item.message.content)) for (const block of item.message.content) {
        if (block.type === 'toolCall') toolNames.set(String(block.id), String(block.name));
      }
      if (item.message.role === 'toolResult') item.message.toolName = toolNames.get(String(item.message.toolCallId)) ?? 'Claude tool';
      if (notify) emit({ type: 'message_end', sessionId, messageId: item.id, message: item.message });
    }
  };
  const refresh = async () => {
    const messages = await getSessionMessages(sessionId, { dir: options.cwd });
    projection.clear(); for (const message of messages) present(message, false);
    return messages;
  };
  const unsupported = (kind: string) => {
    const error = new Error(`Claude requires unsupported interaction: ${kind}`);
    emit({ type: 'error', sessionId, message: error.message });
    if (active) { active.error = error; active.controller.abort(); }
    return error;
  };
  const team = createClaudeTeamServer({ ...(options.teamBinding && { binding: options.teamBinding }), signal: () => active?.controller.signal,
    track(execution) { const run = active; if (!run) throw new Error('Claude requested a Hive tool outside an active turn');
      run.tools.add(execution); void execution.then(() => run.tools.delete(execution), () => run.tools.delete(execution)); } });
  const running: Query = query({ prompt: input, options: {
    cwd: options.cwd, pathToClaudeCodeExecutable: executable, abortController: controller,
    ...(options.sessionId ? { resume: options.sessionId } : { sessionId }), ...(options.model && { model: options.model.id }),
    includePartialMessages: true, permissionMode: 'default', mcpServers: { [team.name]: team.server },
    canUseTool: options.canUseTool ?? ((toolName, input, nativeOptions) => interactions.request(toolName, input, nativeOptions)),
    onUserDialog: options.onUserDialog ?? (async request => { unsupported(`user dialog ${request.dialogKind}`); return { behavior: 'cancelled' }; }),
    ...(options.supportedDialogKinds && { supportedDialogKinds: options.supportedDialogKinds }),
    onElicitation: options.onElicitation ?? (async () => { unsupported('MCP elicitation'); return { action: 'cancel' }; }),
    spawnClaudeCodeProcess(spawnOptions) {
      child = spawn(spawnOptions.command, spawnOptions.args, { cwd: spawnOptions.cwd, env: spawnOptions.env, signal: spawnOptions.signal, stdio: ['pipe', 'pipe', 'pipe'], shell: false });
      // Diagnostics stay out of model/UI output; stderr must still be drained.
      child.stderr.resume();
      child.once('error', fail);
      child.once('exit', (code, signal) => fail(new Error(`Claude Code exited (${code ?? signal ?? 'unknown'})`)));
      child.once('close', processClosed);
      return child;
    },
  } });
  const consume = (async () => {
    try {
      for await (const message of running) {
        if (message.session_id !== sessionId) { fail(new Error('Claude changed the native session identity')); break; }
        if ('parent_tool_use_id' in message && message.parent_tool_use_id !== null) continue;
        if (message.type === 'assistant' || message.type === 'user') {
          present(message as SessionMessage, true);
          if (message.type === 'assistant') {
            active?.assistants.add(message.uuid);
            currentModel = message.message.model;
            for (const block of message.message.content) if (block.type === 'tool_use') emit({ type: 'tool_start', sessionId, toolCallId: block.id, toolName: block.name, args: block.input });
          } else if (Array.isArray(message.message.content)) for (const block of message.message.content) if (block.type === 'tool_result') emit({ type: 'tool_end', sessionId, toolCallId: block.tool_use_id, toolName: toolNames.get(block.tool_use_id) ?? 'Claude tool', result: block.content, isError: block.is_error === true });
        } else if (message.type === 'stream_event') {
          const delta = projection.stream(message);
          if (delta) emit({ type: 'assistant_delta', sessionId, ...delta });
        } else if (message.type === 'tool_progress') emit({ type: 'tool_update', sessionId, toolCallId: message.tool_use_id, toolName: message.tool_name, result: { elapsedSeconds: message.elapsed_time_seconds } });
        else if (message.type === 'result' && active) {
          const belongs = message.user_message_uuid === active.uuid || message.user_message_uuids?.includes(active.uuid);
          if (!belongs) active.reject(new Error('Claude terminal result cannot be correlated to the active input'));
          else { interactions.clear(active.cancelled ? 'aborted' : 'turn-ended', active.uuid); active.resolve(message); }
        } else if (message.type === 'system' && message.subtype === 'init') currentModel = message.model;
        else if (message.type === 'system' && message.subtype === 'session_writes_lost') {
          const error = new Error('Claude could not persist the session transcript'); if (active) { active.error = error; active.reject(error); }
        }
      }
      if (!disposed) fail(new Error('Claude SDK output ended unexpectedly'));
    } catch (error) { fail(asError(error)); }
  })();
  const close = (): Promise<void> => {
    if (closeTask) return closeTask;
    disposed = true; interactions.clear('disposed');
    closeTask = (async () => {
      input.close(); running.close();
      if (child) {
        try { await claudeDeadline(exited, 'shutdown', 5000); }
        catch { child.kill('SIGKILL'); await claudeDeadline(exited, 'forced shutdown', 2000); }
      }
      await consume;
    })();
    return closeTask;
  };
  try {
    const init = await claudeDeadline(running.initializationResult(), 'initialization');
    models = await claudeDeadline(running.supportedModels(), 'model catalog');
    const servers = await claudeDeadline(running.mcpServerStatus(), 'MCP handshake');
    if (!servers.some(server => server.name === team.name && server.status === 'connected')) throw new Error('Claude Hive MCP server did not connect');
    catalogs = { commands: init.commands.map(command => command.name), models: models.map(model => model.value), mcpServers: servers.map(server => ({ name: server.name, status: server.status })) };
    await refresh(); check(); ready = true;
  } catch (error) { await close(); throw error; }
  const reasoning = async () => {check();const {modelId,...state} = await readClaudeReasoning(running,models);currentModel=modelId;return state;};
  return {
    sessionId, teamToolName: team.toolName, get processId() { return child?.pid; }, sessionFile: undefined,
    catalog: () => structuredClone(catalogs), history: () => projection.snapshot().history,
    hasContext: customType => projection.snapshot().history.some(message => message.role === 'custom' && message.customType === customType),
    snapshot: () => projection.snapshot(),
    interactions: () => interactions.snapshot(),
    respondInteraction: (id, response) => { check(); interactions.respond(id, response); },
    completedDeliveryIds: () => metadata.completed.filter(entry => projection.hasNativeUuid(entry.userUuid) && projection.hasNativeUuid(entry.assistantUuid)).map(entry => entry.deliveryId),
    async addContext(text, customType = 'hive-context') {
      check(); if (active) throw new Error('Claude session is already processing a prompt');
      const uuid = randomUUID(); persist({ ...metadata, context: [...metadata.context, { uuid, customType, text }] });
      input.push({ type: 'user', uuid, session_id: sessionId, message: { role: 'user', content: text }, parent_tool_use_id: null, shouldQuery: false, client_composed: true });
      await claudeDeadline((async () => {
        while (!(await refresh()).some(message => message.uuid === uuid)) { check(); await new Promise(resolve => setTimeout(resolve, 25)); }
      })(), 'hidden context persistence', 5000);
    },
    prompt(text, promptOptions) {
      let images: ReturnType<typeof promptImages>;
      try { images = promptImages(promptOptions?.images); } catch (error) { return Promise.reject(error); }
      try { check(); } catch (error) { return Promise.reject(error); }
      if (active) return Promise.reject(new Error('Claude session is already processing a prompt'));
      let resolve!: (value: SDKResultMessage) => void; let reject!: (error: Error) => void;
      const terminal = new Promise<SDKResultMessage>((yes, no) => { resolve = yes; reject = no; }); void terminal.catch(() => {});
      const run: Run = { uuid: randomUUID(), cancelled: false, controller: new AbortController(), tools: new Set(), assistants: new Set(), terminal, resolve, reject, done: Promise.resolve() };
      active = run;
      run.done = Promise.resolve().then(async () => {
        try {
          if (run.cancelled) return;
          if (promptOptions?.deliveryId) {
            const presentation = isDeliveryPresentation(promptOptions.presentation) ? promptOptions.presentation : { version: 1 as const, origin: 'unknown' as const, displayText: text };
            persist({ ...metadata, deliveries: [...metadata.deliveries, { uuid: run.uuid, deliveryId: promptOptions.deliveryId, presentation }] });
          }
          const content = images.length ? [
            ...(text ? [{type:'text' as const,text}] : []),
            ...images.map(image=>({type:'image' as const,source:{type:'base64' as const,media_type:image.mimeType,data:image.data}})),
          ] : text;
          input.push({ type: 'user', uuid: run.uuid, session_id: sessionId, message: { role: 'user', content }, parent_tool_use_id: null,
            ...((promptOptions?.automated || promptOptions?.deliveryId) && { client_composed: true }) });
          const result = await terminal;
          await Promise.allSettled([...run.tools]); const durableMessages = await refresh();
          if (run.cancelled) { if (promptOptions?.deliveryId) throw new Error('Claude delivery was aborted'); return; }
          if (run.error) throw run.error;
          if (result.is_error || result.subtype !== 'success' || (result.terminal_reason && result.terminal_reason !== 'completed')) throw new Error('Claude run did not complete successfully');
          if (promptOptions?.deliveryId) {
            const assistantUuid = claudeCompletionAssistant(durableMessages, run.uuid, run.assistants, result);
            persist({ ...metadata, completed: [...metadata.completed, { deliveryId: promptOptions.deliveryId, userUuid: run.uuid, assistantUuid, resultUuid: result.uuid }] });
          }
        } catch (error) { if (!run.cancelled) emit({ type: 'error', sessionId, message: asError(error).message }); throw error; }
        finally {
          interactions.clear(run.cancelled ? 'aborted' : 'turn-ended', run.uuid);
          run.controller.abort(); await Promise.allSettled([...run.tools]);
          active = undefined; projection.endStream(); emit({ type: 'settled', sessionId, aborted: run.cancelled });
        }
      });
      return run.done;
    },
    async abort() {
      const run = active; if (!run) return;
      run.cancelled = true; interactions.clear('aborted', run.uuid); run.controller.abort();
      try { await claudeDeadline(running.interrupt(), 'interrupt'); }
      catch (error) { fail(asError(error)); running.close(); throw error; }
      finally { await run.done.catch(() => {}); }
    },
    async dispose() { if (active) throw new Error('Abort and await the active Claude prompt before disposal'); await close(); },
    currentModel: () => currentModel ? { provider: 'claude', id: currentModel, name: models.find(model => model.value === currentModel || model.resolvedModel === currentModel)?.displayName ?? currentModel } : undefined,
    listModels: () => models.map(model => ({ provider: 'claude', id: model.value, name: model.displayName, available: true })),
    reasoning,
    async setReasoning(level) {
      check();if(active) throw new Error('Claude session is already processing a prompt');
      if (!(await reasoning()).levels.includes(level)) throw new TypeError('当前 Claude 模型不支持此推理强度。');
      check();if(active) throw new Error('Claude session is already processing a prompt');
      await claudeDeadline(running.applyFlagSettings({effortLevel:level as EffortLevel}),'set reasoning');
      return reasoning();
    },
    async setModel(provider, id) { check(); if (active) throw new Error('Claude session is already processing a prompt'); if (provider !== 'claude') throw new Error('Unsupported Claude model provider'); await claudeDeadline(running.setModel(id), 'set model'); currentModel = id; },
  };
}
export function resolveClaudeExecutable(command: string): string {
  if (process.platform === 'win32' && ['.cmd', '.bat', '.ps1'].includes(extname(command).toLowerCase())) throw new Error('Configure the native Claude Code .exe; shell launchers are not supported');
  const paths = isAbsolute(command) || command.includes('/') || command.includes('\\') ? [resolve(command)]
    : (process.env.PATH ?? '').split(delimiter).flatMap(directory => {
      const path = join(directory, command);
      return process.platform === 'win32' && !extname(command) ? [path, `${path}.exe`] : [path];
    });
  for (const path of paths) { try { accessSync(path, constants.X_OK); return path; } catch (error) { if (!['ENOENT', 'EACCES', 'ENOTDIR'].includes((error as NodeJS.ErrnoException).code ?? '')) throw error; } }
  throw new Error('Original Claude Code executable was not found; configure HIVE_CLAUDE_COMMAND');
}
function asError(value: unknown): Error { return value instanceof Error ? value : new Error(String(value)); }

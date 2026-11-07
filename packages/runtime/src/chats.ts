import { restoreHistoricalReport } from './delivery-presentation.js';
import type { AgentModels } from './agent-models.js';
import { WorkspaceRemovalError, workspaceRemovalBlocked, type WorkspaceRemovalStage } from './workspace-removal.js';
import { randomUUID } from 'node:crypto';
import { existsSync, mkdirSync } from 'node:fs';
import { join } from 'node:path';
import { createClaudeSession, type ClaudeSessionHandle } from './agents/claude.js';
import { createCodexSession, type CodexSessionHandle } from './agents/codex.js';
import { createExternalPiSession, type ExternalPiSessionHandle } from './agents/external-pi.js';
import { createPiSession } from './agents/pi.js';
import type { PiEvent, PiSessionHandle } from './agents/types.js';
import { ChatIndex, type ChatRecord, type ChatBackend } from './chat-index.js';
import type { DesktopSettings } from './desktop-settings.js';
import { CodexInteractionError } from './agents/codex-interactions.js';
import { DeliveryQueue } from './delivery-queue.js';
import type { HiveHost } from './hive-host.js';
import type { HiveAgentRuntime, WorkspaceSummary } from './hive-runtime-contract.js';
import type { StructuredSessionPort, StructuredSessionHandle } from './structured-runtime.js';
import { createTeamTool, createTeamCommand } from './team-tool.js';

interface LiveChat {
  handle: PiSessionHandle; queue: DeliveryQueue; aborted: boolean;
  failure?: Error; cleanup?: Promise<void>; onExit?: (error?: unknown) => void;
}
type QueueState = { type: 'state'; sessionId: string; busy: boolean; pending: number; pendingReports: DeliveryQueue['pendingReports']; completedReportId?: string };
export type ChatEvent = (PiEvent | QueueState) & { sequence: number };
const toolGuidance = '\nDesktop integration: use the registered team tool for every Hive command, passing CLI arguments as an array. Use stdin only when the command explicitly supports --stdin; team send takes task text as a positional argument. Do not run team through bash: this process serves multiple agents and bash has no agent-specific Hive credentials. Omit --cli when spawning to inherit the current backend and model. A normal assistant reply does not report or settle a dispatch. Follow Hive seen/report/accept rules.';

export class Chats implements StructuredSessionPort {
  private readonly index: ChatIndex;
  private readonly live = new Map<string, LiveChat>();
  private readonly opening = new Map<string, Promise<LiveChat>>();
  private readonly sequences = new Map<string, number>();
  private readonly activating = new Map<string, Promise<void>>();
  private readonly sessionDir: string;
  private readonly queueDir: string;
  private closing = false;
  private readonly removing = new Set<string>();

  constructor(private readonly hive: HiveHost, dataDir: string, private readonly emit: (event: ChatEvent) => void,
    private readonly runtime: HiveAgentRuntime, private readonly settings: DesktopSettings, private readonly agentModels: AgentModels) {
    const directory = join(dataDir, 'chats');
    this.sessionDir = join(directory, 'pi');
    this.queueDir = join(directory, 'deliveries');
    mkdirSync(this.sessionDir, { recursive: true });
    this.index = new ChatIndex(directory);
  }
  assertWorkspaceAvailable(workspaceId: string): void {
    if (this.removing.has(workspaceId)) throw workspaceRemovalBlocked(workspaceId);
  }
  list(workspaceId: string) { return this.index.list(workspaceId); }
  async search(query: unknown) {
    if (typeof query !== 'string' || Array.from(query).length > 200) {
      throw new TypeError('搜索内容需为 200 个字符以内的文本。');
    }
    const terms = query.trim().toLocaleLowerCase().split(/\s+/u).filter(Boolean);
    const workspaces = await this.hive.request('GET', '/api/workspaces') as WorkspaceSummary[];
    const groups = await Promise.all(workspaces.filter(workspace => !this.removing.has(workspace.id)).map(async workspace => {
      if (!this.index.list(workspace.id).length) return [];
      const members = await this.hive.request('GET', `/api/ui/workspaces/${workspace.id}/team`) as { id: string; name: string }[];
      const names = new Map(members.map(member => [member.id, member.name]));
      return this.index.list(workspace.id).map(session => ({
        session,
        workspaceName: workspace.name,
        title: !session.customTitle && session.agentId ? names.get(session.agentId) ?? session.title : session.title,
      }));
    }));
    const matches = groups.flat().filter(hit => {
      const backendName = { pi: '内置 Pi', codex: 'Codex', 'pi-rpc': '外部 Pi', claude: 'Claude Code' }[hit.session.backend];
      const searchable = [hit.title, hit.session.title, hit.workspaceName, hit.session.backend, backendName].join(' ').toLocaleLowerCase();
      return terms.every(term => searchable.includes(term));
    }).sort((a, b) => b.session.updatedAt - a.session.updatedAt || a.session.id.localeCompare(b.session.id));
    return { matches: matches.slice(0, 100), total: matches.length };
  }
  rename(id: string, title: unknown): ChatRecord {
    if (this.closing) throw new Error('Chat runtime is closing');
    const current = this.index.get(id);
    this.assertWorkspaceAvailable(current.workspaceId);
    if (typeof title !== 'string') throw new TypeError('Conversation name must be text');
    const name = title.trim();
    if (!name || Array.from(name).length > 120 || /[\u0000-\u001f\u007f-\u009f]/u.test(name)) {
      throw new RangeError('会话名称需为 1–120 个字符，且不能包含换行或控制字符。');
    }
    const renamed = { ...current, title: name, customTitle: true };
    this.index.save(renamed);
    return renamed;
  }
  private event(id: string, event: PiEvent | QueueState) {
    const sequence = (this.sequences.get(id) ?? 0) + 1;
    this.sequences.set(id, sequence);
    this.emit({ ...event, sessionId: id, sequence });
  }
  private async workspace(id: string): Promise<WorkspaceSummary> {
    const workspaces = await this.hive.request('GET', '/api/workspaces') as WorkspaceSummary[];
    const workspace = workspaces.find(workspace => workspace.id === id);
    if (!workspace) throw new Error('Workspace not found');
    return workspace;
  }
  private async load(record: ChatRecord): Promise<LiveChat> {
    const workspace = await this.workspace(record.workspaceId);
    const sessionFile = record.sessionFile;
    if (sessionFile && !existsSync(sessionFile)) {
      throw new Error(`找不到已保存的 Pi 会话文件。恢复文件后可重新打开会话：${sessionFile}`);
    }
    let chat: LiveChat | undefined;
    let earlyFailure: Error | undefined;
    const binding = () => {
      const bound = this.index.get(record.id);
      const agentId = bound.agentId;
      const token = agentId && this.runtime.peekAgentToken(agentId);
      if (!agentId || !token) throw new Error('This conversation is not an active team member');
      return { workspaceId: bound.workspaceId, agentId, token, hivePort: String(this.hive.port), cwd: workspace.path };
    };
    const onEvent = (event: PiEvent) => {
      if (event.type === 'settled') { if (chat) chat.aborted = event.aborted; return; }
      this.event(record.id, event);
    };
    const onUnexpectedExit = (error: Error) => {
      if (chat) this.failLive(record.id, chat, error);
      else earlyFailure = error;
    };
    const model = record.model && { provider: record.model.provider, id: record.model.id };
    const command = createTeamCommand(binding);
    const executables = record.backend === 'pi' ? undefined : (await this.settings.get()).effective;
    const handle = record.backend === 'codex'
      ? await createCodexSession({ cwd: workspace.path, sessionDir: join(this.sessionDir, '..', 'codex'),
        command: executables!.codex.command,
        ...(record.codexThreadId && { threadId: record.codexThreadId }), ...(model && { model }), onEvent,
        onUnexpectedExit,
        onMaterialized: threadId => this.index.save({ ...this.index.get(record.id), codexThreadId: threadId }),
        customTools: [{ name: command.name, description: command.description, parameters: command.parameters,
          execute: (id, args, signal) => command.execute(id, args as { args: string[]; stdin?: string }, signal) }],
      })
      : record.backend === 'claude'
      ? await createClaudeSession({ cwd: workspace.path, sessionDir: join(this.sessionDir, '..', 'claude'),
        command: executables!.claude.command, ...(record.nativeclaudeSessionId && { sessionId: record.nativeclaudeSessionId }),
        ...(model && { model }), onEvent, onUnexpectedExit, teamBinding: binding })
      : record.backend === 'pi-rpc'
      ? await createExternalPiSession({ cwd: workspace.path, sessionDir: join(this.sessionDir, '..', 'external-pi'),
        command: executables!.pi.command,
        ...(sessionFile && { sessionFile }), ...(model && { model }), onEvent, onUnexpectedExit, teamBinding: binding })
      : await createPiSession({ cwd: workspace.path, sessionDir: this.sessionDir,
        ...(sessionFile && { sessionFile }), ...(model && { model }),
        customTools: [createTeamTool(binding)], onEvent });
    try {
      if (earlyFailure) throw earlyFailure;
      if (!handle.hasContext('hive-context') && record.startupContext) await handle.addContext(record.startupContext);
      if (!handle.hasContext('hive-desktop')) {
        await handle.addContext(toolGuidance, 'hive-desktop');
      }
      if (record.backend === 'pi-rpc') {
        await handle.addContext(`Current external Pi integration: use the registered ${(handle as ExternalPiSessionHandle).teamToolName} tool for every Hive team command. This is the current process's team tool; older process tool names no longer exist. Pass CLI arguments as an array. Use stdin only when the command explicitly supports --stdin; team send takes task text as a positional argument. Omit --cli to inherit this backend and model. Never invoke team through bash.`, 'hive-desktop-external');
      }
      if (record.backend === 'claude') {
        await handle.addContext(`Current Claude integration: use the registered ${(handle as ClaudeSessionHandle).teamToolName} MCP tool for every Hive team command. Older process tool names no longer exist. Pass CLI arguments as an array; team send takes task text as a positional argument. Use stdin only when the command supports --stdin. Omit --cli to inherit this backend and model. Never invoke team through bash. Native tool permissions and AskUserQuestion are presented in the desktop chat; wait for the user's response and do not bypass approval. Other native user dialogs and MCP elicitation are not supported.`, 'hive-desktop-claude');
      }
      const model = handle.currentModel();
      this.index.save({ ...this.index.get(record.id), ...(record.backend === 'codex'
        ? (handle as CodexSessionHandle).materialized ? { codexThreadId: handle.sessionId } : {}
        : record.backend === 'claude' ? { sessionFile: null, nativeclaudeSessionId: handle.sessionId }
        : { sessionFile: handle.sessionFile ?? null, ...(record.backend === 'pi-rpc'
          ? { externalPiSessionId: handle.sessionId } : { piSessionId: handle.sessionId }) }), ...(model ? { model } : {}) });
      let wasBusy = false;
      const queue = new DeliveryQueue({
        directory: this.queueDir, sessionId: record.id, startPaused: true,
        completedIds: () => handle.completedDeliveryIds(),
        execute: item => item.triggerTurn ? handle.prompt(item.text, { automated: true, deliveryId: item.id, ...(item.presentation && { presentation: item.presentation }) }) : handle.addContext(item.text),
        onError: error => { if (!chat?.aborted) this.event(record.id, { type: 'error', sessionId: record.id, message: error.message }); },
        onState: (busy, completedReportId) => {
          this.event(record.id, { type: 'state', sessionId: record.id, busy, pending: queue.pendingCount, pendingReports: queue.pendingReports, ...(completedReportId && { completedReportId }) });
          if (wasBusy && !busy) this.event(record.id, { type: 'settled', sessionId: record.id, aborted: chat?.aborted ?? false });
          wasBusy = busy;
        },
      });
      if (earlyFailure) throw earlyFailure;
      chat = { handle, queue, aborted: false };
      this.live.set(record.id, chat);
      return chat;
    } catch (error) { await handle.dispose(); throw error; }
  }
  private snapshot(id: string, chat: LiveChat) {
    const record = this.index.get(id);
    const snapshot = chat.handle.snapshot();
    const model = chat.handle.currentModel();
    const agentId = record.agentId;
    const history = agentId ? snapshot.history.map(message => restoreHistoricalReport(message,
      (dispatchId, payload) => this.hive.findReportPresentation(record.workspaceId, agentId, dispatchId, payload))) : snapshot.history;
    return {
      ...record, ...(record.backend === 'pi' && { model, unavailableModel: model ? undefined : record.model }), ...snapshot, history, interactions: chat.handle.interactions?.() ?? [], models: chat.handle.listModels(),
      busy: chat.queue.busy, pending: chat.queue.pendingCount, pendingReports: chat.queue.pendingReports,
      completedDeliveryIds: chat.handle.completedDeliveryIds(),
      runId: record.agentId ? this.runtime.getActiveRunByAgentId(record.workspaceId, record.agentId)?.runId ?? null : null,
      eventSequence: this.sequences.get(id) ?? 0,
    };
  }
  private async require(id: string): Promise<LiveChat> {
    if (this.closing) throw new Error('Chat runtime is closing');
    this.assertWorkspaceAvailable(this.index.get(id).workspaceId);
    const existing = this.live.get(id);
    if (existing?.cleanup) {
      await existing.cleanup;
      return this.require(id);
    }
    if (existing) return existing;
    const pending = this.opening.get(id);
    if (pending) return pending;
    const opening = this.load(this.index.get(id)).finally(() => this.opening.delete(id));
    this.opening.set(id, opening);
    return opening;
  }
  async createRecord(workspaceId: string, backend: ChatBackend = 'pi') {
    this.assertWorkspaceAvailable(workspaceId);
    if (this.closing) throw new Error('Chat runtime is closing');
    await this.workspace(workspaceId);
    this.assertWorkspaceAvailable(workspaceId);
    if (this.closing) throw new Error('Chat runtime is closing');
    const record: ChatRecord = { id: randomUUID(), workspaceId, backend, title: '新会话', sessionFile: null, updatedAt: Date.now() };
    this.index.save(record);
    return record;
  }
  async create(workspaceId: string, backend: ChatBackend = 'pi') {
    const record = await this.createRecord(workspaceId, backend);
    return this.open(record.id);
  }
  async open(id: string) { return this.snapshot(id, await this.require(id)); }

  /** Hive invokes this for both the selected orchestrator and spawned workers. */
  async start(input: Parameters<StructuredSessionPort['start']>[0]): Promise<StructuredSessionHandle> {
    this.assertWorkspaceAvailable(input.workspace.id);
    const backend = input.config.command === 'hive:codex' ? 'codex' : input.config.command === 'hive:pi-rpc' ? 'pi-rpc' : input.config.command === 'hive:claude' ? 'claude' : 'pi';
    const args = input.config.args ?? [];
    const provider = args[args.indexOf('--provider') + 1];
    const modelId = args[args.indexOf('--model') + 1];
    const explicitModel = args.includes('--provider') || args.includes('--model');
    if (explicitModel && (!args.includes('--provider') || !args.includes('--model') || !provider || !modelId)) {
      throw new Error('Structured launch requires both --provider and --model values');
    }
    let record = this.index.forAgent(input.workspace.id, input.agentId);
    if (record && record.backend !== backend) {
      throw new Error(`This member is bound to a ${record.backend} conversation. Create and select a new ${backend} conversation before switching backend; the existing transcript is preserved.`);
    }
    if (!record) {
      const members = await this.hive.request('GET', `/api/ui/workspaces/${input.workspace.id}/team`) as { id: string; name: string }[];
      this.assertWorkspaceAvailable(input.workspace.id);
      record = { id: randomUUID(), workspaceId: input.workspace.id, agentId: input.agentId, backend,
        title: input.agentId.endsWith(':orchestrator') ? '主会话' : members.find(member => member.id === input.agentId)?.name ?? '成员会话', sessionFile: null, updatedAt: Date.now(),
        ...(args.includes('--provider') && args.includes('--model') && provider && modelId ? { model: { provider, id: modelId, name: modelId } } : {}),
      };
      this.index.save(record);
    }
    const id = record.id;
    const chat = await this.require(id);
    if (chat.failure) throw chat.failure;
    if (explicitModel) {
      const current = chat.handle.currentModel();
      if (!current || current.provider !== provider || current.id !== modelId) {
        if (chat.queue.busy) throw new Error('Stop the current response before changing the launch model');
        await chat.handle.setModel(provider!, modelId!);
        const model = chat.handle.currentModel();
        if (!model || model.provider !== provider || model.id !== modelId) throw new Error('Backend did not select the requested launch model');
        this.index.save({ ...this.index.get(id), model });
      }
    }
    chat.onExit = input.onExit;
    // Backend context determines startup recovery. Display history may omit hidden
    // context, and an empty native session may not have a durable rollout yet.
    return {
      sessionId: chat.handle.sessionId,
      resumed: chat.handle.hasContext('hive-context'),
      ...(chat.handle.sessionFile ? { sessionFile: chat.handle.sessionFile } : {}),
      deliver: async (text, options) => {
        if (options?.triggerTurn === false) {
          const startupContext = text;
          this.index.save({ ...this.index.get(id), startupContext });
          await chat.handle.addContext(startupContext);
        } else await chat.queue.enqueue(text, true, options?.presentation);
      },
      ready: () => chat.queue.resume(),
      stop: () => this.stopLive(id, chat),
    };
  }
  private launchConfig(record: ChatRecord) {
    return { command: record.backend === 'codex' ? 'hive:codex' : record.backend === 'pi-rpc' ? 'hive:pi-rpc' : record.backend === 'claude' ? 'hive:claude' : 'hive:pi', commandPresetId: null,
      args: record.model ? ['--provider', record.model.provider, '--model', record.model.id] : [] };
  }
  private async withWorkspace<T>(workspaceId: string, operation: () => Promise<T>): Promise<T> {
    const previous = this.activating.get(workspaceId) ?? Promise.resolve();
    const action = previous.then(operation, operation);
    const barrier = action.then(() => {}, () => {});
    this.activating.set(workspaceId, barrier);
    try { return await action; }
    finally { if (this.activating.get(workspaceId) === barrier) this.activating.delete(workspaceId); }
  }
  private async activateOnce(id: string) {
    const record = this.index.get(id);
    this.assertWorkspaceAvailable(record.workspaceId);
    const agentId = record.agentId ?? `${record.workspaceId}:orchestrator`;
    const previous = this.index.forAgent(record.workspaceId, agentId);
    if (previous && previous.id !== id) {
      const team = await this.hive.request('GET', `/api/ui/workspaces/${record.workspaceId}/team`) as { pending_task_count: number; open_dispatches?: unknown[] }[];
      const queue = this.live.get(previous.id)?.queue;
      const pending = queue?.pendingCount ?? DeliveryQueue.persistedCount(this.queueDir, previous.id);
      if (queue?.busy || pending || team.some(member => member.pending_task_count > 0 || member.open_dispatches?.length)) {
        throw new Error('当前主会话仍有执行或未结派单，请先完成或停止相关任务，再切换主会话。');
      }
    }
    const active = this.runtime.getActiveRunByAgentId(record.workspaceId, agentId);
    if (previous?.id !== id && active) {
      this.runtime.stopAgentRun(active.runId);
      if (!await this.runtime.waitForRunExit(active.runId, 30_000)) throw new Error('Previous team session is still stopping');
    }
    this.assertWorkspaceAvailable(record.workspaceId);
    this.index.bind(id, agentId);
    this.runtime.configureAgentLaunch(record.workspaceId, agentId, this.launchConfig(this.index.get(id)));
    // The Hive route performs its normal agent status transition and outbox replay.
    await this.hive.request('POST', `/api/workspaces/${record.workspaceId}/agents/${encodeURIComponent(agentId)}/start`);
  }
  async prompt(id: string, text: string) {
    this.assertWorkspaceAvailable(this.index.get(id).workspaceId);
    if (!text.trim()) throw new Error('Message cannot be empty');
    return this.withWorkspace(this.index.get(id).workspaceId, async () => {
      const opened = await this.require(id);
      if (this.index.get(id).backend === 'pi' && !opened.handle.currentModel()) {
        throw new Error('请先为内置 Pi 选择可用模型，再发送消息。');
      }
      await this.activateOnce(id);
      const chat = await this.require(id);
      if (!chat.queue.pendingCount) chat.queue.resume();
      const record = this.index.get(id);
      this.index.save({ ...record, title: !record.customTitle && record.title === '新会话' ? text.trim().slice(0, 60) : record.title, updatedAt: Date.now() });
      if (record.agentId === `${record.workspaceId}:orchestrator`) {
        await this.hive.request('POST', `/api/workspaces/${record.workspaceId}/user-input`, { text });
      } else {
        await this.runtime.deliverSystemMessageToAgent(record.workspaceId, record.agentId!, text, {
          requireActiveRun: true, presentation: { version: 1, origin: 'user', displayText: text },
        });
      }
      return { accepted: true, session: this.index.get(id) };
    });
  }
  async respondInteraction(id: string, interactionId: string, response: unknown) {
    const chat = this.live.get(id);
    if (!chat || chat.failure || chat.cleanup || !chat.handle.respondInteraction) {
      throw new CodexInteractionError('INTERACTION_STALE', 'This conversation has no live native interaction');
    }
    this.assertWorkspaceAvailable(this.index.get(id).workspaceId);
    await chat.handle.respondInteraction(interactionId, response);
    return { accepted: true };
  }
  async abort(id: string) {
    const chat = this.removing.has(this.index.get(id).workspaceId) ? this.live.get(id) : await this.require(id);
    if (!chat) throw workspaceRemovalBlocked(this.index.get(id).workspaceId);
    const paused = chat.queue.pause();
    await chat.handle.abort();
    await paused;
    chat.queue.discardCurrent();
    // Explicit cancel affects this response only. Pending team deliveries stay
    // durable and require an explicit resume, never become successful reports.
    return { ok: true };
  }
  async resume(id: string) {
    this.assertWorkspaceAvailable(this.index.get(id).workspaceId);
    return this.withWorkspace(this.index.get(id).workspaceId, async () => {
      await this.activateOnce(id);
      (await this.require(id)).queue.resume();
      return { ok: true };
    });
  }
  async refreshPiModels(id: string) {
    const record = this.index.get(id);
    this.assertWorkspaceAvailable(record.workspaceId);
    return this.withWorkspace(record.workspaceId, async () => {
      const chat = await this.require(id);
      if (record.backend !== 'pi' || !chat.handle.refreshModels) throw new Error('Only built-in Pi supports reloading its local model configuration');
      if (chat.queue.busy) throw new Error('请在当前回复结束后刷新模型');
      await chat.handle.refreshModels();
      return chat.handle.listModels();
    });
  }
  async setModel(id: string, provider: string, modelId: string) {
    this.assertWorkspaceAvailable(this.index.get(id).workspaceId);
    return this.withWorkspace(this.index.get(id).workspaceId, async () => {
      const chat = await this.require(id);
      if (chat.queue.busy) throw new Error('Stop the current response before changing model');
      await chat.handle.setModel(provider, modelId);
      const model = chat.handle.currentModel();
      const record = { ...this.index.get(id), ...(model ? { model } : {}), updatedAt: Date.now() };
      this.index.save(record);
      if (record.agentId) this.runtime.configureAgentLaunch(record.workspaceId, record.agentId, this.launchConfig(record));
      return { ok: true };
    });
  }
  async removeWorkspace(workspaceId: string, drainWorkspaceTerminals: () => Promise<void>): Promise<{ ok: true; removedSessionIds: string[] }> {
    if (typeof workspaceId !== 'string' || !workspaceId.trim()) throw new Error('workspaceId is required');
    if (this.closing) throw new Error('Chat runtime is closing');
    this.assertWorkspaceAvailable(workspaceId);
    this.removing.add(workspaceId);
    const progress: { stage: WorkspaceRemovalStage; deleted: boolean } = { stage: 'prepare', deleted: false };
    let sessionIds: string[] = [];
    try {
      return await this.hive.withWorkspaceWorkflowIdle(workspaceId, () => this.agentModels.withWorkspaceRemoval(workspaceId, () => this.withWorkspace(workspaceId, async () => {
        await this.workspace(workspaceId);
        await drainWorkspaceTerminals();
        const path = `/api/workspaces/${encodeURIComponent(workspaceId)}`;
        const records = this.index.list(workspaceId);
        const openings = records.flatMap(record => { const opening = this.opening.get(record.id); return opening ? [opening] : []; });
        const opened = await Promise.allSettled(openings);
        // A failed opener has already awaited its own disposal; existing live cleanup must still be awaited below.
        if (opened.some(result => result.status === 'rejected')) throw new Error('工作区会话未正常打开，无法确认其清理完成。');
        sessionIds = this.index.list(workspaceId).map(record => record.id);
        progress.stage = 'stop';
        const listed = await this.hive.request('GET', `/api/ui/workspaces/${encodeURIComponent(workspaceId)}/runs`) as { run_id: string; agent_id: string }[];
        if (!Array.isArray(listed) || !listed.every(run => typeof run.run_id === 'string' && typeof run.agent_id === 'string')) throw new Error('Invalid workspace run list');
        const runs = new Set<string>();
        for (const entry of listed) {
          const active = this.runtime.getActiveRunByAgentId(workspaceId, entry.agent_id);
          if (active?.runId === entry.run_id) runs.add(active.runId); // Shells are owned by Hive's DELETE, not the agent runtime.
        }
        for (const record of this.index.list(workspaceId)) {
          const active = record.agentId && this.runtime.getActiveRunByAgentId(workspaceId, record.agentId);
          if (active) runs.add(active.runId);
        }
        for (const runId of runs) this.runtime.stopAgentRun(runId);
        const stopped = await Promise.all([...runs].map(runId => this.runtime.waitForRunExit(runId, 30_000)));
        if (stopped.some(exited => !exited)) throw new Error('未能确认工作区中的所有成员运行已经退出。');
        const cleanup = await Promise.allSettled(sessionIds.flatMap(id => { const chat = this.live.get(id); return chat ? [this.stopLive(id, chat)] : []; }));
        const errors = cleanup.flatMap(result => result.status === 'rejected' ? [result.reason] : []);
        if (errors.length) throw new AggregateError(errors, '工作区会话清理失败。');
        progress.stage = 'delete';
        await this.hive.request('DELETE', path);
        progress.deleted = true; progress.stage = 'index';
        this.index.removeWorkspace(workspaceId);
        for (const id of sessionIds) this.sequences.delete(id);
        // Native transcripts and physical delivery spools are retained; removed chats are no longer navigable.
        return { ok: true as const, removedSessionIds: sessionIds };
      })));
    } catch (error) {
      if (error instanceof WorkspaceRemovalError) throw error;
      if (error && typeof error === 'object' && 'code' in error && error.code === 'WORKSPACE_WORKFLOW_BUSY') {
        throw new WorkspaceRemovalError('WORKSPACE_REMOVAL_BLOCKED', '工作区的工作流仍在启动、运行或清理中；请等待其实际执行与清理结束后再移除。',
          { workspaceId, workspaceRemoved: false, removedSessionIds: [], stage: 'prepare', localIndexCleaned: false });
      }
      let workspaceRemoved: boolean | null = progress.deleted;
      if (progress.stage === 'delete' && !progress.deleted) {
        try {
          const workspaces = await this.hive.request('GET', '/api/workspaces') as WorkspaceSummary[];
          if (!Array.isArray(workspaces)) throw new Error('Invalid workspace list');
          workspaceRemoved = !workspaces.some(workspace => workspace.id === workspaceId);
        } catch { workspaceRemoved = null; }
      }
      const detail = error instanceof Error ? error.message : String(error);
      const message = workspaceRemoved === true ? `工作区已移除，但桌面记录或后续清理未完成：${detail}`
        : workspaceRemoved === null ? `移除失败，暂时无法确认工作区是否仍存在；请刷新工作区列表：${detail}`
        : `工作区尚未移除，部分会话可能已停止；原有会话记录和待投递数据保留：${detail}`;
      throw new WorkspaceRemovalError(workspaceRemoved === true ? 'WORKSPACE_REMOVAL_PARTIAL' : 'WORKSPACE_REMOVAL_FAILED', message,
        { workspaceId, workspaceRemoved, removedSessionIds: workspaceRemoved === true ? sessionIds : [], stage: progress.stage, localIndexCleaned: false }, { cause: error });
    } finally { this.removing.delete(workspaceId); }
  }
  private failLive(id: string, chat: LiveChat, error: Error): void {
    if (chat.cleanup) return;
    chat.failure = error;
    this.event(id, { type: 'error', sessionId: id, message: error.message });
    const paused = chat.queue.pause();
    chat.cleanup = (async () => {
      const cleanup = await Promise.allSettled([chat.handle.abort(), paused]);
      try { await chat.handle.dispose(); }
      finally {
        if (this.live.get(id) === chat) this.live.delete(id);
        chat.onExit?.(error);
      }
      const failures = cleanup.filter(result => result.status === 'rejected').map(result => result.reason);
      if (failures.length) throw new AggregateError(failures, 'Backend exit cleanup failed');
    })();
    void chat.cleanup.catch(cleanupError => this.event(id, { type: 'error', sessionId: id,
      message: cleanupError instanceof Error ? cleanupError.message : String(cleanupError) }));
  }
  private async stopLive(id: string, chat: LiveChat) {
    if (chat.cleanup) return chat.cleanup;
    const paused = chat.queue.pause();
    await chat.handle.abort();
    await paused;
    await chat.handle.dispose();
    this.live.delete(id);
  }
  async close() {
    this.closing = true;
    await Promise.allSettled(this.opening.values());
    await Promise.all([...this.live.entries()].map(([id, chat]) => this.stopLive(id, chat)));
  }
}

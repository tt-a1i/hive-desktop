import { randomUUID } from 'node:crypto';
import { existsSync, mkdirSync } from 'node:fs';
import { join } from 'node:path';
import { createPiSession } from './agents/pi.js';
import type { PiEvent, PiSessionHandle } from './agents/types.js';
import { ChatIndex, type ChatRecord } from './chat-index.js';
import { DeliveryQueue } from './delivery-queue.js';
import type { HiveHost } from './hive-host.js';
import type { HiveAgentRuntime, WorkspaceSummary } from './hive-runtime-contract.js';
import type { StructuredSessionPort, StructuredSessionHandle } from './structured-runtime.js';
import { createTeamTool } from './team-tool.js';

interface LiveChat { handle: PiSessionHandle; queue: DeliveryQueue; aborted: boolean }
export type ChatEvent = (PiEvent | { type: 'state'; sessionId: string; busy: boolean; pending: number }) & { sequence: number };
const toolGuidance = '\nDesktop integration: use the registered team tool for every Hive command, passing CLI arguments as an array and long input through stdin. Do not run team through bash: this process serves multiple agents and bash has no agent-specific Hive credentials. Omit --cli when spawning to inherit the current backend and model. A normal assistant reply does not report or settle a dispatch. Follow Hive seen/report/accept rules.';

export class Chats implements StructuredSessionPort {
  private readonly index: ChatIndex;
  private readonly live = new Map<string, LiveChat>();
  private readonly opening = new Map<string, Promise<LiveChat>>();
  private readonly sequences = new Map<string, number>();
  private readonly activating = new Map<string, Promise<void>>();
  private readonly sessionDir: string;
  private readonly queueDir: string;
  private closing = false;

  constructor(private readonly hive: HiveHost, dataDir: string, private readonly emit: (event: ChatEvent) => void,
    private readonly runtime: HiveAgentRuntime) {
    const directory = join(dataDir, 'chats');
    this.sessionDir = join(directory, 'pi');
    this.queueDir = join(directory, 'deliveries');
    mkdirSync(this.sessionDir, { recursive: true });
    this.index = new ChatIndex(directory);
  }
  list(workspaceId: string) { return this.index.list(workspaceId); }
  private event(id: string, event: PiEvent | { type: 'state'; sessionId: string; busy: boolean; pending: number }) {
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
    const sessionFile = record.sessionFile && existsSync(record.sessionFile) ? record.sessionFile : undefined;
    let chat: LiveChat | undefined;
    const handle = await createPiSession({
      cwd: workspace.path, sessionDir: this.sessionDir,
      ...(sessionFile ? { sessionFile } : {}),
      ...(record.model ? { model: { provider: record.model.provider, id: record.model.id } } : {}),
      customTools: [createTeamTool(() => {
        const bound = this.index.get(record.id);
        const agentId = bound.agentId;
        const token = agentId && this.runtime.peekAgentToken(agentId);
        if (!agentId || !token) throw new Error('This conversation is not an active team member');
        return { workspaceId: bound.workspaceId, agentId, token, hivePort: String(this.hive.port), cwd: workspace.path };
      })],
      onEvent: event => {
        if (event.type === 'settled') { if (chat) chat.aborted = event.aborted; return; }
        this.event(record.id, event);
      },
    });
    try {
      if (!sessionFile && record.startupContext) await handle.addContext(record.startupContext);
      if (!handle.history().some(message => !!message && typeof message === 'object' && 'customType' in message && message.customType === 'hive-desktop')) {
        await handle.addContext(toolGuidance, 'hive-desktop');
      }
      const model = handle.currentModel();
      this.index.save({ ...record, sessionFile: handle.sessionFile ?? null, piSessionId: handle.sessionId, ...(model ? { model } : {}) });
      let wasBusy = false;
      const queue = new DeliveryQueue({
        directory: this.queueDir, sessionId: record.id, startPaused: true,
        completedIds: () => handle.completedDeliveryIds(),
        execute: item => item.triggerTurn ? handle.prompt(item.text, { automated: true, deliveryId: item.id, ...(item.presentation && { presentation: item.presentation }) }) : handle.addContext(item.text),
        onError: error => { if (!chat?.aborted) this.event(record.id, { type: 'error', sessionId: record.id, message: error.message }); },
        onState: busy => {
          this.event(record.id, { type: 'state', sessionId: record.id, busy, pending: queue.pending.length });
          if (wasBusy && !busy) this.event(record.id, { type: 'settled', sessionId: record.id, aborted: chat?.aborted ?? false });
          wasBusy = busy;
        },
      });
      chat = { handle, queue, aborted: false };
      this.live.set(record.id, chat);
      return chat;
    } catch (error) { handle.dispose(); throw error; }
  }
  private snapshot(id: string, chat: LiveChat) {
    const record = this.index.get(id);
    return {
      ...record, ...chat.handle.snapshot(), models: chat.handle.listModels(),
      busy: chat.queue.busy, pending: chat.queue.pending.length,
      runId: record.agentId ? this.runtime.getActiveRunByAgentId(record.workspaceId, record.agentId)?.runId ?? null : null,
      eventSequence: this.sequences.get(id) ?? 0,
    };
  }
  private async require(id: string): Promise<LiveChat> {
    if (this.closing) throw new Error('Chat runtime is closing');
    const existing = this.live.get(id);
    if (existing) return existing;
    const pending = this.opening.get(id);
    if (pending) return pending;
    const opening = this.load(this.index.get(id)).finally(() => this.opening.delete(id));
    this.opening.set(id, opening);
    return opening;
  }
  async create(workspaceId: string) {
    if (this.closing) throw new Error('Chat runtime is closing');
    await this.workspace(workspaceId);
    if (this.closing) throw new Error('Chat runtime is closing');
    const record: ChatRecord = { id: randomUUID(), workspaceId, title: '新会话', sessionFile: null, updatedAt: Date.now() };
    this.index.save(record);
    return this.open(record.id);
  }
  async open(id: string) { return this.snapshot(id, await this.require(id)); }

  /** Hive invokes this for both the selected orchestrator and spawned workers. */
  async start(input: Parameters<StructuredSessionPort['start']>[0]): Promise<StructuredSessionHandle> {
    let record = this.index.forAgent(input.workspace.id, input.agentId);
    if (!record) {
      const members = await this.hive.request('GET', `/api/ui/workspaces/${input.workspace.id}/team`) as { id: string; name: string }[];
      const args = input.config.args ?? [];
      const provider = args[args.indexOf('--provider') + 1];
      const modelId = args[args.indexOf('--model') + 1];
      record = { id: randomUUID(), workspaceId: input.workspace.id, agentId: input.agentId,
        title: input.agentId.endsWith(':orchestrator') ? '主会话' : members.find(member => member.id === input.agentId)?.name ?? '成员会话', sessionFile: null, updatedAt: Date.now(),
        ...(args.includes('--provider') && args.includes('--model') && provider && modelId ? { model: { provider, id: modelId, name: modelId } } : {}),
      };
      this.index.save(record);
    }
    const id = record.id;
    const chat = await this.require(id);
    // Keep resumed empty sessions authoritative: Pi's file does not exist until
    // its first user message, so a newly loaded SDK can have a new session id.
    return {
      sessionId: chat.handle.sessionId,
      resumed: chat.handle.history().some(message => !!message && typeof message === 'object' && 'customType' in message && message.customType === 'hive-context'),
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
    return { command: 'hive:pi', commandPresetId: null,
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
    const agentId = record.agentId ?? `${record.workspaceId}:orchestrator`;
    const previous = this.index.forAgent(record.workspaceId, agentId);
    if (previous && previous.id !== id) {
      const old = await this.require(previous.id);
      const team = await this.hive.request('GET', `/api/ui/workspaces/${record.workspaceId}/team`) as { pending_task_count: number; open_dispatches?: unknown[] }[];
      if (old?.queue.busy || old?.queue.pending.length || team.some(member => member.pending_task_count > 0 || member.open_dispatches?.length)) {
        throw new Error('当前主会话仍有执行或未结派单，请先完成或停止相关任务，再切换主会话。');
      }
    }
    const active = this.runtime.getActiveRunByAgentId(record.workspaceId, agentId);
    if (previous?.id !== id && active) {
      this.runtime.stopAgentRun(active.runId);
      if (!await this.runtime.waitForRunExit(active.runId, 30_000)) throw new Error('Previous team session is still stopping');
    }
    this.index.bind(id, agentId);
    this.runtime.configureAgentLaunch(record.workspaceId, agentId, this.launchConfig(this.index.get(id)));
    // The Hive route performs its normal agent status transition and outbox replay.
    await this.hive.request('POST', `/api/workspaces/${record.workspaceId}/agents/${encodeURIComponent(agentId)}/start`);
  }
  async prompt(id: string, text: string) {
    if (!text.trim()) throw new Error('Message cannot be empty');
    return this.withWorkspace(this.index.get(id).workspaceId, async () => {
      await this.require(id);
      await this.activateOnce(id);
      const chat = await this.require(id);
      if (!chat.queue.pending.length) chat.queue.resume();
      const record = this.index.get(id);
      this.index.save({ ...record, title: record.title === '新会话' ? text.trim().slice(0, 60) : record.title, updatedAt: Date.now() });
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
  async abort(id: string) {
    const chat = await this.require(id);
    const paused = chat.queue.pause();
    await chat.handle.abort();
    await paused;
    chat.queue.discardCurrent();
    // Explicit cancel affects this response only. Pending team deliveries stay
    // durable and require an explicit resume, never become successful reports.
    return { ok: true };
  }
  async resume(id: string) {
    return this.withWorkspace(this.index.get(id).workspaceId, async () => {
      await this.activateOnce(id);
      (await this.require(id)).queue.resume();
      return { ok: true };
    });
  }
  async setModel(id: string, provider: string, modelId: string) {
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
  private async stopLive(id: string, chat: LiveChat) {
    const paused = chat.queue.pause();
    await chat.handle.abort();
    await paused;
    chat.handle.dispose();
    this.live.delete(id);
  }
  async close() {
    this.closing = true;
    await Promise.allSettled(this.opening.values());
    await Promise.all([...this.live.entries()].map(([id, chat]) => this.stopLive(id, chat)));
  }
}

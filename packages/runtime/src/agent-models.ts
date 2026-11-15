import { AgentProfiles, defaultAgentProfileId, type AgentProfileRef, type ResolvedAgentProfile } from './agent-profiles.js';
import { createAgentSession } from './agent-session-factory.js';
import { workspaceRemovalBlocked } from './workspace-removal.js';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import type { PiSessionHandle, PiEvent } from './agents/types.js';
import type { ChatBackend } from './chat-index.js';
import type { DesktopSettings } from './desktop-settings.js';
import type { HiveHost } from './hive-host.js';
import type { WorkspaceSummary } from './hive-runtime-contract.js';

type Model = { provider: string; id: string; name: string };
export interface AgentModelCatalog { supported?: boolean; reason?: string; models: (Model & { available: boolean })[]; current?: Model }
export class AgentModelsInputError extends Error {
  constructor(message: string) { super(message); this.name = 'AgentModelsInputError'; }
}

/** Read native catalogs without binding a Hive member, creating a desktop chat, or prompting. */
export class AgentModels {
  private readonly pending = new Map<string, Promise<AgentModelCatalog>>();
  private readonly closeAbort = new AbortController();
  private closing = false;
  private readonly removing = new Set<string>();
  constructor(private readonly hive: HiveHost, private readonly settings: DesktopSettings, private readonly profiles = new AgentProfiles(hive)) {}
  async list(workspaceId: string, backend: unknown, ref?: AgentProfileRef): Promise<AgentModelCatalog> {
    if (this.closing) return Promise.reject(new Error('Model catalog runtime is closing'));
    if (backend !== 'pi' && backend !== 'codex' && backend !== 'pi-rpc' && backend !== 'claude' && backend !== 'acp') {
      return Promise.reject(new AgentModelsInputError('Unsupported model catalog backend'));
    }
    if (typeof workspaceId !== 'string' || !workspaceId.trim()) return Promise.reject(new AgentModelsInputError('workspaceId is required'));
    if (this.removing.has(workspaceId)) return Promise.reject(workspaceRemovalBlocked(workspaceId));
    await this.profiles.initialize(await this.settings.get());
    const profile = await this.profiles.resolveForExecution(ref ?? await this.profiles.currentRef(defaultAgentProfileId(backend)), backend);
    if (this.removing.has(workspaceId)) throw workspaceRemovalBlocked(workspaceId);
    if (this.closing) throw new Error('Model catalog runtime is closing');
    if (backend === 'acp') {
      const workspaces=await this.hive.request('GET','/api/workspaces') as WorkspaceSummary[];
      if(!workspaces.some(workspace=>workspace.id===workspaceId))throw new AgentModelsInputError('Workspace not found');
      return {models:[],supported:false,reason:'ACP model catalog and model selection are not implemented'};
    }
    const key = JSON.stringify([workspaceId, profile.ref.id, profile.ref.revision]);
    const pending = this.pending.get(key);
    if (pending) return pending;
    const request = this.load(workspaceId, backend, profile).finally(() => this.pending.delete(key));
    this.pending.set(key, request);
    return request;
  }
  private async load(workspaceId: string, backend: ChatBackend, profile: ResolvedAgentProfile): Promise<AgentModelCatalog> {
    const workspaces = await this.hive.request('GET', '/api/workspaces') as WorkspaceSummary[];
    const workspace = workspaces.find(candidate => candidate.id === workspaceId);
    if (!workspace) throw new AgentModelsInputError('Workspace not found');
    this.closeAbort.signal.throwIfAborted();
    const directory = await mkdtemp(join(tmpdir(), 'hive-agent-models-'));
    let handle: PiSessionHandle | undefined;
    let failure: Error | undefined;
    const onEvent = (event: PiEvent) => { if (event.type === 'error') failure = new Error(event.message); };
    const onUnexpectedExit = (error: Error) => { failure = error; };
    try {
      this.closeAbort.signal.throwIfAborted();
      const options = { cwd: workspace.path, sessionDir: directory, onEvent };
      handle = await createAgentSession({backend, profile,
        ...(backend === 'pi' ? {pi: {...options, onUnexpectedExit}}
          : backend === 'codex' ? {codex: {...options, signal: this.closeAbort.signal, onUnexpectedExit}}
          : backend === 'claude' ? {claude: {...options, signal: this.closeAbort.signal, onUnexpectedExit}}
          : {externalPi: {...options, signal: this.closeAbort.signal, onUnexpectedExit}})});
      this.closeAbort.signal.throwIfAborted();
      if (failure) throw failure;
      const models = handle.listModels().map(model => ({ provider: model.provider, id: model.id, name: model.name, available: model.available }));
      const selected = handle.currentModel();
      return { models, ...(selected && { current: { provider: selected.provider, id: selected.id, name: selected.name } }) };
    } finally {
      try { await handle?.dispose(); }
      finally { await rm(directory, { recursive: true, force: true }); }
    }
  }
  async withWorkspaceRemoval<T>(workspaceId: string, operation: () => Promise<T>): Promise<T> {
    if (this.removing.has(workspaceId)) throw workspaceRemovalBlocked(workspaceId);
    this.removing.add(workspaceId);
    try {
      const requests = [...this.pending].filter(([key]) => (JSON.parse(key) as unknown[])[0] === workspaceId).map(([, request]) => request);
      const settled = await Promise.allSettled(requests);
      const errors = settled.flatMap(result => result.status === 'rejected' ? [result.reason] : []);
      if (errors.length) throw new AggregateError(errors, '工作区模型查询未正常结束，无法确认其原生进程已经退出。');
      return await operation();
    } finally { this.removing.delete(workspaceId); }
  }
  async close(): Promise<void> {
    this.closing = true;
    this.closeAbort.abort(new Error('Model catalog runtime is closing'));
    await Promise.allSettled(this.pending.values());
  }
}

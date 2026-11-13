import { workspaceRemovalBlocked } from './workspace-removal.js';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createClaudeSession } from './agents/claude.js';
import { createPiSession } from './agents/pi.js';
import { createCodexSession } from './agents/codex.js';
import { createExternalPiSession } from './agents/external-pi.js';
import type { PiSessionHandle, PiEvent } from './agents/types.js';
import type { ChatBackend } from './chat-index.js';
import type { DesktopSettings } from './desktop-settings.js';
import type { HiveHost } from './hive-host.js';
import type { WorkspaceSummary } from './hive-runtime-contract.js';

type Model = { provider: string; id: string; name: string };
export interface AgentModelCatalog { models: (Model & { available: boolean })[]; current?: Model }
export class AgentModelsInputError extends Error {
  constructor(message: string) { super(message); this.name = 'AgentModelsInputError'; }
}

/** Read native catalogs without binding a Hive member, creating a desktop chat, or prompting. */
export class AgentModels {
  private readonly pending = new Map<string, Promise<AgentModelCatalog>>();
  private readonly closeAbort = new AbortController();
  private closing = false;
  private readonly removing = new Set<string>();
  constructor(private readonly hive: HiveHost, private readonly settings: DesktopSettings) {}
  list(workspaceId: string, backend: unknown): Promise<AgentModelCatalog> {
    if (this.closing) return Promise.reject(new Error('Model catalog runtime is closing'));
    if (backend !== 'pi' && backend !== 'codex' && backend !== 'pi-rpc' && backend !== 'claude') {
      return Promise.reject(new AgentModelsInputError('Unsupported model catalog backend'));
    }
    if (typeof workspaceId !== 'string' || !workspaceId.trim()) return Promise.reject(new AgentModelsInputError('workspaceId is required'));
    if (this.removing.has(workspaceId)) return Promise.reject(workspaceRemovalBlocked(workspaceId));
    const key = `${backend}:${workspaceId}`;
    const pending = this.pending.get(key);
    if (pending) return pending;
    const request = this.load(workspaceId, backend).finally(() => this.pending.delete(key));
    this.pending.set(key, request);
    return request;
  }
  private async load(workspaceId: string, backend: ChatBackend): Promise<AgentModelCatalog> {
    const workspaces = await this.hive.request('GET', '/api/workspaces') as WorkspaceSummary[];
    const workspace = workspaces.find(candidate => candidate.id === workspaceId);
    if (!workspace) throw new AgentModelsInputError('Workspace not found');
    const executables = backend === 'pi' ? undefined : (await this.settings.get()).effective;
    this.closeAbort.signal.throwIfAborted();
    const directory = await mkdtemp(join(tmpdir(), 'hive-agent-models-'));
    let handle: PiSessionHandle | undefined;
    let failure: Error | undefined;
    const onEvent = (event: PiEvent) => { if (event.type === 'error') failure = new Error(event.message); };
    const onUnexpectedExit = (error: Error) => { failure = error; };
    try {
      this.closeAbort.signal.throwIfAborted();
      const options = { cwd: workspace.path, sessionDir: directory, onEvent };
      handle = backend === 'pi' ? await createPiSession(options)
        : backend === 'codex' ? await createCodexSession({ ...options, command: executables!.codex.command, signal: this.closeAbort.signal, onUnexpectedExit })
        : backend === 'claude' ? await createClaudeSession({ ...options, command: executables!.claude.command, signal: this.closeAbort.signal, onUnexpectedExit })
        : await createExternalPiSession({ ...options, command: executables!.pi.command, signal: this.closeAbort.signal, onUnexpectedExit });
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
      const requests = [...this.pending].filter(([key]) => key.slice(key.indexOf(':') + 1) === workspaceId).map(([, request]) => request);
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

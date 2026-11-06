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
  private closing = false;
  constructor(private readonly hive: HiveHost, private readonly settings: DesktopSettings) {}
  list(workspaceId: string, backend: unknown): Promise<AgentModelCatalog> {
    if (this.closing) return Promise.reject(new Error('Model catalog runtime is closing'));
    if (backend !== 'pi' && backend !== 'codex' && backend !== 'pi-rpc' && backend !== 'claude') {
      return Promise.reject(new AgentModelsInputError('Unsupported model catalog backend'));
    }
    if (typeof workspaceId !== 'string' || !workspaceId.trim()) return Promise.reject(new AgentModelsInputError('workspaceId is required'));
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
    const directory = await mkdtemp(join(tmpdir(), 'hive-agent-models-'));
    let handle: PiSessionHandle | undefined;
    let failure: Error | undefined;
    const onEvent = (event: PiEvent) => { if (event.type === 'error') failure = new Error(event.message); };
    const onUnexpectedExit = (error: Error) => { failure = error; };
    try {
      const options = { cwd: workspace.path, sessionDir: directory, onEvent };
      handle = backend === 'pi' ? await createPiSession(options)
        : backend === 'codex' ? await createCodexSession({ ...options, command: executables!.codex.command, onUnexpectedExit })
        : backend === 'claude' ? await createClaudeSession({ ...options, command: executables!.claude.command, onUnexpectedExit })
        : await createExternalPiSession({ ...options, command: executables!.pi.command, onUnexpectedExit });
      if (failure) throw failure;
      const models = handle.listModels().map(model => ({ provider: model.provider, id: model.id, name: model.name, available: model.available }));
      const selected = handle.currentModel();
      return { models, ...(selected && { current: { provider: selected.provider, id: selected.id, name: selected.name } }) };
    } finally {
      try { await handle?.dispose(); }
      finally { await rm(directory, { recursive: true, force: true }); }
    }
  }
  async close(): Promise<void> {
    this.closing = true;
    await Promise.allSettled(this.pending.values());
  }
}

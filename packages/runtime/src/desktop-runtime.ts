import { AgentProfiles, defaultAgentProfileId, type AgentProfileRef } from './agent-profiles.js';
import { withStructuredProfile } from './structured-launch.js';
import { ExecutionMutations } from './execution-mutations.js';
import { ProjectWorktreeInitialization } from './project-worktree-initialization.js';
import { ProjectScriptSettings } from './project-script-settings.js';
import { ProjectScriptRuns, type ScriptEvent } from './project-script-runs.js';
import { Terminals, type TerminalEvent } from './terminal.js';
import { AgentModels } from './agent-models.js';
import { join } from 'node:path';
import { Chats, type ChatEvent } from './chats.js';
import { createHiveHost } from './hive-host.js';
import type { HiveAgentRuntime } from './hive-runtime-contract.js';
import { DesktopSettings, type DesktopExecutableEnvironment } from './desktop-settings.js';
import { createStructuredRuntime } from './structured-runtime.js';
import { PiAuth } from './pi-auth.js';
import { PiModelConfig } from './pi-model-config.js';
import { isWorkerRole } from '@tt-a1i/hive/dist/src/shared/types.js';
import { structuredCommand } from './structured-runtime.js';

export async function createDesktopRuntime(dataDir: string, emit: (event: ChatEvent) => void,
  options: { executableEnvironment?: DesktopExecutableEnvironment; onTerminalEvent?: (event: TerminalEvent) => void; onScriptEvent?: (event: ScriptEvent) => void } = {}) {
  const executionMutations = new ExecutionMutations();
  let chats: Chats;
  let agents: HiveAgentRuntime | undefined;
  let profiles: AgentProfiles;
  const hive = await createHiveHost({
    dataDir: join(dataDir, 'hive'),
    agentRuntimeFactory: context => {
      const structured = createStructuredRuntime(context, { start: input => chats.start(input) }, backend => profiles.currentRefSnapshot(defaultAgentProfileId(backend)));
      const gated = new Set(['startAgent', 'resumeRun', 'writeStatusPrompt', 'writeSendPrompt', 'writeCancelPrompt',
        'writeUserInputPrompt', 'writeSystemMessageToAgent', 'deliverUserInputToOrchestrator', 'deliverSystemMessageToAgent']);
      const deferred = new Set(['startAgent', 'writeStatusPrompt', 'writeSendPrompt', 'writeCancelPrompt', 'deliverUserInputToOrchestrator', 'deliverSystemMessageToAgent']);
      agents = new Proxy(structured, {get(target, property, receiver) {
        if (property === 'executionAdmissionBlocked') return () => executionMutations.isLeased('');
        if (property === 'beginExecutionAdmission') return (workspaceId: string) => executionMutations.begin(workspaceId);
        const value = Reflect.get(target, property, receiver) as unknown;
        if (typeof property !== 'string' || !gated.has(property) || typeof value !== 'function') return value;
        return (...args: unknown[]) => {
          const first = args[0];
          const workspace = typeof first === 'string' ? first : (first as {id?: string} | undefined)?.id ?? '';
          if (deferred.has(property) && executionMutations.isLeased(workspace)) {
            // Preserve already-recorded Hive facts and claims; execute only after
            // the operation releases its lease. Recheck if another lease begins.
            const retry = async (): Promise<unknown> => {
              await executionMutations.whenReleased();
              if (executionMutations.isLeased(workspace)) return retry();
              const release = executionMutations.begin(workspace);
              try { return await Reflect.apply(value, target, args); }
              finally { release(); }
            };
            return retry();
          }
          const release = executionMutations.begin(workspace);
          try {
            const result: unknown = Reflect.apply(value, target, args);
            if (result instanceof Promise) return result.finally(release);
            release(); return result;
          } catch (error) { release(); throw error; }
        };
      }});
      return agents;
    },
  });
  const unsubscribeAdmission = executionMutations.onRelease(() => hive.resumeExecutionDeliveries());
  const settings = new DesktopSettings(hive, options.executableEnvironment);
  profiles = new AgentProfiles(hive);
  settings.attachProfiles(profiles);
  const agentModels = new AgentModels(hive, settings, profiles);
  const piAuth = new PiAuth();
  const piModels = new PiModelConfig();
  try {
    if (!agents) throw new Error('Hive runtime factory was not installed');
    await profiles.initialize(await settings.get());
    chats = new Chats(hive, dataDir, emit, agents, settings, agentModels, profiles);
    await chats.initializeProfiles();
  } catch (error) { await hive.close(); throw error; }
  const terminal = new Terminals(hive, workspaceId => chats.assertWorkspaceAvailable(workspaceId), options.onTerminalEvent ?? (() => {}), (workspaceId, sessionId) => chats.executionDirectory(workspaceId, sessionId), executionMutations);
  const projectScripts = new ProjectScriptSettings(hive.projectScriptState, workspaceId => chats.assertWorkspaceAvailable(workspaceId));
  let scriptRuns: ProjectScriptRuns;
  try {
    scriptRuns = new ProjectScriptRuns(hive, projectScripts, dataDir, workspaceId => chats.assertWorkspaceAvailable(workspaceId),
      (workspaceId, sessionId) => chats.executionDirectory(workspaceId, sessionId), options.onScriptEvent, executionMutations);
  } catch (error) { await hive.close(); await chats.close(); throw error; }
  const initialization = new ProjectWorktreeInitialization(dataDir, projectScripts, scriptRuns,
    (workspaceId, sessionId) => chats.initializationPlan(workspaceId, sessionId), workspaceId => chats.assertWorkspaceAvailable(workspaceId));
  chats.initialization = initialization;
  chats.configureHistory(executionMutations, () => terminal.hasAnyActiveShell());
  let closing: Promise<void> | undefined;
  return { hive, chats, profiles, agents, executionMutations, settings, agentModels, terminal, piAuth, piModels, projectScripts, scriptRuns, initialization,
    createMember: (workspaceId: string, input: { name: unknown; role: unknown; backend: unknown; profile?: AgentProfileRef; model?: {provider: string; id: string}; description?: unknown }) => {
      chats.assertWorkspaceAvailable(workspaceId);
      if (typeof input.name !== 'string' || !input.name.trim() || !isWorkerRole(input.role)
        || typeof input.backend !== 'string' || !structuredCommand(`hive:${input.backend}`)
        || (input.description !== undefined && typeof input.description !== 'string')) throw new TypeError('Invalid desktop member configuration');
      if (input.backend === 'acp' && input.model) throw new TypeError('ACP model selection is not implemented');
      const profile = profiles.resolveForExecutionSnapshot(input.profile ?? profiles.currentRefSnapshot(defaultAgentProfileId(input.backend as import('./chat-index.js').ChatBackend)), input.backend as import('./chat-index.js').ChatBackend);
      const launch = withStructuredProfile({command: `hive:${input.backend}`, args: [...(input.backend === 'acp' ? ['--profile-id',profile.ref.id,'--profile-revision',String(profile.ref.revision)] : []), ...(input.model ? ['--provider', input.model.provider, '--model', input.model.id] : [])], commandPresetId: null}, profile.ref);
      return hive.createMember(workspaceId, { name: input.name, role: input.role,
        ...(input.description !== undefined && { description: input.description as string }) }, launch);
    },
    removeWorkspace: async (workspaceId: string) => {
      await projectScripts.list(workspaceId); // Refuse silently discarding unreadable settings.
      return chats.removeWorkspace(workspaceId, async () => {await scriptRuns.stopWorkspace(workspaceId); await terminal.detachWorkspace(workspaceId);});
    },
    requestHive: (method: string, path: string, body?: unknown) => {
      if (!['GET', 'HEAD'].includes(method.toUpperCase())) {
        const match = /^\/api\/(?:ui\/)?workspaces\/([^/?#]+)(?:[/?#]|$)/.exec(path);
        if (match) chats.assertWorkspaceAvailable(decodeURIComponent(match[1]!));
      }
      return hive.request(method, path, body);
    },
    close: () => closing ??= (async () => {
      chats.beginClose();
      await chats.history?.close();
      unsubscribeAdmission();
      initialization.close();
      const scripts = await Promise.allSettled([scriptRuns.close()]);
      const stage1 = await Promise.allSettled([settings.close(), piModels.close(), piAuth.close(), terminal.close(), agentModels.close()]);
      const stage2 = await Promise.allSettled([hive.close()]);
      const stage3 = await Promise.allSettled([chats.close()]);
      const errors = [...scripts, ...stage1, ...stage2, ...stage3].flatMap(result => result.status === 'rejected' ? [result.reason] : []);
      if (errors.length === 1) throw errors[0];
      if (errors.length > 1) throw new AggregateError(errors, 'Desktop runtime close failed');
    })(),
  };
}

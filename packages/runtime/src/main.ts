import { attachmentIds, ProjectCopyChangedError, QuestionAttachmentCopyError } from './image-attachments.js';
import { parseStructuredLaunch } from './structured-launch.js';
import { isAgentDriver, validateAgentProfileLaunch, validateAgentProfileRef, type UpdateAgentProfile } from './agent-profiles.js';
import type { HandoffInput,HandoffApplyInput } from './session-handoffs.js';
import { HandoffError } from './handoff-operation-journal.js';
import { SessionIdentityError, worktreeDraftAvailability, type ChatRecord } from './chat-index.js';
import { HistoryOperationError } from './history-operation-journal.js';
import { CodeCheckpointError } from './code-checkpoints.js';
import { WorktreeInitializationError } from './project-worktree-initialization.js';
import { GitWorktreeError } from './git-worktrees.js';
import { ProjectScriptRunError } from './project-script-runs.js';
import { ProjectScriptSettingsError } from './project-script-settings.js';
import { getAgentDir } from '@earendil-works/pi-coding-agent';
import { format } from 'node:util';
import { createHash } from 'node:crypto';
import { createInterface } from 'node:readline';
import { mkdirSync } from 'node:fs';
import { CodexInteractionError } from './agents/codex-interactions.js';
import { WorkspaceRemovalError } from './workspace-removal.js';
import { HiveHttpError, withInboxDispatchBackends } from './hive-host.js';
import { createDesktopRuntime } from './desktop-runtime.js';
import { structuredCommand } from './structured-runtime.js';
import { desktopDataDir } from './paths.js';
import { remoteCommands } from './remote-commands.js';
import { captureRuntimeInfo } from './runtime-info.js';
import { DesktopUpdates } from './desktop-updates.js';
import { DesktopImport } from './desktop-import.js';
import { scratchWorkspace } from './scratch-workspace.js';
import { desktopHistoryMessage } from './desktop-history.js';
import { boundOversizedToolOutput, sliceHistoryWindow, StaleHistorySnapshotError } from './history-cursor.js';
import { collectReadinessReport } from './readiness.js';
import { collectSkillsAndRules } from './skills-rules.js';
import {
  probeMcpServer,
  removeProjectMcpServer,
  resolveMcpServerTarget,
  toggleProjectMcpServer,
  upsertProjectMcpServer,
} from './mcp-manager.js';

const protocolWrite = process.stdout.write.bind(process.stdout);
let outputClosed = false;
let closeInput: (() => void) | undefined;
process.stdout.on('error', (error: NodeJS.ErrnoException) => {
  outputClosed = true;
  if (error.code !== 'EPIPE') console.error(error);
  closeInput?.();
});
// The SDK and user extensions may log at any point. Keep ordinary stdout writes
// away from the desktop wire; only this captured writer sends protocol frames.
process.stdout.write = process.stderr.write.bind(process.stderr);
console.log = (...args: unknown[]) => { process.stderr.write(`${format(...args)}\n`); };
const pendingWrites = new Set<Promise<void>>();
const writeFrame = (message: unknown): Promise<void> => {
  if (outputClosed) return Promise.resolve();
  // Keep legacy malformed text readable by Rust without rewriting stored history.
  const frame = JSON.stringify(message, (_key, value: unknown) =>
    typeof value === 'string' ? value.replace(/[\uD800-\uDFFF]/gu, '\uFFFD') : value);
  let tracked!: Promise<void>;
  tracked = new Promise<void>(resolve => {
    protocolWrite(`${frame}\n`, () => resolve());
  }).finally(() => { pendingWrites.delete(tracked); });
  pendingWrites.add(tracked);
  return tracked;
};
const send = (message: unknown) => { void writeFrame(message); };

const runtimeInfo = captureRuntimeInfo();
const dataDir = desktopDataDir();
mkdirSync(dataDir, { recursive: true });
const runtime = await createDesktopRuntime(dataDir, event => send({ event: 'agent', data: event }), {
  onTerminalEvent: data => send({ event: 'terminal', data }),
  onScriptEvent: data => send({ event: 'script', data }),
  onInitializationChange: data => send({ event: 'worktreeInitializationChanged', data }),
});
const updates = new DesktopUpdates(dataDir, new URL('../../desktop-update-channel.json', import.meta.url));
const { hive, chats, profiles, settings, agentModels, terminal, piAuth } = runtime;
const desktopImport = new DesktopImport(dataDir, profiles, runtime.requestHive,
  (workspace, sessions) => chats.importNativeSessions(workspace, sessions));
let closing: Promise<void> | undefined;
let closingInstallerId: string | undefined;
const searches = new Map<number | string, AbortController>();
async function runSearch<T>(requestId: number | string, search: (signal: AbortSignal) => Promise<T>) {
  if (searches.has(requestId)) throw new Error('Search request id is already in use');
  const controller = new AbortController();
  searches.set(requestId, controller);
  try { return await search(controller.signal); }
  finally { searches.delete(requestId); }
}
const close = (installerId?: string) => {
  if (!closing) {
    for (const search of searches.values()) search.abort();
    closingInstallerId = installerId;
    closing = Promise.allSettled([updates.close(installerId), desktopImport.close().then(() => runtime.close())]).then(([update, sidecar]) => {
      if (sidecar.status === 'rejected') throw sidecar.reason;
      if (update.status === 'rejected') throw update.reason;
    });
  }
  return closing;
};

function record(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error('Expected an object');
  return value as Record<string, unknown>;
}
function text(params: Record<string, unknown>, name: string): string {
  const value = params[name];
  if (typeof value !== 'string' || !value.trim()) throw new Error(`${name} is required`);
  return value;
}

function exact(params: Record<string, unknown>, allowed: string[]) {
  if (Object.keys(params).some(key => !allowed.includes(key))) throw new Error('Unexpected request field');
}
function navigationSearchSession(session: ChatRecord, includeInboxActivity = false) {
  const {id, title, customTitle, pinned, pinRank, archived, updatedAt, workspaceId, agentId, originAgentId, identityState,
    backend, execution, scratch, profile, worktreeReleased} = session;
  return {id, title, customTitle, pinned, pinRank, archived, updatedAt, workspaceId, agentId, originAgentId, identityState,
    backend, execution, scratch, profile, worktreeReleased,
    ...worktreeDraftAvailability(session),
    ...(includeInboxActivity && session.inboxReply?.receivedAt !== undefined
      && {inboxReply: {revision: session.inboxReply.revision, receivedAt: session.inboxReply.receivedAt}})};
}
function viewRevision(value: unknown): number {
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 1)
    throw new TypeError('viewRevision must be a positive safe integer');
  return value;
}
function storeVersion(params: Record<string, unknown>): number {
  if (!Number.isSafeInteger(params.expectedStoreVersion) || (params.expectedStoreVersion as number) < 1) throw new Error('expectedStoreVersion is required');
  return params.expectedStoreVersion as number;
}
function optionalProfile(params: Record<string, unknown>) {
  return params.profile === undefined ? {} : {profile: validateAgentProfileRef(params.profile)};
}
function optionalModel(params: Record<string, unknown>) {
  if (params.model === undefined) return {};
  const model = record(params.model); exact(model, ['provider', 'id']);
  return {model: {provider: text(model, 'provider'), id: text(model, 'id')}};
}
async function dispatch(requestId: number | string, method: string, params: Record<string, unknown>): Promise<unknown> {
  if (closing && method !== 'shutdown') throw new Error('Desktop runtime is closing');
  switch (method) {
    case 'desktop.import.discover': exact(params, []); return desktopImport.discover();
    case 'desktop.import.apply': exact(params, ['projectIds']); return desktopImport.apply(params.projectIds);
    case 'desktop.updates.check': exact(params, []); return updates.check();
    case 'desktop.updates.stage': exact(params, ['version', 'sha256']); return updates.stage(text(params, 'version'), text(params, 'sha256'));
    case 'desktop.updates.prepare': exact(params, ['version', 'sha256']); return updates.prepare(text(params, 'version'), text(params, 'sha256'));
    case 'desktop.updates.prepareInstaller': exact(params, ['version', 'sha256']); return updates.prepareInstaller(text(params, 'version'), text(params, 'sha256'));
    case 'desktop.updates.discardInstaller': exact(params, ['id']); await updates.discardInstaller(text(params, 'id')); return { ok: true };
    case 'chat.acp.uncertainDeliveries': exact(params,['sessionId']);return chats.uncertainAcpDeliveries(text(params,'sessionId'));
    case 'chat.queue.uncertainDeliveries': exact(params,['sessionId']);return chats.uncertainDeliveries(text(params,'sessionId'));
    case 'chat.queue.discardUncertainDelivery': {exact(params,['sessionId','deliveryId','expectedText','expectedProfile','expectedNativeSessionId','expectedVersion']);if(typeof params.expectedText!=='string')throw new Error('expectedText is required');return chats.discardUncertainDelivery(text(params,'sessionId'),text(params,'deliveryId'),params.expectedText,params.expectedProfile,text(params,'expectedNativeSessionId'),params.expectedVersion);}
    case 'chat.queue.retryUncertainDelivery': {exact(params,['sessionId','deliveryId','expectedText','expectedProfile','expectedNativeSessionId','expectedVersion']);if(typeof params.expectedText!=='string')throw new Error('expectedText is required');return chats.retryUncertainDelivery(text(params,'sessionId'),text(params,'deliveryId'),params.expectedText,params.expectedProfile,text(params,'expectedNativeSessionId'),params.expectedVersion);}
    case 'chat.acp.prepareModes': exact(params,['sessionId']);return chats.prepareAcpModes(text(params,'sessionId'));
    case 'chat.acp.discardUncertainDelivery': {exact(params,['sessionId','deliveryId','expectedText']);if(typeof params.expectedText!=='string')throw new Error('expectedText is required');return chats.discardUncertainAcpDelivery(text(params,'sessionId'),text(params,'deliveryId'),params.expectedText);}
    case 'agent.capabilities': exact(params,['sessionId']);return chats.capabilities(text(params,'sessionId'));
    case 'agent.profiles.list': exact(params, []); return profiles.list();
    case 'agent.profiles.create': {
      exact(params, ['driver', 'label', 'launch', 'expectedStoreVersion']);
      if (!isAgentDriver(params.driver)) throw new Error('Unsupported profile driver');
      return profiles.create({driver: params.driver, label: text(params, 'label'), launch: validateAgentProfileLaunch(params.driver, params.launch)}, storeVersion(params));
    }
    case 'agent.profiles.update': {
      exact(params, ['id', 'label', 'launch', 'expectedStoreVersion']);
      const current = await profiles.getRef(await profiles.currentRef(text(params, 'id')));
      const change: UpdateAgentProfile = {...(params.label !== undefined && {label: text(params, 'label')}),
        ...(params.launch !== undefined && {launch: validateAgentProfileLaunch(current.driver, params.launch)})};
      if (!Object.keys(change).length) throw new Error('A profile change is required');
      return profiles.update(current.ref.id, change, storeVersion(params));
    }
    case 'agent.profiles.disable':
    case 'agent.profiles.enable': {
      exact(params, ['id', 'expectedStoreVersion']);
      return profiles.enableDisable(text(params, 'id'), method === 'agent.profiles.enable', storeVersion(params));
    }
    case 'chat.handoff.capabilities': exact(params,['workspaceId','sourceSessionId']);return chats.handoffs!.capabilities(text(params,'workspaceId'),text(params,'sourceSessionId'));
    case 'chat.handoff.preview': return chats.handoffs!.preview(params as unknown as HandoffInput);
    case 'chat.handoff.apply': return chats.handoffs!.apply(params as unknown as HandoffApplyInput);
    case 'chat.handoff.get':exact(params,['workspaceId','operationId']);return chats.handoffs!.get(text(params,'workspaceId'),text(params,'operationId'));
    case 'chat.handoff.list':exact(params,['workspaceId']);return chats.handoffs!.list(text(params,'workspaceId'));
    case 'chat.handoff.recover':exact(params,['workspaceId','operationId']);return chats.handoffs!.recover(text(params,'workspaceId'),text(params,'operationId'));
    case 'chat.handoffTargetState': {
      exact(params, ['sessionId', 'viewRevision']);
      const id = text(params, 'sessionId');
      return params.viewRevision === undefined ? chats.handoffTargetState(id)
        : chats.readDesktopView(id, viewRevision(params.viewRevision), () => chats.handoffTargetState(id));
    }
    case 'history.capabilities': return chats.history!.capabilities(text(params,'workspaceId'),text(params,'sessionId'));
    case 'checkpoint.preview': return chats.history!.checkpointPreview(text(params,'workspaceId'),text(params,'sessionId'));
    case 'checkpoint.capture': return chats.history!.checkpointCapture(text(params,'workspaceId'),text(params,'sessionId'),text(params,'expectedVersion'),text(params,'requestId'));
    case 'checkpoint.list': return chats.history!.checkpointList(text(params,'workspaceId'),text(params,'sessionId'));
    case 'history.fork.preview': case 'history.rewind.preview': {
      if(Object.keys(params).some(key=>!['workspaceId','sessionId','boundary','code','edit'].includes(key)))throw new Error('Supply only source session, exact native boundary, and code choice');
      const boundary=record(params.boundary),code=record(params.code);
      if(Object.keys(boundary).some(key=>key!=='nativeUserItemId')||Object.keys(code).some(key=>!['kind','checkpointId'].includes(key))||!['current','checkpoint'].includes(code.kind as string)||params.edit!==undefined&&typeof params.edit!=='boolean')throw new Error('Invalid history fork selection');
      return chats.history!.preview({workspaceId:text(params,'workspaceId'),sessionId:text(params,'sessionId'),boundary:{nativeUserItemId:text(boundary,'nativeUserItemId')},code:code.kind==='current'?{kind:'current'}:{kind:'checkpoint',checkpointId:text(code,'checkpointId')},...(params.edit!==undefined&&{edit:params.edit as boolean}),...(method==='history.rewind.preview'&&{rewind:true})});
    }
    case 'history.fork.apply': case 'history.rewind.apply': {
      exact(params, method==='history.rewind.apply'?['workspaceId','sessionId','previewToken','requestId','confirmed']:['workspaceId','sessionId','previewToken','requestId']);
      if(method==='history.rewind.apply'&&params.confirmed!==true)throw new Error('Explicit confirmation is required for an in-place rewind');
      return chats.history!.apply(text(params,'workspaceId'),text(params,'sessionId'),text(params,'previewToken'),text(params,'requestId'),method==='history.rewind.apply');
    }
    case 'history.operations.recover': return chats.history!.recover(text(params,'workspaceId'),text(params,'operationId'));
    case 'history.operations.get': return chats.history!.get(text(params,'workspaceId'),text(params,'operationId'));
    case 'history.operations.list': return chats.history!.list(text(params,'workspaceId'));
    case 'pi.models.get': return runtime.piModels.get();
    case 'pi.models.change': return runtime.piModels.change(params.revision, params.change);
    case 'pi.vault.get': exact(params, []); return runtime.piVault.view();
    case 'pi.vault.change': {
      exact(params, ['revision', 'configRevision', 'provider', 'reference']);
      return runtime.piVault.change(params.revision, params.configRevision, text(params, 'provider'), params.reference);
    }
    case 'pi.auth.providers': return piAuth.providers();
    case 'pi.auth.start': return piAuth.start(text(params, 'providerId'), params.authType);
    case 'pi.auth.status': return piAuth.status();
    case 'pi.auth.respond': {
      if (typeof params.value !== 'string') throw new Error('value must be a string');
      return piAuth.respond(text(params, 'loginId'), text(params, 'promptId'), params.value);
    }
    case 'pi.auth.cancel': return piAuth.cancel(text(params, 'loginId'));
    case 'pi.auth.logout': return piAuth.logout(text(params, 'providerId'));
    case 'initialize': {
      return { version: runtimeInfo.version ?? 'unknown', runtimeInfo, dataDir, remoteCommands: remoteCommands(dataDir), piAgentDir: getAgentDir(), hivePort: hive.port, navigation: hive.getNavigation() };
    }
    case 'desktop.navigation.save': {
      const sessions = record(params.sessions);
      if (!(params.workspace === null || typeof params.workspace === 'string')
        || !Object.values(sessions).every(id => typeof id === 'string')) throw new Error('Invalid workspace selection');
      const value = JSON.stringify({ workspace: params.workspace, sessions });
      hive.saveNavigation(value);
      return { ok: true };
    }
    case 'team.member.create': { exact(params, ['workspaceId', 'name', 'role', 'backend', 'description', 'profile', 'model']); return runtime.createMember(text(params, 'workspaceId'), { name: params.name, role: params.role, backend: params.backend, ...optionalProfile(params), ...optionalModel(params),
      ...(params.description !== undefined && { description: params.description }) }); }
    case 'member.worktree.create': {
      exact(params, ['workspaceId', 'memberId', 'backend', 'branch', 'skipSetup', 'expectedSetupRevision', 'profile']);
      if (params.backend !== undefined && !['pi','codex','pi-rpc','claude','acp'].includes(params.backend as string)) throw new Error('Unsupported chat backend');
      if (params.branch !== undefined && typeof params.branch !== 'string') throw new Error('Branch must be text');
      if (params.expectedSetupRevision !== undefined && typeof params.expectedSetupRevision !== 'string') throw new Error('Invalid setup revision');
      if (params.skipSetup !== undefined && typeof params.skipSetup !== 'boolean') throw new Error('Invalid setup selection');
      return chats.createMemberWorktree(text(params, 'workspaceId'), text(params, 'memberId'), { ...optionalProfile(params),
        ...(params.skipSetup !== undefined && {skipSetup: params.skipSetup as boolean}), ...(params.expectedSetupRevision !== undefined && {expectedSetupRevision: params.expectedSetupRevision as string}),
        ...(params.backend !== undefined && {backend: params.backend as 'pi' | 'codex' | 'pi-rpc' | 'claude' | 'acp'}),
        ...(typeof params.branch === 'string' && {branch: params.branch})});
    }
    case 'member.worktrees': return chats.memberWorktrees(text(params, 'workspaceId'), text(params, 'memberId'));
    case 'team.members': {
      const workspace = text(params, 'workspaceId');
      const members = await hive.request('GET', `/api/ui/workspaces/${encodeURIComponent(workspace)}/team`) as { id: string; status?: string }[];
      return members.map(member => {
        const desktop_activity = chats.describeMemberActivity(workspace, member.id, member.status);
        const launch = runtime.agents.peekAgentLaunchConfig(workspace, member.id);
        if (!launch || !structuredCommand(launch.command)) return {...member, desktop_activity};
        try {
          const parsed = parseStructuredLaunch(launch);
          const profile = parsed.profile.kind === 'explicit' ? parsed.profile.ref : {id: parsed.profile.id, revision: 1};
          return {...member, desktop_activity, desktop_profile: profile, ...(parsed.model && {desktop_model: parsed.model})};
        } catch (error) { return {...member, desktop_activity, desktop_launch_error: error instanceof Error ? error.message : String(error)}; }

      });
    }
    case 'agent.models': { exact(params, ['workspaceId', 'backend', 'profile']); return agentModels.list(text(params, 'workspaceId'), params.backend, optionalProfile(params).profile); }
    case 'desktop.settings.codexLimits': { exact(params, ['command', 'profile']); if (params.profile !== undefined && params.command !== undefined) throw new Error('Select a saved profile or an executable draft'); return settings.codexLimits(params.command, optionalProfile(params).profile); }
    case 'desktop.settings.codexAccount': { exact(params, ['command', 'profile']); if (params.profile !== undefined && params.command !== undefined) throw new Error('Select a saved profile or an executable draft'); return settings.codexAccount(params.command, optionalProfile(params).profile); }
    case 'desktop.settings.probe': { exact(params, ['backend', 'command', 'profile']); if (params.profile !== undefined && params.command !== undefined) throw new Error('Select a saved profile or an executable draft'); return settings.probe(params.backend, params.command, optionalProfile(params).profile); }
    case 'desktop.settings.get': return settings.get();
    case 'desktop.readiness': {
      exact(params, ['workspaceId']);
      let workspacePath: string | undefined;
      if (typeof params.workspaceId === 'string' && params.workspaceId.trim()) {
        try {
          const workspaces = await hive.request('GET', '/api/workspaces') as { id: string; directory?: string; path?: string }[];
          const ws = workspaces.find(w => w.id === params.workspaceId);
          workspacePath = ws?.directory ?? ws?.path;
        } catch {
          // Best-effort project-level MCP scan; global readiness still succeeds.
        }
      }
      return collectReadinessReport({ settings, profiles, ...(workspacePath ? { workspacePath } : {}) });
    }
    case 'desktop.skillsAndRules': {
      let workspacePath: string | undefined;
      if (typeof params.workspaceId === 'string' && params.workspaceId.trim()) {
        try {
          const workspaces = await runtime.requestHive('GET', '/api/workspaces') as Array<{ id: string; directory?: string; path?: string }>;
          const ws = workspaces.find(w => w.id === params.workspaceId);
          workspacePath = ws?.directory ?? ws?.path;
        } catch {}
      }
      return collectSkillsAndRules({ ...(workspacePath ? { workspacePath } : {}) });
    }
    case 'desktop.usageSummary': {
      const wsId = typeof params.workspaceId === 'string' && params.workspaceId.trim() ? params.workspaceId.trim() : undefined;
      return chats.localUsageSummary(wsId);
    }
    case 'mcp.probe': {
      exact(params, ['workspaceId', 'name', 'scope', 'configPath', 'timeoutMs']);
      const name = text(params, 'name');
      let workspacePath: string | undefined;
      if (typeof params.workspaceId === 'string' && params.workspaceId.trim()) {
        try {
          const workspaces = await runtime.requestHive('GET', '/api/workspaces') as Array<{ id: string; directory?: string; path?: string }>;
          const ws = workspaces.find(w => w.id === params.workspaceId);
          workspacePath = ws?.directory ?? ws?.path;
        } catch {}
      }
      const target = resolveMcpServerTarget({
        name,
        ...(params.scope === 'global' || params.scope === 'project' ? { scope: params.scope } : {}),
        ...(typeof params.configPath === 'string' && params.configPath.trim() ? { configPath: params.configPath.trim() } : {}),
        ...(workspacePath ? { workspacePath } : {}),
      });
      if (!target) {
        return {
          name,
          ok: false,
          status: 'error',
          latencyMs: 0,
          tools: [],
          error: `找不到 MCP 服务 "${name}" 的配置项`,
        };
      }
      const timeoutMs = typeof params.timeoutMs === 'number' && Number.isFinite(params.timeoutMs) ? params.timeoutMs : undefined;
      return probeMcpServer(target, timeoutMs);
    }
    case 'mcp.upsertProjectServer': {
      exact(params, ['workspaceId', 'name', 'transport', 'command', 'args', 'url', 'env', 'disabled']);
      const workspaceId = text(params, 'workspaceId');
      chats.assertWorkspaceAvailable(workspaceId);
      const workspaces = await runtime.requestHive('GET', '/api/workspaces') as Array<{ id: string; directory?: string; path?: string }>;
      const ws = workspaces.find(w => w.id === workspaceId);
      const workspacePath = ws?.directory ?? ws?.path;
      if (!workspacePath) throw new Error('找不到目标项目目录');
      const transport = params.transport === 'http' ? 'http' : 'stdio';
      const env = params.env && typeof params.env === 'object' && !Array.isArray(params.env)
        ? Object.fromEntries(Object.entries(params.env as Record<string, unknown>).map(([k, v]) => [k, String(v)]))
        : undefined;
      return upsertProjectMcpServer(workspacePath, {
        name: text(params, 'name'),
        transport,
        ...(typeof params.command === 'string' ? { command: params.command } : {}),
        ...(Array.isArray(params.args) ? { args: params.args.map(String) } : {}),
        ...(typeof params.url === 'string' ? { url: params.url } : {}),
        ...(env ? { env } : {}),
        ...(typeof params.disabled === 'boolean' ? { disabled: params.disabled } : {}),
      });
    }
    case 'mcp.toggleProjectServer': {
      exact(params, ['workspaceId', 'name', 'disabled']);
      const workspaceId = text(params, 'workspaceId');
      chats.assertWorkspaceAvailable(workspaceId);
      if (typeof params.disabled !== 'boolean') throw new TypeError('disabled must be a boolean');
      const workspaces = await runtime.requestHive('GET', '/api/workspaces') as Array<{ id: string; directory?: string; path?: string }>;
      const ws = workspaces.find(w => w.id === workspaceId);
      const workspacePath = ws?.directory ?? ws?.path;
      if (!workspacePath) throw new Error('找不到目标项目目录');
      return toggleProjectMcpServer(workspacePath, text(params, 'name'), params.disabled);
    }
    case 'mcp.removeProjectServer': {
      exact(params, ['workspaceId', 'name']);
      const workspaceId = text(params, 'workspaceId');
      chats.assertWorkspaceAvailable(workspaceId);
      const workspaces = await runtime.requestHive('GET', '/api/workspaces') as Array<{ id: string; directory?: string; path?: string }>;
      const ws = workspaces.find(w => w.id === workspaceId);
      const workspacePath = ws?.directory ?? ws?.path;
      if (!workspacePath) throw new Error('找不到目标项目目录');
      return removeProjectMcpServer(workspacePath, text(params, 'name'));
    }
    case 'desktop.settings.update': return settings.update(params);
    case 'worktree.initialization.get':
    case 'worktree.initialization.retry':
    case 'worktree.initialization.skip': {
      const mutation = method !== 'worktree.initialization.get';
      if (Object.keys(params).some(key => !['workspaceId', 'sessionId', ...(mutation ? ['expectedVersion'] : [])].includes(key))) throw new WorktreeInitializationError('Supply only the original session and expected setup state version.');
      const workspaceId = text(params, 'workspaceId'), sessionId = text(params, 'sessionId');
      return method === 'worktree.initialization.get' ? runtime.initialization.get(workspaceId, sessionId)
        : method === 'worktree.initialization.retry' ? runtime.initialization.retry(workspaceId, sessionId, text(params, 'expectedVersion'))
        : runtime.initialization.skip(workspaceId, sessionId, text(params, 'expectedVersion'));
    }
    case 'script.settings.list': return runtime.projectScripts.list(text(params, 'workspaceId'));
    case 'script.settings.save': return runtime.projectScripts.save(text(params, 'workspaceId'), text(params, 'expectedRevision'), record(params.script) as {id?: string; name: unknown; command: unknown});
    case 'script.settings.remove': return runtime.projectScripts.remove(text(params, 'workspaceId'), text(params, 'expectedRevision'), text(params, 'scriptId'));
    case 'script.run': return runtime.scriptRuns.run(params);
    case 'script.runs': return runtime.scriptRuns.list(text(params, 'workspaceId'), params.requestId);
    case 'script.get': return runtime.scriptRuns.get(text(params, 'workspaceId'), text(params, 'runId'), params.offset as number | undefined, params.limit as number | undefined);
    case 'script.stop': return runtime.scriptRuns.stop(text(params, 'workspaceId'), text(params, 'runId'));
    case 'terminal.list': return terminal.list(text(params, 'workspaceId'), params.sessionId === undefined ? undefined : text(params, 'sessionId'));
    case 'terminal.start': return terminal.start(text(params, 'workspaceId'), params.sessionId === undefined ? undefined : text(params, 'sessionId'));
    case 'terminal.attach': return terminal.attach({ ...(params.sessionId === undefined ? {} : {sessionId: text(params, 'sessionId')}), attachmentId: text(params, 'attachmentId'), workspaceId: text(params, 'workspaceId'), runId: text(params, 'runId'), cols: params.cols as number, rows: params.rows as number });
    case 'terminal.restoreComplete': return terminal.restoreComplete({ attachmentId: text(params, 'attachmentId'), sequence: params.sequence as number });
    case 'terminal.ack': return terminal.ack({ attachmentId: text(params, 'attachmentId'), sequence: params.sequence as number, bytes: params.bytes as number });
    case 'terminal.input': return terminal.input({ attachmentId: text(params, 'attachmentId'), bytes: params.bytes, ...(params.userInput !== undefined && { userInput: params.userInput as boolean }) });
    case 'terminal.resize': return terminal.resize({ attachmentId: text(params, 'attachmentId'), cols: params.cols as number, rows: params.rows as number });
    case 'terminal.detach': return terminal.detach({ attachmentId: text(params, 'attachmentId') });
    case 'terminal.stop': return terminal.stop({ ...(params.sessionId === undefined ? {} : {sessionId: text(params, 'sessionId')}), workspaceId: text(params, 'workspaceId'), runId: text(params, 'runId') });
    case 'workspace.saveTasks': {
      const workspace = text(params, 'workspaceId');
      if (typeof params.base !== 'string' || typeof params.content !== 'string') throw new Error('Expected task document text');
      const base_sha256 = createHash('sha256').update(params.base).digest('hex');
      try {
        return await runtime.requestHive('PUT', `/api/workspaces/${encodeURIComponent(workspace)}/tasks`, { base_sha256, content: params.content });
      } catch (error) {
        if (error instanceof HiveHttpError && error.status === 409 && error.body && typeof error.body === 'object'
          && 'code' in error.body && error.body.code === 'TASKS_CHANGED'
          && 'current_content' in error.body && typeof error.body.current_content === 'string') {
          return { conflict: true, currentContent: error.body.current_content };
        }
        throw error;
      }
    }
    case 'workspace.remove': {
      try { return await runtime.removeWorkspace(text(params, 'workspaceId')); }
      catch (error) {
        if (error instanceof WorkspaceRemovalError) return { ok: false, error: { code: error.code, message: error.message, details: error.details } };
        throw error;
      }
    }
    case 'workspace.scratch.promote': {
      exact(params, ['workspaceId', 'sessionId', 'name']);
      return chats.promoteScratchProject(text(params, 'workspaceId'), text(params, 'sessionId'), text(params, 'name'), runtime.requestHive);
    }
    case 'workspace.scratch.links': exact(params, []); return chats.listScratchProjectLinks();
    case 'hive.request': {
      const method = text(params, 'method');
      const path = text(params, 'path');
      const result = await runtime.requestHive(method, path, params.body);
      const inbox = method === 'GET' && /^\/api\/ui\/workspaces\/([^/?#]+)\/action-center$/.exec(path);
      return inbox ? withInboxDispatchBackends(result, decodeURIComponent(inbox[1]!),
        (workspaceId, dispatchId) => chats.inboxDispatchBackend(workspaceId, dispatchId)) : result;
    }
    case 'inbox.actionableQuestions': {
      const ids = params.workspaceIds;
      if (!Array.isArray(ids) || ids.length > 32 || !ids.every(id => typeof id === 'string' && id.length <= 64)) {
        throw new Error('Expected up to 32 inbox workspace identities');
      }
      return runtime.hive.inboxQuestions(ids);
    }
    case 'chat.pinned': return chats.pinned();
    case 'chat.pins.move': {
      exact(params, ['source', 'target', 'position', 'expectedOrder']);
      const reference = (value: unknown) => {
        const ref = record(value);
        exact(ref, ['workspaceId', 'sessionId']);
        return {workspaceId: text(ref, 'workspaceId'), sessionId: text(ref, 'sessionId')};
      };
      const {position, expectedOrder} = params;
      if (position !== 'before' && position !== 'after') throw new TypeError('Expected a before or after pin position');
      if (!Array.isArray(expectedOrder) || !expectedOrder.every(id => typeof id === 'string' && id.trim())
        || new Set(expectedOrder).size !== expectedOrder.length) throw new TypeError('Expected the ordered IDs of the visible pins');
      return chats.movePinned(reference(params.source), reference(params.target), position, expectedOrder);
    }
    case 'chat.attachments.add': {
      const workspace = text(params, 'workspaceId');
      chats.assertWorkspaceAvailable(workspace);
      return chats.attachments.add(workspace, params.path);
    }
    case 'chat.attachments.path': {
      const items = await chats.attachments.get(text(params, 'workspaceId'), [text(params, 'id')]);
      if (!items[0]?.filePath) throw new Error('请使用图片查看器查看图片。');
      return {path:items[0].filePath};
    }
    case 'chat.attachments.get': return chats.attachments.get(text(params, 'workspaceId'), params.ids);
    case 'artifact.resolve': {
      const {workspaceId: _workspaceId, ...source} = params;
      return chats.resolveArtifact(text(params, 'workspaceId'), source);
    }
    case 'artifact.htmlSnapshot': {
      const {workspaceId: _workspaceId, ...source} = params;
      return chats.artifactHtmlSnapshot(text(params, 'workspaceId'), source);
    }
    case 'artifact.htmlSavedPath': {
      const {workspaceId: _workspaceId, check, ...source} = params;
      if (check !== undefined && typeof check !== 'boolean') throw new TypeError('check must be boolean');
      return chats.artifactHtmlSavedPath(text(params, 'workspaceId'), source, check === true);
    }
    case 'dispatch.session': return chats.dispatchSession(text(params, 'workspaceId'), params.dispatchId, params.sourceAgentId);
    case 'chat.attachments.clipboard': {
      const workspace = text(params, 'workspaceId');
      chats.assertWorkspaceAvailable(workspace);
      return chats.attachments.addClipboard(workspace, params.image);
    }
    case 'git.status': return chats.gitStatus(text(params, 'workspaceId'), params.offset, params.limit, params.sessionId);
    case 'git.branches': return chats.gitBranches(text(params, 'workspaceId'), params.sessionId);
    case 'git.checkoutBranch': return chats.gitCheckoutBranch(
      text(params, 'workspaceId'),
      text(params, 'branch'),
      params.create === true,
      typeof params.startPoint === 'string' ? params.startPoint : undefined,
      params.sessionId,
    );
    case 'git.fetch': return chats.gitFetch(
      text(params, 'workspaceId'),
      typeof params.remote === 'string' ? params.remote : undefined,
      params.prune !== false,
      params.sessionId,
    );
    case 'git.pull': return chats.gitPull(
      text(params, 'workspaceId'),
      params.rebase === true,
      params.sessionId,
    );
    case 'git.stashList': return chats.gitStashList(text(params, 'workspaceId'), params.sessionId);
    case 'git.stashPush': return chats.gitStashPush(
      text(params, 'workspaceId'),
      typeof params.message === 'string' ? params.message : undefined,
      params.includeUntracked !== false,
      params.sessionId,
    );
    case 'git.stashPop': return chats.gitStashPop(text(params, 'workspaceId'), text(params, 'ref'), params.sessionId);
    case 'git.stashDrop': return chats.gitStashDrop(text(params, 'workspaceId'), text(params, 'ref'), params.sessionId);
    case 'git.discardFile': return chats.gitDiscardFile(text(params, 'workspaceId'), text(params, 'path'), params.sessionId);
    case 'git.log': return chats.gitLog(
      text(params, 'workspaceId'),
      typeof params.limit === 'number' && Number.isFinite(params.limit) ? params.limit : undefined,
      params.sessionId,
    );
    case 'git.commitDiff': return chats.gitCommitDiff(text(params, 'workspaceId'), text(params, 'oid'), params.sessionId);
    case 'git.diff': return chats.gitDiff(text(params, 'workspaceId'), text(params, 'path'), params.comparison, params.sessionId);
    case 'git.index.preview': return chats.gitIndexPreview(text(params, 'workspaceId'), params.operation, params.paths, params.sessionId);
    case 'git.index.apply': return chats.gitIndexApply(text(params, 'workspaceId'), params.operation, params.paths, params.expectedVersion, params.sessionId);
    case 'git.commit.preview': return chats.gitCommitPreview(text(params, 'workspaceId'), params.sessionId, params.includePatch, params.expectedVersion);
    case 'git.commit.apply': return chats.gitCommitApply(text(params, 'workspaceId'), params.expectedVersion, params.message, params.includeOutsideProject, params.sessionId);
    case 'git.remotes': return chats.gitRemotes(text(params, 'workspaceId'), params.sessionId);
    case 'git.push.preview': return chats.gitPushPreview(text(params, 'workspaceId'), params.remote, params.branch, params.sessionId);
    case 'git.push.apply': return chats.gitPushApply(text(params, 'workspaceId'), params.remote, params.branch, params.expectedVersion, params.sessionId);
    case 'git.pr.review.preview': {
      exact(params, ['workspaceId', 'remote', 'reference']);
      return chats.gitPrReviewPreview(text(params, 'workspaceId'), text(params, 'remote'), text(params, 'reference'));
    }
    case 'git.pr.review.open': {
      exact(params, ['workspaceId', 'remote', 'reference', 'expectedVersion', 'checkout', 'backend', 'profile']);
      const {checkout, backend} = params;
      if (checkout !== 'current' && checkout !== 'worktree') throw new Error('Unsupported PR checkout');
      if (backend !== 'pi' && backend !== 'codex' && backend !== 'pi-rpc' && backend !== 'claude' && backend !== 'acp')
        throw new Error('Unsupported chat backend');
      return chats.openGithubPrReview(text(params, 'workspaceId'), {checkout, backend, ...optionalProfile(params),
        remote: text(params, 'remote'), reference: text(params, 'reference'), expectedVersion: text(params, 'expectedVersion')},
      () => terminal.hasAnyActiveShell());
    }
    case 'git.pr.preview': return chats.gitPrPreview(text(params, 'workspaceId'), params);
    case 'git.pr.create': return chats.gitPrApply(text(params, 'workspaceId'), 'create', params);
    case 'git.pr.update': return chats.gitPrApply(text(params, 'workspaceId'), 'update', params);
    case 'project.files.list': return chats.projectFilesList(text(params, 'workspaceId'), params.directory, params.offset, params.sessionId);
    case 'project.files.search': return runSearch(requestId, signal => chats.projectFilesSearch(text(params, 'workspaceId'), params.query, params.offset, params.sessionId, signal, params.caseSensitive, params.regex));
    case 'project.files.searchContent': return runSearch(requestId, signal => chats.projectFilesSearchContent(text(params, 'workspaceId'), params.query, params.offset, params.sessionId, signal, params.caseSensitive, params.wholeWord, params.regex));
    case 'project.files.preview': return chats.projectFilesPreview(text(params, 'workspaceId'), params.relativePath, params.sessionId);
    case 'project.files.resolveOpen': return chats.projectFilesResolveOpen(text(params, 'workspaceId'), params.relativePath, params.sessionId);
    case 'project.files.attach': return chats.projectFilesAttach(text(params, 'workspaceId'), params.relativePath, params.sessionId, params.expectedSha256);
    case 'project.files.status': return chats.projectFileStatus(text(params, 'workspaceId'), text(params, 'id'));
    case 'chat.search': {
      if (params.navigationOnly !== undefined && typeof params.navigationOnly !== 'boolean')
        throw new TypeError('navigationOnly must be boolean');
      const limit = params.limit;
      const memberId = params.memberId === undefined ? undefined : text(params, 'memberId');
      const backend = params.backend;
      const archiveSnapshot = params.archiveSnapshot;
      const activitySinceMs = params.activitySinceMs;
      if (backend !== undefined && !isAgentDriver(backend)) throw new TypeError('Unsupported chat backend');
      if (archiveSnapshot !== undefined
        && (typeof archiveSnapshot !== 'string' || !/^[\da-f]{64}$/.test(archiveSnapshot)))
        throw new TypeError('Invalid archive snapshot');
      if (activitySinceMs !== undefined
        && (typeof activitySinceMs !== 'number' || !Number.isSafeInteger(activitySinceMs) || activitySinceMs < 0))
        throw new TypeError('Invalid inbox activity cutoff');
      if ((limit !== undefined || memberId !== undefined || backend !== undefined || archiveSnapshot !== undefined || activitySinceMs !== undefined)
        && (params.archived !== true || params.navigationOnly !== true || params.query !== ''))
        throw new TypeError('Scoped archive paging requires an empty archived navigation query');
      if (limit !== undefined && (typeof limit !== 'number' || !Number.isSafeInteger(limit) || limit < 1 || limit > 100))
        throw new TypeError('Invalid archive page size');
      if (memberId !== undefined) text(params, 'workspaceId');
      return runSearch(requestId, async signal => {
        const page = await chats.search(params.query, params.archived, params.offset, params.workspaceId, signal,
          {...(limit !== undefined && {limit}), ...(memberId !== undefined && {memberId}),
            ...(backend !== undefined && {backend}), ...(archiveSnapshot !== undefined && {archiveSnapshot}),
            ...(activitySinceMs !== undefined && {activitySinceMs})});
        if (!params.navigationOnly) return page;
        return {...page, matches: page.matches.map(hit => ({...hit,
          session: navigationSearchSession(hit.session, activitySinceMs !== undefined)}))};
      });
    }
    case 'chat.searchMessages': {
      exact(params, ['sessionId', 'viewRevision', 'query']);
      return runSearch(requestId, signal => chats.searchMessages(text(params, 'sessionId'), viewRevision(params.viewRevision), params.query, signal));
    }
    case 'chat.search.cancel':
    case 'project.files.search.cancel': {
      exact(params, ['requestId']);
      const target = params.requestId;
      if (typeof target !== 'number' && typeof target !== 'string') throw new Error('Search request id is required');
      const search = searches.get(target);
      search?.abort();
      return {cancelled: search !== undefined};
    }
    case 'chat.list': return chats.list(text(params, 'workspaceId'));
    case 'chat.reference.resolve': {
      exact(params, ['workspaceId', 'sessionId']);
      const session = chats.conversationReference(text(params, 'workspaceId'), text(params, 'sessionId'));
      return session ? navigationSearchSession(session) : null;
    }
    case 'chat.execution': return chats.sessionExecution(text(params, 'sessionId'));
    case 'chat.worktrees.pending': return chats.pendingWorktrees(text(params, 'workspaceId'));
    case 'chat.worktrees.release': {
      exact(params, ['workspaceId', 'sessionId', 'expectedOwnerSessionId']);
      const workspaceId = text(params, 'workspaceId'), id = text(params, 'sessionId');
      return chats.releaseWorktreeUse(workspaceId, id, text(params, 'expectedOwnerSessionId'),
        () => terminal.hasActiveSessionShell(workspaceId, id));
    }
    case 'chat.worktrees.removal.preview':
    case 'chat.worktrees.removal.apply': {
      const apply = method === 'chat.worktrees.removal.apply';
      exact(params, ['workspaceId', 'sessionId', ...(apply ? ['expectedVersion'] : [])]);
      const workspaceId = text(params, 'workspaceId'), id = text(params, 'sessionId');
      const hasShell = () => terminal.hasActiveShell(workspaceId);
      return apply ? chats.applyWorktreeRemoval(workspaceId, id, text(params, 'expectedVersion'), hasShell)
        : chats.previewWorktreeRemoval(workspaceId, id, hasShell);
    }
    case 'chat.worktrees.recover': {
      exact(params, ['workspaceId', 'sessionId', 'backend', 'memberId', 'profile']);
      if (params.backend !== undefined && params.backend !== 'pi' && params.backend !== 'codex' && params.backend !== 'pi-rpc' && params.backend !== 'claude' && params.backend !== 'acp') throw new Error('Unsupported chat backend');
      return chats.recoverWorktreeRecord(text(params, 'workspaceId'), text(params, 'sessionId'), params.backend, params.memberId === undefined ? undefined : text(params, 'memberId'), optionalProfile(params).profile);
    }
    case 'chat.createInWorktreeRecord':
    case 'chat.createFreshWorktreeDraftRecord': {
      exact(params, ['workspaceId', 'sourceSessionId', 'requestId']);
      const requestId = text(params, 'requestId').toLowerCase();
      if (!/^[a-f0-9]{8}-[a-f0-9]{4}-4[a-f0-9]{3}-[89ab][a-f0-9]{3}-[a-f0-9]{12}$/.test(requestId)) {
        throw new TypeError('A UUIDv4 request identity is required');
      }
      const workspace = text(params, 'workspaceId'), source = text(params, 'sourceSessionId');
      return method === 'chat.createFreshWorktreeDraftRecord' ? chats.createFreshWorktreeDraftRecord(workspace, source, requestId)
        : chats.createInWorktreeRecord(workspace, source, requestId);
    }
    case 'chat.createScratchRecord': {
      exact(params, ['backend', 'profile', 'requestId']);
      if (!isAgentDriver(params.backend)) throw new TypeError('Unsupported chat backend');
      const requestId = text(params, 'requestId').toLowerCase();
      if (!/^[a-f0-9]{8}-[a-f0-9]{4}-4[a-f0-9]{3}-[89ab][a-f0-9]{3}-[a-f0-9]{12}$/.test(requestId)) {
        throw new TypeError('A UUIDv4 request identity is required');
      }
      return chats.createScratchRecord(params.backend, optionalProfile(params).profile, requestId,
        () => scratchWorkspace(dataDir, runtime.requestHive));
    }
    case 'chat.copyDraftRecord': {
      exact(params, ['requestId', 'workspaceId', 'sourceSessionId', 'targetWorkspaceId', 'backend', 'profile', 'attachmentIds']);
      if (!isAgentDriver(params.backend)) throw new TypeError('Unsupported chat backend');
      const requestId = text(params, 'requestId').toLowerCase();
      if (!/^[a-f0-9]{8}-[a-f0-9]{4}-4[a-f0-9]{3}-[89ab][a-f0-9]{3}-[a-f0-9]{12}$/.test(requestId)) {
        throw new TypeError('A UUIDv4 request identity is required');
      }
      if (!Array.isArray(params.attachmentIds)) throw new TypeError('attachmentIds is required');
      return chats.copyDraftRecord({requestId, workspaceId: text(params, 'workspaceId'),
        ...(params.sourceSessionId !== undefined && {sourceSessionId: text(params, 'sourceSessionId')}),
        targetWorkspaceId: text(params, 'targetWorkspaceId'), backend: params.backend, ...optionalProfile(params),
        attachmentIds: attachmentIds(params.attachmentIds)});
    }
    case 'chat.createRecord':
    case 'chat.create': {
      exact(params, ['workspaceId', 'backend', 'profile', 'isolation', 'branch', 'skipSetup', 'expectedSetupRevision', ...(method === 'chat.createRecord' ? ['deferFirstConnection'] : [])]);
      if (params.backend !== undefined && params.backend !== 'pi' && params.backend !== 'codex' && params.backend !== 'pi-rpc' && params.backend !== 'claude' && params.backend !== 'acp') throw new Error('Unsupported chat backend');
      if (params.isolation !== undefined && params.isolation !== 'worktree') throw new Error('Unsupported session isolation');
      if (params.branch !== undefined && (params.isolation !== 'worktree' || typeof params.branch !== 'string')) throw new Error('A branch is only valid for a new worktree session');
      const workspace = text(params, 'workspaceId');
      if (params.expectedSetupRevision !== undefined && typeof params.expectedSetupRevision !== 'string') throw new Error('Invalid setup revision');
      if (params.skipSetup !== undefined && typeof params.skipSetup !== 'boolean') throw new Error('Invalid setup selection');
      if (params.deferFirstConnection !== undefined && typeof params.deferFirstConnection !== 'boolean') throw new TypeError('deferFirstConnection must be a boolean');
      const options = {...optionalProfile(params), ...(params.skipSetup !== undefined && {skipSetup: params.skipSetup as boolean}), ...(params.expectedSetupRevision !== undefined && {expectedSetupRevision: params.expectedSetupRevision as string}), ...(params.isolation === 'worktree' && {isolation: 'worktree' as const}), ...(typeof params.branch === 'string' && {branch: params.branch}), ...(params.deferFirstConnection === true && {deferFirstConnection: true})};
      return method === 'chat.createRecord' ? chats.createRecord(workspace, params.backend, options) : chats.create(workspace, params.backend, options);
    }
    case 'chat.pin': {
      const session = chats.setPinned(text(params, 'sessionId'), params.pinned);
      return { session, sessions: chats.list(session.workspaceId) };
    }
    case 'chat.archive': {
      const undo = params.undo === undefined ? undefined : record(params.undo);
      if (undo) exact(undo, ['workspaceId', 'version']);
      const inbox = params.inbox === undefined ? undefined : record(params.inbox);
      if (inbox) {
        exact(inbox, ['workspaceId', 'replyRevision']);
        if (typeof inbox.replyRevision !== 'number' || !Number.isSafeInteger(inbox.replyRevision) || inbox.replyRevision < 0)
          throw new TypeError('Invalid inbox reply revision');
      }
      const result = await chats.changeArchive(text(params, 'sessionId'), params.archived,
        undo && {workspaceId: text(undo, 'workspaceId'), version: viewRevision(undo.version)},
        inbox && {workspaceId: text(inbox, 'workspaceId'), replyRevision: inbox.replyRevision as number},
        params.restoreWorkspaceId === undefined ? undefined : text(params, 'restoreWorkspaceId'));
      return { ...result, sessions: chats.list(result.session.workspaceId) };
    }
    case 'chat.rename': return chats.rename(text(params, 'sessionId'), params.title);
    case 'chat.open': {
      if (params.includeLiveHistory !== undefined && typeof params.includeLiveHistory !== 'boolean') {
        throw new TypeError('includeLiveHistory must be a boolean');
      }
      if (params.preferSavedHistory !== undefined && typeof params.preferSavedHistory !== 'boolean') {
        throw new TypeError('preferSavedHistory must be a boolean');
      }
      if (params.deferHistoryImages !== undefined && typeof params.deferHistoryImages !== 'boolean') {
        throw new TypeError('deferHistoryImages must be a boolean');
      }
      if (params.historyLimit !== undefined && (typeof params.historyLimit !== 'number' || !Number.isSafeInteger(params.historyLimit) || params.historyLimit <= 0)) {
        throw new TypeError('historyLimit must be a positive integer');
      }
      if (params.historyAroundMessageId !== undefined && typeof params.historyAroundMessageId !== 'string') {
        throw new TypeError('historyAroundMessageId must be a string');
      }
      const id = text(params, 'sessionId');
      const historyOptions = params.historyLimit !== undefined && params.includeLiveHistory !== false ? {
        limit: params.historyLimit,
        ...(params.historyAroundMessageId ? {aroundMessageId: params.historyAroundMessageId} : {}),
      } : undefined;
      const snapshot = await (params.viewRevision === undefined
        ? chats.open(id, params.includeLiveHistory, false, historyOptions)
        : chats.openDesktopView(id, viewRevision(params.viewRevision), params.includeLiveHistory, historyOptions, params.preferSavedHistory));
      const page = historyOptions ? ('historyPage' in snapshot && snapshot.historyPage
        ? {history: snapshot.history, historyMessageIds: snapshot.historyMessageIds, historyPage: snapshot.historyPage}
        : sliceHistoryWindow(snapshot.history, snapshot.historyMessageIds, historyOptions)) : undefined;
      const paged = page ? {...snapshot, ...page, history: page.history.map(message => boundOversizedToolOutput(message))} : snapshot;
      return params.deferHistoryImages ? {...paged, history: paged.history.map(
        (message, index) => desktopHistoryMessage(message, paged.historyMessageIds[index])
      )} : paged;
    }
    case 'chat.historyPage': {
      exact(params, ['sessionId', 'beforeIndex', 'beforeMessageId', 'afterIndex', 'afterMessageId', 'limit', 'deferHistoryImages', 'viewRevision', 'snapshot']);
      const before = params.beforeIndex !== undefined;
      const after = params.afterIndex !== undefined;
      if (before === after || (!before && params.beforeMessageId !== undefined) || (!after && params.afterMessageId !== undefined)) {
        throw new TypeError('Provide either a before or an after history cursor');
      }
      const indexName = before ? 'beforeIndex' : 'afterIndex';
      const idName = before ? 'beforeMessageId' : 'afterMessageId';
      const index = params[indexName];
      const messageId = params[idName];
      if (typeof index !== 'number' || !Number.isSafeInteger(index) || index < 0) {
        throw new TypeError(`${indexName} must be a nonnegative integer`);
      }
      if (messageId !== undefined && typeof messageId !== 'string') {
        throw new TypeError(`${idName} must be a string`);
      }
      const limit = params.limit;
      if (limit !== undefined && (typeof limit !== 'number' || !Number.isSafeInteger(limit) || limit <= 0)) {
        throw new TypeError('limit must be a positive integer');
      }
      if (params.deferHistoryImages !== undefined && typeof params.deferHistoryImages !== 'boolean') {
        throw new TypeError('deferHistoryImages must be a boolean');
      }
      const id = text(params, 'sessionId');
      const snapshot = params.snapshot;
      if (snapshot !== undefined && (typeof snapshot !== 'string' || !snapshot || snapshot.length > 256)) {
        throw new TypeError('snapshot must be a nonempty history snapshot token');
      }
      const read = () => chats.historyPage(id, {
        ...(before
          ? { beforeIndex: index, ...(messageId && { beforeMessageId: messageId }) }
          : { afterIndex: index, ...(messageId && { afterMessageId: messageId }) }),
        ...(limit !== undefined && { limit }),
        ...(snapshot !== undefined && { snapshot }),
      });
      const page = await (params.viewRevision === undefined
        ? read()
        : chats.readDesktopView(id, viewRevision(params.viewRevision), read));
      const boundedHistory = page.history.map(m => boundOversizedToolOutput(m));
      return params.deferHistoryImages ? {
        ...page,
        history: boundedHistory.map((message, index) => desktopHistoryMessage(message, page.historyMessageIds[index])),
      } : { ...page, history: boundedHistory };
    }
    case 'chat.toolOutputSlice': {
      exact(params, ['sessionId', 'messageId', 'blockIndex', 'byteOffset', 'byteLength']);
      if (params.blockIndex !== undefined && (typeof params.blockIndex !== 'number' || !Number.isSafeInteger(params.blockIndex) || params.blockIndex < 0)) {
        throw new TypeError('blockIndex must be a nonnegative integer');
      }
      if (params.byteOffset !== undefined && (typeof params.byteOffset !== 'number' || !Number.isSafeInteger(params.byteOffset) || params.byteOffset < 0)) {
        throw new TypeError('byteOffset must be a nonnegative integer');
      }
      if (params.byteLength !== undefined && (typeof params.byteLength !== 'number' || !Number.isSafeInteger(params.byteLength) || params.byteLength <= 0)) {
        throw new TypeError('byteLength must be a positive integer');
      }
      return chats.toolOutputSlice(text(params, 'sessionId'), {
        messageId: text(params, 'messageId'),
        ...(params.blockIndex !== undefined ? { blockIndex: params.blockIndex } : {}),
        ...(params.byteOffset !== undefined ? { byteOffset: params.byteOffset } : {}),
        ...(params.byteLength !== undefined ? { byteLength: params.byteLength } : {}),
      });
    }
    case 'chat.clearView': {
      exact(params, ['viewRevision']);
      return chats.clearDesktopView(viewRevision(params.viewRevision));
    }
    case 'chat.historyImage': {
      exact(params, ['workspaceId', 'sessionId', 'messageId', 'imageIndex']);
      if (typeof params.imageIndex !== 'number' || !Number.isSafeInteger(params.imageIndex) || params.imageIndex < 0) {
        throw new TypeError('imageIndex must be a nonnegative integer');
      }
      return chats.historyImage(text(params, 'workspaceId'), text(params, 'sessionId'), text(params, 'messageId'), params.imageIndex);
    }
    case 'chat.prompt': {
      if (typeof params.text !== 'string') throw new TypeError('text must be a string');
      return chats.prompt(text(params, 'sessionId'), params.text, params.attachmentIds, params.busyMode, params.requireIdle);
    }
    case 'chat.queue.edit': {
      if (typeof params.text !== 'string') throw new TypeError('Queue text must be a string');
      return chats.mutateQueuedInput(text(params, 'sessionId'), text(params, 'deliveryId'), params.expectedText, params.text);
    }
    case 'chat.queue.cancel': return chats.mutateQueuedInput(text(params, 'sessionId'), text(params, 'deliveryId'), params.expectedText);
    case 'chat.interactionAttachments.list': return chats.listInteractionAttachments(text(params, 'sessionId'));
    case 'chat.interactionAttachments.status': return chats.interactionAttachmentStatus(text(params, 'sessionId'), text(params, 'interactionId'));
    case 'chat.interactionAttachments.save': return chats.saveInteractionAttachments(text(params, 'sessionId'), text(params, 'interactionId'), params.attachmentsByQuestionId);
    case 'chat.respondInteraction': return chats.respondInteraction(text(params, 'sessionId'), text(params, 'interactionId'), params.response, params.attachmentsByQuestionId);
    case 'chat.abort': return chats.abort(text(params, 'sessionId'));
    case 'chat.resume': return chats.resume(text(params, 'sessionId'));
    case 'chat.refreshPiModels': return chats.refreshPiModels(text(params, 'sessionId'));
    case 'chat.commands': return chats.commands(text(params, 'sessionId'));
    case 'chat.usage': return chats.usage(text(params, 'sessionId'));
    case 'chat.codexModes.previewRecovery':
      exact(params, ['workspaceId', 'sessionId', 'viewRevision']);
      return chats.recoverCodexModes(text(params, 'workspaceId'), text(params, 'sessionId'), viewRevision(params.viewRevision));
    case 'chat.codexModes.applyRecovery': {
      exact(params, ['workspaceId', 'sessionId', 'viewRevision', 'sessionVersion', 'choices']);
      const choices = record(params.choices);
      exact(choices, ['mode', 'permissionMode']);
      return chats.recoverCodexModes(text(params, 'workspaceId'), text(params, 'sessionId'), viewRevision(params.viewRevision), {
        sessionVersion: viewRevision(params.sessionVersion),
        choices: {
          ...(choices.mode === null ? {mode: null} : choices.mode !== undefined ? {mode: text(choices, 'mode')} : {}),
          ...(choices.permissionMode !== undefined && {permissionMode: text(choices, 'permissionMode')}),
        },
      });
    }
    case 'chat.permissionMode':
    case 'chat.mode': {
      const id = text(params, 'sessionId'), permission = method === 'chat.permissionMode';
      return params.viewRevision === undefined ? chats.mode(id, undefined, permission)
        : chats.readDesktopView(id, viewRevision(params.viewRevision), () => chats.mode(id, undefined, permission, true));
    }
    case 'chat.setPermissionMode': return chats.mode(text(params, 'sessionId'), text(params, 'mode'), true);
    case 'chat.setMode': return chats.mode(text(params, 'sessionId'), text(params, 'mode'));
    case 'chat.reasoning': {
      const id = text(params, 'sessionId');
      return params.viewRevision === undefined ? chats.reasoning(id)
        : chats.readDesktopView(id, viewRevision(params.viewRevision), () => chats.reasoning(id, undefined, true));
    }
    case 'chat.setReasoning': return chats.reasoning(text(params, 'sessionId'), text(params, 'level'));
    case 'chat.setModel': return chats.setModel(text(params, 'sessionId'), text(params, 'provider'), text(params, 'modelId'));
    case 'shutdown': {
      exact(params, ['desktopInstallerId']);
      const installerId = params.desktopInstallerId === undefined ? undefined : text(params, 'desktopInstallerId');
      await close(installerId);
      if (installerId !== undefined && closingInstallerId !== installerId) throw new Error('The prepared desktop installer was not transferred');
      return { ok: true, ...(installerId !== undefined && { desktopInstallerId: installerId }) };
    }
    default: throw new Error(`Unknown desktop method: ${method}`);
  }
}

const inFlight = new Set<Promise<void>>();
const input = createInterface({ input: process.stdin, crlfDelay: Infinity });
input.on('line', line => {
  let id: number | string | null = null;
  let method: string | undefined;
  let task!: Promise<void>;
  task = (async () => {
    try {
      const message = record(JSON.parse(line));
      if (typeof message.id !== 'string' && typeof message.id !== 'number') throw new Error('Request id is required');
      id = message.id;
      method = text(message, 'method');
      const result = await dispatch(id, method, record(message.params ?? {}));
      await writeFrame({ id, result });
    } catch (error) {
      if ((method === 'chat.interactionAttachments.save' || method === 'chat.respondInteraction')
        && error instanceof QuestionAttachmentCopyError) {
        await writeFrame({id, error: {
          code: 'QUESTION_ATTACHMENT_COPY_UNAVAILABLE', message: QuestionAttachmentCopyError.publicMessage, attachmentId: error.attachmentId,
        }});
        return;
      }
      await writeFrame({ id, error: {
        code: (method === 'chat.search' || method === 'chat.searchMessages' || method === 'project.files.search' || method === 'project.files.searchContent') && error instanceof Error && error.name === 'AbortError' ? 'SEARCH_CANCELLED'
          : error instanceof StaleHistorySnapshotError ? error.code : error instanceof ProjectCopyChangedError ? 'PROJECT_COPY_CHANGED' : error instanceof HandoffError ? error.code : error instanceof SessionIdentityError && (error.code.startsWith('HANDOFF_') || error.code.startsWith('PIN_')) ? error.code : error instanceof HistoryOperationError ? `HISTORY_${error.code}` : error instanceof CodeCheckpointError ? `CHECKPOINT_${error.code}` : error instanceof WorktreeInitializationError || error instanceof GitWorktreeError ? error.code : error instanceof ProjectScriptRunError ? error.wireCode : error instanceof ProjectScriptSettingsError ? `SCRIPT_SETTINGS_${error.code}` : error instanceof HiveHttpError ? `HIVE_HTTP_${error.status}` : error instanceof CodexInteractionError ? error.code : 'DESKTOP_ERROR',
        message: error instanceof Error ? error.message : String(error),
        ...(error instanceof HiveHttpError ? { details: error.body } : {}),
      } });
    } finally {
      if (method === 'shutdown') input.close();
    }
  })().finally(() => { inFlight.delete(task); });
  inFlight.add(task);
});
let exiting = false;
input.on('close', () => {
  if (exiting) return;
  exiting = true;
  const closed = close().then(() => true, error => { console.error(error); return false; });
  void (async () => {
    await Promise.allSettled([...inFlight]);
    const ok = await closed;
    await Promise.allSettled([...pendingWrites]);
    process.exit(ok ? 0 : 1);
  })();
});
closeInput = () => input.close();
if (outputClosed) closeInput();
for (const signal of ['SIGINT', 'SIGTERM'] as const) {
  process.once(signal, () => input.close());
}

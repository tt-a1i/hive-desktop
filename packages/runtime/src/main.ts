import { getAgentDir } from '@earendil-works/pi-coding-agent';
import { format } from 'node:util';
import { createHash } from 'node:crypto';
import { createInterface } from 'node:readline';
import { mkdirSync } from 'node:fs';
import { CodexInteractionError } from './agents/codex-interactions.js';
import { WorkspaceRemovalError } from './workspace-removal.js';
import { HiveHttpError } from './hive-host.js';
import { createDesktopRuntime } from './desktop-runtime.js';
import { structuredCommand } from './structured-runtime.js';
import { desktopDataDir } from './paths.js';
import { remoteCommands } from './remote-commands.js';
import { captureRuntimeInfo } from './runtime-info.js';

const protocolWrite = process.stdout.write.bind(process.stdout);
// The SDK and user extensions may log at any point. Keep ordinary stdout writes
// away from the desktop wire; only this captured writer sends protocol frames.
process.stdout.write = process.stderr.write.bind(process.stderr);
console.log = (...args: unknown[]) => { process.stderr.write(`${format(...args)}\n`); };
const pendingWrites = new Set<Promise<void>>();
const writeFrame = (message: unknown): Promise<void> => {
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
const runtime = await createDesktopRuntime(dataDir, event => send({ event: 'agent', data: event }), { onTerminalEvent: data => send({ event: 'terminal', data }) });
const { hive, chats, settings, agentModels, terminal, piAuth } = runtime;
let closing: Promise<void> | undefined;
const close = () => closing ??= runtime.close();

function record(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error('Expected an object');
  return value as Record<string, unknown>;
}
function text(params: Record<string, unknown>, name: string): string {
  const value = params[name];
  if (typeof value !== 'string' || !value.trim()) throw new Error(`${name} is required`);
  return value;
}

async function dispatch(method: string, params: Record<string, unknown>): Promise<unknown> {
  if (closing && method !== 'shutdown') throw new Error('Desktop runtime is closing');
  switch (method) {
    case 'pi.models.get': return runtime.piModels.get();
    case 'pi.models.change': return runtime.piModels.change(params.revision, params.change);
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
    case 'team.member.create': return runtime.createMember(text(params, 'workspaceId'), { name: params.name, role: params.role, backend: params.backend,
      ...(params.description !== undefined && { description: params.description }) });
    case 'team.members': {
      const workspace = text(params, 'workspaceId');
      const members = await hive.request('GET', `/api/ui/workspaces/${encodeURIComponent(workspace)}/team`) as { id: string }[];
      return members.map(member => {
        const launch = runtime.agents.peekAgentLaunchConfig(workspace, member.id);
        if (!launch || !structuredCommand(launch.command)) return member;
        const args = launch.args ?? [];
        const providerIndex = args.indexOf('--provider');
        const modelIndex = args.indexOf('--model');
        const provider = providerIndex >= 0 && args[providerIndex + 1];
        const id = modelIndex >= 0 && args[modelIndex + 1];
        return provider && id ? { ...member, desktop_model: { provider, id } } : member;
      });
    }
    case 'agent.models': return agentModels.list(text(params, 'workspaceId'), params.backend);
    case 'desktop.settings.codexAccount': return settings.codexAccount(params.command);
    case 'desktop.settings.probe': return settings.probe(params.backend, params.command);
    case 'desktop.settings.get': return settings.get();
    case 'desktop.settings.update': return settings.update(params);
    case 'terminal.list': return terminal.list(text(params, 'workspaceId'));
    case 'terminal.start': return terminal.start(text(params, 'workspaceId'));
    case 'terminal.attach': return terminal.attach({ attachmentId: text(params, 'attachmentId'), workspaceId: text(params, 'workspaceId'), runId: text(params, 'runId'), cols: params.cols as number, rows: params.rows as number });
    case 'terminal.restoreComplete': return terminal.restoreComplete({ attachmentId: text(params, 'attachmentId'), sequence: params.sequence as number });
    case 'terminal.ack': return terminal.ack({ attachmentId: text(params, 'attachmentId'), sequence: params.sequence as number, bytes: params.bytes as number });
    case 'terminal.input': return terminal.input({ attachmentId: text(params, 'attachmentId'), bytes: params.bytes, ...(params.userInput !== undefined && { userInput: params.userInput as boolean }) });
    case 'terminal.resize': return terminal.resize({ attachmentId: text(params, 'attachmentId'), cols: params.cols as number, rows: params.rows as number });
    case 'terminal.detach': return terminal.detach({ attachmentId: text(params, 'attachmentId') });
    case 'terminal.stop': return terminal.stop({ workspaceId: text(params, 'workspaceId'), runId: text(params, 'runId') });
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
    case 'hive.request': return runtime.requestHive(text(params, 'method'), text(params, 'path'), params.body);
    case 'chat.pinned': return chats.pinned();
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
    case 'chat.attachments.clipboard': {
      const workspace = text(params, 'workspaceId');
      chats.assertWorkspaceAvailable(workspace);
      return chats.attachments.addClipboard(workspace, params.image);
    }
    case 'chat.search': return chats.search(params.query, params.archived, params.offset);
    case 'chat.list': return chats.list(text(params, 'workspaceId'));
    case 'chat.createRecord':
    case 'chat.create': {
      if (params.backend !== undefined && params.backend !== 'pi' && params.backend !== 'codex' && params.backend !== 'pi-rpc' && params.backend !== 'claude') throw new Error('Unsupported chat backend');
      const workspace = text(params, 'workspaceId');
      return method === 'chat.createRecord' ? chats.createRecord(workspace, params.backend) : chats.create(workspace, params.backend);
    }
    case 'chat.pin': {
      const session = chats.setPinned(text(params, 'sessionId'), params.pinned);
      return { session, sessions: chats.list(session.workspaceId) };
    }
    case 'chat.archive': {
      const session = await chats.setArchived(text(params, 'sessionId'), params.archived);
      return { session, sessions: chats.list(session.workspaceId) };
    }
    case 'chat.rename': return chats.rename(text(params, 'sessionId'), params.title);
    case 'chat.open': return chats.open(text(params, 'sessionId'));
    case 'chat.prompt': {
      if (typeof params.text !== 'string') throw new TypeError('text must be a string');
      return chats.prompt(text(params, 'sessionId'), params.text, params.attachmentIds);
    }
    case 'chat.respondInteraction': return chats.respondInteraction(text(params, 'sessionId'), text(params, 'interactionId'), params.response);
    case 'chat.abort': return chats.abort(text(params, 'sessionId'));
    case 'chat.resume': return chats.resume(text(params, 'sessionId'));
    case 'chat.refreshPiModels': return chats.refreshPiModels(text(params, 'sessionId'));
    case 'chat.commands': return chats.commands(text(params, 'sessionId'));
    case 'chat.usage': return chats.usage(text(params, 'sessionId'));
    case 'chat.reasoning': return chats.reasoning(text(params, 'sessionId'));
    case 'chat.setReasoning': return chats.reasoning(text(params, 'sessionId'), text(params, 'level'));
    case 'chat.setModel': return chats.setModel(text(params, 'sessionId'), text(params, 'provider'), text(params, 'modelId'));
    case 'shutdown': await close(); return { ok: true };
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
      const result = await dispatch(method, record(message.params ?? {}));
      await writeFrame({ id, result });
    } catch (error) {
      await writeFrame({ id, error: {
        code: error instanceof HiveHttpError ? `HIVE_HTTP_${error.status}` : error instanceof CodexInteractionError ? error.code : 'DESKTOP_ERROR',
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
for (const signal of ['SIGINT', 'SIGTERM'] as const) {
  process.once(signal, () => input.close());
}

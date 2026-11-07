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

const protocolWrite = process.stdout.write.bind(process.stdout);
// The SDK and user extensions may log at any point. Keep ordinary stdout writes
// away from the desktop wire; only this captured writer sends protocol frames.
process.stdout.write = process.stderr.write.bind(process.stderr);
console.log = (...args: unknown[]) => { process.stderr.write(`${format(...args)}\n`); };
const send = (message: unknown) => protocolWrite(`${JSON.stringify(message)}\n`);

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
      return { version: '0.1.0', dataDir, piAgentDir: getAgentDir(), hivePort: hive.port, navigation: hive.getNavigation() };
    }
    case 'desktop.navigation.save': {
      const sessions = record(params.sessions);
      if (!(params.workspace === null || typeof params.workspace === 'string')
        || !Object.values(sessions).every(id => typeof id === 'string')) throw new Error('Invalid workspace selection');
      const value = JSON.stringify({ workspace: params.workspace, sessions });
      hive.saveNavigation(value);
      return { ok: true };
    }
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
    case 'chat.search': return chats.search(params.query);
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
    case 'chat.rename': return chats.rename(text(params, 'sessionId'), params.title);
    case 'chat.open': return chats.open(text(params, 'sessionId'));
    case 'chat.prompt': return chats.prompt(text(params, 'sessionId'), text(params, 'text'));
    case 'chat.respondInteraction': return chats.respondInteraction(text(params, 'sessionId'), text(params, 'interactionId'), params.response);
    case 'chat.abort': return chats.abort(text(params, 'sessionId'));
    case 'chat.resume': return chats.resume(text(params, 'sessionId'));
    case 'chat.refreshPiModels': return chats.refreshPiModels(text(params, 'sessionId'));
    case 'chat.setModel': return chats.setModel(text(params, 'sessionId'), text(params, 'provider'), text(params, 'modelId'));
    case 'shutdown': await close(); return { ok: true };
    default: throw new Error(`Unknown desktop method: ${method}`);
  }
}

const input = createInterface({ input: process.stdin, crlfDelay: Infinity });
input.on('line', line => {
  let id: number | string | null = null;
  void (async () => {
    const message = record(JSON.parse(line));
    if (typeof message.id !== 'string' && typeof message.id !== 'number') throw new Error('Request id is required');
    id = message.id;
    const method = text(message, 'method');
    const result = await dispatch(method, record(message.params ?? {}));
    send({ id, result });
    if (method === 'shutdown') input.close();
  })().catch(error => send({ id, error: {
    code: error instanceof HiveHttpError ? `HIVE_HTTP_${error.status}` : error instanceof CodexInteractionError ? error.code : 'DESKTOP_ERROR',
    message: error instanceof Error ? error.message : String(error),
    ...(error instanceof HiveHttpError ? { details: error.body } : {}),
  } }));
});
input.on('close', () => { void close().then(() => process.exit(0), error => { console.error(error); process.exit(1); }); });
for (const signal of ['SIGINT', 'SIGTERM'] as const) {
  process.once(signal, () => input.close());
}

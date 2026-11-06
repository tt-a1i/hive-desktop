import { format } from 'node:util';
import { createInterface } from 'node:readline';
import { mkdirSync } from 'node:fs';
import { HiveHttpError } from './hive-host.js';
import { createDesktopRuntime } from './desktop-runtime.js';
import { desktopDataDir } from './paths.js';

const protocolWrite = process.stdout.write.bind(process.stdout);
// The SDK and user extensions may log at any point. Keep ordinary stdout writes
// away from the desktop wire; only this captured writer sends protocol frames.
process.stdout.write = process.stderr.write.bind(process.stderr);
console.log = (...args: unknown[]) => { process.stderr.write(`${format(...args)}\n`); };
const send = (message: unknown) => protocolWrite(`${JSON.stringify(message)}\n`);

const dataDir = desktopDataDir();
mkdirSync(dataDir, { recursive: true });
const runtime = await createDesktopRuntime(dataDir, event => send({ event: 'agent', data: event }));
const { hive, chats, settings, agentModels } = runtime;
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
    case 'initialize': return { version: '0.1.0', dataDir, hivePort: hive.port };
    case 'agent.models': return agentModels.list(text(params, 'workspaceId'), params.backend);
    case 'desktop.settings.get': return settings.get();
    case 'desktop.settings.update': return settings.update(params);
    case 'hive.request': return hive.request(text(params, 'method'), text(params, 'path'), params.body);
    case 'chat.list': return chats.list(text(params, 'workspaceId'));
    case 'chat.create': {
      if (params.backend !== undefined && params.backend !== 'pi' && params.backend !== 'codex' && params.backend !== 'pi-rpc' && params.backend !== 'claude') throw new Error('Unsupported chat backend');
      return chats.create(text(params, 'workspaceId'), params.backend);
    }
    case 'chat.open': return chats.open(text(params, 'sessionId'));
    case 'chat.prompt': return chats.prompt(text(params, 'sessionId'), text(params, 'text'));
    case 'chat.abort': return chats.abort(text(params, 'sessionId'));
    case 'chat.resume': return chats.resume(text(params, 'sessionId'));
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
    code: error instanceof HiveHttpError ? `HIVE_HTTP_${error.status}` : 'DESKTOP_ERROR',
    message: error instanceof Error ? error.message : String(error),
    ...(error instanceof HiveHttpError ? { details: error.body } : {}),
  } }));
});
input.on('close', () => { void close().then(() => process.exit(0), error => { console.error(error); process.exit(1); }); });
for (const signal of ['SIGINT', 'SIGTERM'] as const) {
  process.once(signal, () => input.close());
}

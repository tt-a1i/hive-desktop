import { randomUUID } from 'node:crypto';
import { createRequire } from 'node:module';
import { createServer, connect, type Socket } from 'node:net';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createInterface } from 'node:readline';
import { fileURLToPath } from 'node:url';
import { createTeamCommand } from '../team-tool.js';

type Binding = { workspaceId: string; agentId: string; token: string; hivePort: string; cwd: string };
const BINDING_ENV = 'HIVE_ACP_TEAM_BINDING';
const PROOF_ENV = 'HIVE_ACP_TEAM_PROOF';

/** Create a private stdio MCP server descriptor for one ACP session. */
export function createAcpTeamServer(binding: Binding) {
  const nonce = randomUUID();
  const socketPath = process.platform === 'win32'
    ? `\\\\.\\pipe\\hive-acp-${nonce}` : join(tmpdir(), `hive-acp-${nonce}.sock`);
  let seen = false;
  const sockets = new Set<Socket>();
  const proof = createServer((socket) => {
    sockets.add(socket);
    socket.once('close', () => sockets.delete(socket));
    let data = '';
    socket.setEncoding('utf8');
    socket.on('data', (chunk: string) => {
      data += chunk;
      if (data.length > 128) socket.destroy();
      else if (data === nonce) { seen = true; socket.end(); }
    });
    socket.on('error', () => {});
  });
  // A failed proof channel must not prevent normal tool use.
  const listening = new Promise<boolean>((resolve) => {
    proof.once('error', () => resolve(false));
    proof.listen(socketPath, () => resolve(true));
  });
  const path = fileURLToPath(import.meta.url);
  const args = path.endsWith('.ts') ? ['--import', createRequire(import.meta.url).resolve('tsx/esm'), path] : [path];
  let disposed = false;
  return {
    server: { name: 'hive-team', command: process.execPath, args, env: [] as { name: string; value: string }[] },
    childEnv: { [BINDING_ENV]: JSON.stringify(binding), [PROOF_ENV]: JSON.stringify({ socketPath, nonce }) },
    connected: () => seen,
    dispose: async () => {
      if (disposed) return;
      disposed = true;
      if (!await listening) return;
      for (const socket of sockets) socket.destroy();
      await new Promise<void>((resolve) => proof.close(() => resolve()));
    },
  };
}

function object(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function runServer(binding: Binding) {
  // The binding is needed only in this process and the native team CLI children.
  delete process.env[BINDING_ENV];
  const proofRaw = process.env[PROOF_ENV];
  delete process.env[PROOF_ENV];
  const team = createTeamCommand(binding);
  const active = new Map<string | number, AbortController>();
  let initialized = false;
  let ready = false;
  let ended = false;
  const send = (value: unknown) => { if (!ended) process.stdout.write(`${JSON.stringify(value)}\n`); };
  const markConnected = () => {
    if (!proofRaw) return;
    try {
      const { socketPath, nonce } = JSON.parse(proofRaw) as { socketPath: string; nonce: string };
      const socket = connect(socketPath, () => socket.end(nonce));
      socket.on('error', () => {});
    } catch { /* Proof is optional. */ }
  };
  const input = createInterface({ input: process.stdin, crlfDelay: Infinity });
  input.on('line', (line) => {
    let message: unknown;
    try { message = JSON.parse(line); } catch { send({ jsonrpc: '2.0', id: null, error: { code: -32700, message: 'Parse error' } }); return; }
    if (!object(message) || message.jsonrpc !== '2.0' || typeof message.method !== 'string') {
      send({ jsonrpc: '2.0', id: null, error: { code: -32600, message: 'Invalid Request' } }); return;
    }
    const { method, params, id } = message;
    if (id === undefined) {
      if (method === 'notifications/initialized' && initialized) ready = true;
      if (method === 'notifications/cancelled' && object(params) && (typeof params.requestId === 'string' || typeof params.requestId === 'number')) active.get(params.requestId)?.abort();
      return;
    }
    if (typeof id !== 'string' && typeof id !== 'number') { send({ jsonrpc: '2.0', id: null, error: { code: -32600, message: 'Invalid Request' } }); return; }
    const result = (value: unknown) => send({ jsonrpc: '2.0', id, result: value });
    const error = (code: number, text: string) => send({ jsonrpc: '2.0', id, error: { code, message: text } });
    if (method === 'initialize') {
      if (initialized || !object(params) || params.protocolVersion !== '2025-11-25' || !object(params.capabilities) || !object(params.clientInfo)) return error(-32602, 'Invalid initialize parameters');
      initialized = true;
      return result({ protocolVersion: '2025-11-25', capabilities: { tools: {} }, serverInfo: { name: 'hive-team', version: '1.0.0' } });
    }
    if (!ready) return error(-32600, 'Server is not initialized');
    if (method === 'ping') return result({});
    if (method === 'tools/list') {
      markConnected();
      return result({ tools: [{ name: team.name, description: team.description, inputSchema: { type: 'object', properties: { args: { type: 'array', items: { type: 'string' }, description: 'CLI arguments after team, one argument per array item.' }, stdin: { type: 'string', description: 'Input for commands using --stdin.' } }, required: ['args'] } }] });
    }
    if (method !== 'tools/call') return error(-32601, 'Method not found');
    if (!object(params) || params.name !== 'team' || !object(params.arguments) || !Array.isArray(params.arguments.args) || !params.arguments.args.every((arg: unknown) => typeof arg === 'string') || (params.arguments.stdin !== undefined && typeof params.arguments.stdin !== 'string')) return error(-32602, 'Invalid tool arguments');
    const controller = new AbortController();
    active.set(id, controller);
    const args = params.arguments.args as string[];
    const stdin = params.arguments.stdin as string | undefined;
    void team.execute(String(id), { args, ...(stdin === undefined ? {} : { stdin }) }, controller.signal).then(
      (value) => result({ content: value.content }),
      (failure: unknown) => result({ content: [{ type: 'text', text: failure instanceof Error ? failure.message : String(failure) }], isError: true }),
    ).finally(() => active.delete(id));
  });
  input.on('close', () => { ended = true; for (const controller of active.values()) controller.abort(); });
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1] && process.env[BINDING_ENV]) {
  try { runServer(JSON.parse(process.env[BINDING_ENV]) as Binding); }
  catch { process.exitCode = 1; }
}

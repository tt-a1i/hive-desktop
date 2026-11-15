import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { createInterface } from 'node:readline';
import { createAcpTeamServer } from '../src/agents/acp-team-stdio.js';

let received = false;
const http = createServer((req, res) => {
  assert.equal(req.url, '/api/workspaces/workspace-1/team');
  assert.equal(req.headers['x-hive-agent-token'], 'secret-token');
  received = true;
  res.writeHead(200, { 'content-type': 'application/json' }).end('{"members":[]}');
});
await new Promise<void>((resolve) => http.listen(0, '127.0.0.1', resolve));
const address = http.address();
assert(address && typeof address !== 'string');
const bridge = createAcpTeamServer({ workspaceId: 'workspace-1', agentId: 'agent-1', token: 'secret-token', hivePort: String(address.port), cwd: process.cwd() });
assert(!JSON.stringify(bridge.server).includes('secret-token'));
const child = spawn(bridge.server.command, bridge.server.args, { env: { ...process.env, ...bridge.childEnv }, stdio: ['pipe', 'pipe', 'pipe'] });
let stderr = '';
child.stderr.on('data', (chunk) => { stderr += chunk; });
const lines = createInterface({ input: child.stdout })[Symbol.asyncIterator]();
async function request(id: number, method: string, params: unknown) {
  child.stdin.write(`${JSON.stringify({ jsonrpc: '2.0', id, method, params })}\n`);
  const next = await lines.next();
  assert(!next.done, `bridge exited: ${stderr}`);
  const response = JSON.parse(next.value);
  assert.equal(response.id, id);
  return response;
}
try {
  const init = await request(1, 'initialize', { protocolVersion: '2025-11-25', capabilities: {}, clientInfo: { name: 'test', version: '1' } });
  assert.equal(init.result.protocolVersion, '2025-11-25');
  child.stdin.write('{"jsonrpc":"2.0","method":"notifications/initialized"}\n');
  const list = await request(2, 'tools/list', {});
  assert.equal(list.result.tools[0].name, 'team');
  for (let i = 0; i < 100 && !bridge.connected(); i++) await new Promise((resolve) => setTimeout(resolve, 10));
  assert(bridge.connected());
  const invalid = await request(3, 'tools/call', { name: 'team', arguments: { args: [42] } });
  assert.equal(invalid.error.code, -32602);
  const unknown = await request(4, 'unknown', {});
  assert.equal(unknown.error.code, -32601);
  const call = await request(5, 'tools/call', { name: 'team', arguments: { args: ['list'] } });
  assert.deepEqual(JSON.parse(call.result.content[0].text), { members: [] });
  assert(received);
  assert(!stderr.includes('secret-token'));
  console.log('ACP team MCP: initialization, discovery, actual CLI call and errors passed');
} finally {
  child.stdin.end();
  await new Promise<void>((resolve) => child.once('close', () => resolve()));
  await bridge.dispose();
  await new Promise<void>((resolve) => http.close(() => resolve()));
}

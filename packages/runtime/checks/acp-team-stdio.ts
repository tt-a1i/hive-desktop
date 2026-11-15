import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { createInterface } from 'node:readline';
import { acpAgentEnvironment, createAcpTeamServer } from '../src/agents/acp-team-stdio.js';

const configuredEnv={HIVE_ACP_TEAM_BINDING:'stale-binding',HIVE_ACP_TEAM_PROOF:'stale-proof',OPENAI_API_KEY:'fixture-key-no-network',CODEX_HOME:'/fixture/native/config'};
const agentEnv=acpAgentEnvironment(configuredEnv);
assert.equal(agentEnv.HIVE_ACP_TEAM_BINDING,undefined);assert.equal(agentEnv.HIVE_ACP_TEAM_PROOF,undefined);
assert.equal(agentEnv.OPENAI_API_KEY,configuredEnv.OPENAI_API_KEY);assert.equal(agentEnv.CODEX_HOME,configuredEnv.CODEX_HOME);
assert.equal(configuredEnv.HIVE_ACP_TEAM_BINDING,'stale-binding');
let received = false;
let reflectFailure=false;
const http = createServer((req, res) => {
  assert.equal(req.url, '/api/workspaces/workspace-1/team');
  assert.equal(req.headers['x-hive-agent-token'], 'secret-token');
  received = true;
  if(reflectFailure){res.writeHead(500,{'content-type':'application/json'}).end(JSON.stringify({error:'secret-token reflected upstream failure'}));return;}
  res.writeHead(200, { 'content-type': 'application/json' }).end('{"members":[]}');
});
await new Promise<void>((resolve) => http.listen(0, '127.0.0.1', resolve));
const address = http.address();
assert(address && typeof address !== 'string');
const bridge = createAcpTeamServer({ workspaceId: 'workspace-1', agentId: 'agent-1', token: 'secret-token', hivePort: String(address.port), cwd: process.cwd() });
assert(!JSON.stringify({command:bridge.server.command,args:bridge.server.args}).includes('secret-token'));
assert.deepEqual(bridge.server.env.map(entry=>entry.name).sort(),['HIVE_ACP_TEAM_BINDING','HIVE_ACP_TEAM_PROOF']);
assert.equal(JSON.parse(bridge.server.env.find(entry=>entry.name==='HIVE_ACP_TEAM_BINDING')!.value).token,'secret-token');
const child = spawn(bridge.server.command, bridge.server.args, { env: { ...Object.fromEntries(['HOME','LOGNAME','PATH','SHELL','USER','LANG','LC_ALL','TERM','TMPDIR','TZ'].flatMap(name=>process.env[name] === undefined ? [] : [[name,process.env[name]]])), ...Object.fromEntries(bridge.server.env.map(({name,value})=>[name,value])) }, stdio: ['pipe', 'pipe', 'pipe'] });
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
  const init = await request(1, 'initialize', { protocolVersion: '2025-06-18', capabilities: {}, clientInfo: { name: 'test', version: '1' } });
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
  reflectFailure=true;
  const reflected=await request(6,'tools/call',{name:'team',arguments:{args:['list']}});
  assert.equal(reflected.result.isError,true);
  assert(!JSON.stringify(reflected).includes('secret-token'));
  assert(JSON.stringify(reflected).includes('[redacted]'));
  assert(!stderr.includes('secret-token'));
  console.log('ACP team MCP: initialization, discovery, actual CLI call and errors passed');
} finally {
  child.stdin.end();
  await new Promise<void>((resolve) => child.once('close', () => resolve()));
  await bridge.dispose();
  await new Promise<void>((resolve) => http.close(() => resolve()));
}

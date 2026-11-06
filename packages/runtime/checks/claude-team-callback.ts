/** SDK callback + real team CLI + loopback HTTP fixture; no Claude/model tool dispatch. */
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { mkdtemp } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createClaudeTeamServer, type ClaudeTeamBinding } from '../src/agents/claude-team.js';
const directory = await mkdtemp(join(tmpdir(), 'hive-claude-team-callback-'));
const requests: { path: string; token: string | undefined }[] = [];
let sawHangingRequest!: () => void;
const hangingRequest = new Promise<void>(resolve => { sawHangingRequest = resolve; });
const server = createServer((req, res) => {
  requests.push({ path: req.url!, token: req.headers['x-hive-agent-token'] as string | undefined });
  if (req.url === '/api/workspaces/hang/team') { sawHangingRequest(); return; }
  res.writeHead(200, { 'content-type': 'application/json' }).end('{"members":[]}');
});
await new Promise<void>(resolve => server.listen(0, '127.0.0.1', resolve));
const address = server.address(); assert.ok(address && typeof address !== 'string');
let binding: ClaudeTeamBinding | undefined;
let signal: AbortSignal | undefined;
let bindingCalls = 0;
const executions = new Set<Promise<unknown>>();
const team = createClaudeTeamServer({ binding: () => { bindingCalls++; return binding; }, signal: () => signal,
  track(execution) { executions.add(execution); void execution.then(() => executions.delete(execution), () => executions.delete(execution)); } });
try {
  await assert.rejects(team.definition.handler({ args: ['list'], stdin: undefined }, {}));
  assert.equal(bindingCalls, 0, 'Out-of-run callback must fail before reading a binding or starting CLI');
  signal = new AbortController().signal;
  await assert.rejects(team.definition.handler({ args: ['list'], stdin: undefined }, {}));
  binding = { workspaceId: 'first', agentId: 'agent', token: 'synthetic-test-token-one', hivePort: String(address.port), cwd: directory };
  await team.definition.handler({ args: ['list'], stdin: undefined }, {});
  binding = { ...binding, workspaceId: 'second', token: 'synthetic-test-token-two' };
  await team.definition.handler({ args: ['list'], stdin: undefined }, {});
  assert.deepEqual(requests.map(request => request.path), ['/api/workspaces/first/team', '/api/workspaces/second/team']);
  assert.deepEqual(requests.map(request => request.token), ['synthetic-test-token-one', 'synthetic-test-token-two']);
  assert.equal(executions.size, 0);
  assert.equal(bindingCalls, 3);
  binding = { ...binding, workspaceId: 'hang' };
  const controller = new AbortController(); signal = controller.signal;
  const pending = team.definition.handler({ args: ['list'], stdin: undefined }, {});
  void pending.catch(() => {});
  await hangingRequest;
  assert.equal(executions.size, 1);
  controller.abort();
  await assert.rejects(pending);
  assert.equal(executions.size, 0);
} finally { await new Promise<void>((resolve, reject) => server.close(error => error ? reject(error) : resolve())); }
console.log('Claude SDK callback lazy binding + actual Hive CLI verified against HTTP fixture; no model calls');

import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createTeamTool } from '../src/team-tool.js';

const requests: Array<{ url?: string; headers: Record<string, unknown>; body: string }> = [];
const server = createServer(async (req, res) => {
  let body = '';
  for await (const chunk of req) body += chunk;
  requests.push({ url: req.url, headers: req.headers, body });
  if (req.url === '/api/workspaces/hang/team') {
    return;
  }
  if (req.url?.includes('/team') && req.method === 'GET') {
    res.writeHead(200, { 'content-type': 'application/json' }).end(JSON.stringify({ members: [] }));
  } else if (req.url === '/api/team/report') {
    res.writeHead(200, { 'content-type': 'application/json' }).end('{}');
  } else {
    res.writeHead(403, { 'content-type': 'application/json' }).end('{"error":"denied"}');
  }
});
await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve));
const address = server.address();
assert(address && typeof address !== 'string');
const cwd = await mkdtemp(join(tmpdir(), 'hive-team-tool-'));
const original = { ...process.env };
try {
  const tool = createTeamTool({ workspaceId: 'workspace-1', agentId: 'agent-1', token: 'secret-token', hivePort: String(address.port), cwd });
  // The SDK supplies the last two context arguments; this check invokes the tool directly.
  const execute = (args: { args: string[]; stdin?: string }, signal?: AbortSignal) =>
    tool.execute('check', args, signal, undefined, undefined as never);
  const list = await execute({ args: ['list'] });
  assert.deepEqual(JSON.parse((list.content[0] as { text: string }).text), { members: [] });
  assert.equal(requests[0]?.url, '/api/workspaces/workspace-1/team');
  assert.equal(requests[0]?.headers['x-hive-agent-id'], 'agent-1');
  assert.equal(requests[0]?.headers['x-hive-agent-token'], 'secret-token');
  const body = 'line one\n$(touch should-not-exist)';
  await execute({ args: ['report', '--stdin', '--dispatch', 'dispatch-1', '--seen', '2'], stdin: body });
  assert.deepEqual(JSON.parse(requests[1]!.body), {
    dispatch_id: 'dispatch-1', seen_seq: 2, project_id: 'workspace-1', from_agent_id: 'agent-1',
    token: 'secret-token', result: body, artifacts: [],
  });
  await assert.rejects(execute({ args: ['messages', '--dispatch', 'dispatch-1'] }), /403.*denied/);
  const controller = new AbortController();
  controller.abort();
  await assert.rejects(execute({ args: ['list'] }, controller.signal), /aborted/);
  const hangingTool = createTeamTool({ workspaceId: 'hang', agentId: 'agent-1', token: 'secret-token', hivePort: String(address.port), cwd });
  const activeController = new AbortController();
  const pending = hangingTool.execute('check', { args: ['list'] }, activeController.signal, undefined, undefined as never);
  while (!requests.some((request) => request.url === '/api/workspaces/hang/team')) {
    await new Promise((resolve) => setTimeout(resolve, 10));
  }
  activeController.abort();
  await assert.rejects(pending, /aborted/);
  assert.equal(JSON.stringify({ ...process.env }) === JSON.stringify(original), true, 'process environment changed');
  console.log('team tool: real CLI list, report, failure and abort checks passed');
} finally {
  await new Promise<void>((resolve, reject) => server.close((error) => error ? reject(error) : resolve()));
  await rm(cwd, { recursive: true, force: true });
}

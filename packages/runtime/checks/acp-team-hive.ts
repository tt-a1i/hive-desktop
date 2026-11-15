/** Synthetic ACP peers, real MCP bridge, Hive HTTP/SQLite and native team CLI. No model requests. */
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { createServer } from 'node:http';
import { once } from 'node:events';
import { createDesktopRuntime } from '../src/desktop-runtime.js';
import { createAcpSession } from '../src/agents/acp.js';
import type { Chats } from '../src/chats.js';
import type { DeliveryQueue } from '../src/delivery-queue.js';

const dir = await mkdtemp(join(tmpdir(), 'hive-acp-team-'));
const agentDir = join(dir, 'agent'); await mkdir(agentDir);
execFileSync('git', ['init', '-q', dir]);
const oldAgent = process.env.PI_CODING_AGENT_DIR, oldOffline = process.env.PI_OFFLINE;
process.env.PI_CODING_AGENT_DIR = agentDir; process.env.PI_OFFLINE = '1';
let modelRequests = 0;
const modelServer = createServer((_req, res) => { modelRequests++; res.writeHead(500).end(); });
modelServer.listen(0, '127.0.0.1'); await once(modelServer, 'listening');
const address = modelServer.address(); assert(address && typeof address !== 'string');
await writeFile(join(agentDir, 'models.json'), JSON.stringify({ providers: { openai: { api: 'openai-responses', baseUrl: `http://127.0.0.1:${address.port}/v1`, apiKey: 'fixture', models: [{ id: 'gpt-6.1-terra', name: 'Fixture', input: ['text'] }] } } }));
await writeFile(join(agentDir, 'settings.json'), JSON.stringify({ defaultProvider: 'openai', defaultModel: 'gpt-6.1-terra' }));
const runtime = await createDesktopRuntime(join(dir, 'state'), () => {});
const sessions: Awaited<ReturnType<typeof createAcpSession>>[] = [];
const queues = (chats: Chats) => (chats as unknown as { live: Map<string, { queue: DeliveryQueue }> }).live;
try {
  const workspace = await runtime.hive.request('POST', '/api/workspaces', { path: dir, name: 'ACP team', autostart_orchestrator: false }) as { id: string };
  const chat = await runtime.chats.create(workspace.id); await runtime.chats.resume(chat.id);
  const orchestrator = `${workspace.id}:orchestrator`;
  const peer = resolve('checks/fixtures/acp-team-peer.cjs');
  async function session(agentId: string) {
    const token = runtime.agents.peekAgentToken(agentId); assert(token);
    const handle = await createAcpSession({ command: process.execPath, args: [peer], cwd: dir, sessionDir: dir, onEvent: () => {}, teamBinding: () => ({ workspaceId: workspace.id, agentId, token, hivePort: String(runtime.hive.port), cwd: dir }) });
    sessions.push(handle);
    for (let i = 0; i < 100 && !handle.teamConnected(); i++) await new Promise(r => setTimeout(r, 10));
    assert(handle.teamConnected()); return handle;
  }
  async function call(handle: Awaited<ReturnType<typeof session>>, args: string[] | null) {
    await handle.prompt(JSON.stringify(args));
    const last = handle.history().at(-1) as { content: Array<{ text: string }> };
    const result = JSON.parse(last.content[0]!.text);
    assert(!result.isError, JSON.stringify(result));
    return result.content[0].text;
  }
  const lead = await session(orchestrator);
  await call(lead, ['spawn', 'coder', '--name', 'acp-worker']);
  const members = await runtime.hive.request('GET', `/api/ui/workspaces/${workspace.id}/team`) as Array<{ id: string; name: string }>;
  const worker = members.find(m => m.name === 'acp-worker'); assert(worker);
  await runtime.hive.request('POST', `/api/workspaces/${workspace.id}/agents/${encodeURIComponent(worker.id)}/start`);
  const workerChat = runtime.chats.list(workspace.id).find(c => c.agentId === worker.id); assert(workerChat);
  await queues(runtime.chats).get(workerChat.id)!.queue.pause();
  await queues(runtime.chats).get(chat.id)!.queue.pause();
  const dispatchId = JSON.parse(await call(lead, ['send', 'acp-worker', 'fixture task'])).dispatch_id as string;
  const detail = () => runtime.requestHive('GET', `/api/ui/workspaces/${workspace.id}/dispatches/${dispatchId}`) as Promise<{ state: string; report_text: string | null }>;
  assert.equal((await detail()).state, 'submitted');
  const workerSession = await session(worker.id);
  await call(workerSession, null);
  assert.equal((await detail()).state, 'submitted', 'ACP end_turn must not report dispatch');
  const inbox = JSON.parse(await call(workerSession, ['messages', '--dispatch', dispatchId]));
  await call(workerSession, ['report', 'fixture report', '--dispatch', dispatchId, '--seen', String(inbox.required_seen_seq)]);
  assert.equal((await detail()).state, 'reported');
  assert.equal((await detail()).report_text, 'fixture report');
  assert(queues(runtime.chats).get(chat.id)!.queue.pending.some(item => item.presentation?.dispatchId === dispatchId && item.presentation.displayText === 'fixture report'));
  assert.equal(modelRequests, 0);
  console.log('ACP team: real Hive spawn, dispatch, end_turn, seen, report and original orchestrator delivery passed; no model inference');
} finally {
  for (const session of sessions) await session.dispose();
  await runtime.close(); await new Promise<void>(resolve => modelServer.close(() => resolve()));
  if (oldAgent === undefined) delete process.env.PI_CODING_AGENT_DIR; else process.env.PI_CODING_AGENT_DIR = oldAgent;
  if (oldOffline === undefined) delete process.env.PI_OFFLINE; else process.env.PI_OFFLINE = oldOffline;
  await rm(dir, { recursive: true, force: true });
}

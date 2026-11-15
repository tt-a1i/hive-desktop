/** Synthetic ACP peers, real MCP bridge, Hive HTTP/SQLite and native team CLI. No model requests. */
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
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
const privateTokens:string[]=[];const eventReceipts:string[]=[];
const sessions: Awaited<ReturnType<typeof createAcpSession>>[] = [];
const queues = (chats: Chats) => (chats as unknown as { live: Map<string, { queue: DeliveryQueue }> }).live;
try {
  const workspace = await runtime.hive.request('POST', '/api/workspaces', { path: dir, name: 'ACP team', autostart_orchestrator: false }) as { id: string };
  const chat = await runtime.chats.create(workspace.id); await runtime.chats.resume(chat.id);
  const orchestrator = `${workspace.id}:orchestrator`;
  const peer = fileURLToPath(new URL('./fixtures/acp-team-peer.cjs', import.meta.url));
  async function session(agentId: string) {
    const token = runtime.agents.peekAgentToken(agentId); assert(token);privateTokens.push(token);
    const handle = await createAcpSession({ command: process.execPath, args: [peer], env:{HIVE_ACP_TEAM_BINDING:'stale-binding',HIVE_ACP_TEAM_PROOF:'stale-proof'}, cwd: dir, sessionDir: dir, onEvent: event=>eventReceipts.push(JSON.stringify(event)), teamBinding: () => ({ workspaceId: workspace.id, agentId, token, hivePort: String(runtime.hive.port), cwd: dir }) });
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
  // A peer that reflects the temporary descriptor must not leak its token to history/events.
  const reflected=await call(lead,['__fixture_reflect_private_descriptor']);
  assert(!reflected.includes('socketPath')&&!reflected.includes('hivePort'),'both complete private descriptor values must be redacted');
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
  // Changing the current Worker conversation must not retarget the original dispatch source.
  const workerRun = runtime.agents.getActiveRunByAgentId(workspace.id, worker.id)!;
  runtime.agents.stopAgentRun(workerRun.runId);
  assert.equal(await runtime.agents.waitForRunExit(workerRun.runId, 5000), true);
  const replacement = await runtime.chats.createRecord(workspace.id, 'pi');
  const index = (runtime.chats as unknown as { index: import('../src/chat-index.js').ChatIndex }).index;
  index.bind(replacement.id, worker.id);
  await runtime.chats.open(replacement.id);
  await queues(runtime.chats).get(replacement.id)!.queue.pause();
  const source = await runtime.chats.dispatchSession(workspace.id, dispatchId, worker.id);
  assert.equal(source.sourceSessionId, workerChat.id);
  assert.equal(source.session.originAgentId, worker.id);
  assert.equal(index.forAgent(workspace.id, worker.id)!.id, replacement.id);
  for (const session of sessions) {
    const transcript = await readFile(session.sessionFile!, 'utf8');
    for (const token of privateTokens) assert(!transcript.includes(token), 'team tokens must never be persisted');
    assert(!transcript.includes('socketPath'),'private proof descriptor must never be persisted');
  }
  assert(!eventReceipts.some(event=>event.includes('socketPath')),'private proof descriptor must not leak in events');
  for(const token of privateTokens)assert(!eventReceipts.some(event=>event.includes(token)),'reflected descriptors must not leak tokens in events');
  assert.equal(modelRequests, 0);
  console.log('ACP team: real Hive spawn, dispatch, end_turn, seen, report and original orchestrator delivery passed; no model inference');
} finally {
  for (const session of sessions) await session.dispose();
  await runtime.close(); await new Promise<void>(resolve => modelServer.close(() => resolve()));
  if (oldAgent === undefined) delete process.env.PI_CODING_AGENT_DIR; else process.env.PI_CODING_AGENT_DIR = oldAgent;
  if (oldOffline === undefined) delete process.env.PI_OFFLINE; else process.env.PI_OFFLINE = oldOffline;
  await rm(dir, { recursive: true, force: true });
}

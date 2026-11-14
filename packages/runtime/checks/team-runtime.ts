/** Real Hive HTTP/SQLite + real Pi SDK + real team CLI integration; no model requests.
 * A timing barrier delays (but does not replace) one real HTTP submission.
 * Private queue access only pauses delivery to ensure the check cannot request a model.
 */
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import { once } from 'node:events';
import { DatabaseSync } from 'node:sqlite';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createDesktopRuntime } from '../src/desktop-runtime.js';
import { createTeamTool } from '../src/team-tool.js';
import type { DeliveryQueue } from '../src/delivery-queue.js';
import { restoreHistoricalReport } from '../src/delivery-presentation.js';
import type { Chats, ChatEvent } from '../src/chats.js';

assert.ok(!process.argv.includes('--live-report'), 'This isolated fixture must not execute a model');
const directory = await mkdtemp(join(tmpdir(), 'hive-team-runtime-'));
const agentDir = join(directory, 'agent');
await mkdir(agentDir);
const previousAgentDir = process.env.PI_CODING_AGENT_DIR;
const previousOffline = process.env.PI_OFFLINE;
process.env.PI_CODING_AGENT_DIR = agentDir;
process.env.PI_OFFLINE = '1';
let modelRequests = 0;
const modelServer = createServer((_request, response) => {
  modelRequests++;
  response.writeHead(500).end('Unexpected model request');
});
modelServer.listen(0, '127.0.0.1');
await once(modelServer, 'listening');
const address = modelServer.address();
assert.ok(address && typeof address !== 'string');
await writeFile(join(agentDir, 'models.json'), JSON.stringify({ providers: { openai: {
  api: 'openai-responses', baseUrl: `http://127.0.0.1:${address.port}/v1`, apiKey: 'fixture-only',
  models: [{ id: 'gpt-6.1-terra', name: 'Terra fixture metadata', input: ['text'] }],
} } }));
await writeFile(join(agentDir, 'settings.json'), JSON.stringify({ defaultProvider: 'openai', defaultModel: 'gpt-6.1-terra' }));
type Runtime = Awaited<ReturnType<typeof createDesktopRuntime>>;
const queues = (chats: Chats) => (chats as unknown as { live: Map<string, { queue: DeliveryQueue }> }).live;
let runtime: Runtime | undefined;
try {
  const dataDir = join(directory, 'state');
  const stateEvents: Extract<ChatEvent, { type: 'state' }>[] = [];
  runtime = await createDesktopRuntime(dataDir, event => { if (event.type === 'state') stateEvents.push(event); });
  const first = runtime;
  const workspace = await first.hive.request('POST', '/api/workspaces', {
    path: directory, name: 'Team runtime integration', autostart_orchestrator: false,
  }) as { id: string };
  const a = await first.chats.create(workspace.id);
  await first.chats.resume(a.id);
  assert.ok((await first.chats.open(a.id)).startupContext);
  const b = await first.chats.create(workspace.id);
  await first.chats.resume(b.id);
  const initializedB = await first.chats.open(b.id);
  assert.ok(initializedB.startupContext);
  assert.ok(initializedB.history.some(message => message && typeof message === 'object'
    && 'customType' in message && message.customType === 'hive-desktop'));
  assert.ok(initializedB.history.some(message => message && typeof message === 'object'
    && 'customType' in message && message.customType === 'hive-context'));
  await first.chats.resume(a.id);

  // Select the locally configured fixture model without invoking it or modifying global defaults.
  const testModel = a.models.find(model => model.id === 'gpt-6.1-terra' && model.provider === 'openai');
  assert.ok(testModel, 'Fixture OpenAI model must be registered');
  await first.chats.setModel(a.id, testModel.provider, testModel.id);
  const agentId = `${workspace.id}:orchestrator`;
  const token = first.agents.peekAgentToken(agentId);
  assert.ok(token);
  const tool = createTeamTool({ workspaceId: workspace.id, agentId, token,
    hivePort: String(first.hive.port), cwd: directory });
  const spawned = await tool.execute('spawn-check', { args: ['spawn', 'coder', '--name', 'sdk-worker'] },
    undefined, undefined, undefined as never);
  const result = JSON.parse((spawned.content[0] as { text: string }).text) as { id?: string; agent_id?: string; worker_id?: string };
  const members = await first.hive.request('GET', `/api/ui/workspaces/${workspace.id}/team`) as Array<{ id: string; name: string }>;
  const worker = members.find(member => member.name === 'sdk-worker');
  assert.ok(worker, `Worker missing after successful CLI spawn: ${JSON.stringify(result)}`);
  const launch = first.agents.peekAgentLaunchConfig(workspace.id, worker.id);
  assert.equal(launch?.command, 'hive:pi');
  assert.deepEqual(launch.args, ['--provider', testModel.provider, '--model', testModel.id]);
  await first.hive.request('POST', `/api/workspaces/${workspace.id}/agents/${encodeURIComponent(worker.id)}/start`);
  assert.ok(first.agents.getActiveRunByAgentId(workspace.id, worker.id));

  const actualRequest = first.hive.request.bind(first.hive);
  let release!: () => void;
  let reached!: () => void;
  const blocked = new Promise<void>(resolve => { release = resolve; });
  const atInput = new Promise<void>(resolve => { reached = resolve; });
  let switchReachedTeam = false;
  first.hive.request = async (method, path, body) => {
    if (path === `/api/ui/workspaces/${workspace.id}/team`) switchReachedTeam = true;
    if (path.endsWith('/user-input')) {
      reached();
      await blocked;
      // Pause whichever chat is bound at the actual HTTP submission boundary.
      // This also prevents a model request if the routing regression returns.
      const bound = first.chats.list(workspace.id).find(chat => chat.agentId === agentId);
      assert.ok(bound);
      await queues(first.chats).get(bound.id)!.queue.pause();
    }
    return actualRequest(method, path, body);
  };
  const userText = 'Acknowledge with OK. When the worker report arrives, acknowledge receipt briefly. No tools needed.';
  first.chats.rename(a.id, '新会话');
  const input = first.chats.prompt(a.id, userText);
  await atInput;
  const switching = first.chats.resume(b.id);
  const switchVerdict = switching.then(() => 'switched' as const, () => 'blocked' as const);
  // Let the competing operation enter its first asynchronous boundary. A broken
  // lock reaches the real team request; finish that switch before releasing input.
  await new Promise<void>(resolve => setImmediate(resolve));
  if (switchReachedTeam) await switchVerdict;
  release();
  await input;
  assert.equal(await switchVerdict, 'blocked');
  assert.equal(first.chats.list(workspace.id).find(chat => chat.id === a.id)?.title, '新会话');
  assert.equal(first.chats.list(workspace.id).find(chat => chat.id === a.id)?.customTitle, true);
  assert.equal(first.chats.list(workspace.id).find(chat => chat.id === a.id)?.agentId, agentId);
  assert.ok(queues(first.chats).get(a.id)!.queue.pending.some(item => item.text.includes(userText)));
  const presentation = queues(first.chats).get(a.id)!.queue.pending[0]?.presentation;
  assert.ok(presentation);
  assert.equal(presentation.version, 1);
  assert.equal(presentation.origin, 'user');
  assert.equal(presentation.displayText, userText);
  assert.ok(Number.isSafeInteger(presentation.messageSequence));
  const db = new DatabaseSync(join(dataDir, 'hive/runtime.sqlite'), { readOnly: true });
  try {
    const row = db.prepare('SELECT workspace_id, worker_id, type, text FROM messages WHERE sequence = ?').get(presentation.messageSequence!);
    assert.deepEqual({ ...row }, { workspace_id: workspace.id, worker_id: agentId, type: 'user_input', text: userText });
  } finally { db.close(); }
  assert.equal((await first.chats.open(b.id)).pending, 0);
  // Exercise the installed npm runtime's actual send/report contract, with both
  // recipients paused so durable acceptance cannot invoke either model.
  const workerChat = first.chats.list(workspace.id).find(chat => chat.agentId === worker.id);
  assert.ok(workerChat);
  await queues(first.chats).get(workerChat.id)!.queue.pause();
  await queues(first.chats).get(a.id)!.queue.pause();
  const sent = await tool.execute('send-check', { args: ['send', 'sdk-worker', 'durable-worker-task'] },
    undefined, undefined, undefined as never);
  const dispatchId = (JSON.parse((sent.content[0] as { text: string }).text) as { dispatch_id: string }).dispatch_id;
  assert.ok(dispatchId);
  type Dispatch = { id: string; state: string; submitted_at: number | null; report_text: string | null };
  const dispatches = () => first.hive.request('GET', `/api/ui/workspaces/${workspace.id}/dispatches`) as Promise<Dispatch[]>;
  const detail = () => first.requestHive('GET', `/api/ui/workspaces/${workspace.id}/dispatches/${dispatchId}`) as Promise<Dispatch>;
  const anonymous = await fetch(`http://127.0.0.1:${first.hive.port}/api/ui/workspaces/${workspace.id}/dispatches/${dispatchId}`);
  assert.equal(anonymous.status, 403);
  await anonymous.body?.cancel();
  await assert.rejects(first.requestHive('GET', `/api/ui/workspaces/${workspace.id}/dispatches/missing`), { status: 404 });
  const submitted = await detail();
  assert.deepEqual(submitted, (await dispatches()).find(dispatch => dispatch.id === dispatchId));
  assert.equal(submitted?.state, 'submitted');
  assert.equal(typeof submitted.submitted_at, 'number');
  assert.ok(queues(first.chats).get(workerChat.id)!.queue.pending.some(item =>
    item.text.includes(dispatchId) && item.text.includes('durable-worker-task')));
  const taskDelivery = queues(first.chats).get(workerChat.id)!.queue.pending.find(item => item.text.includes(dispatchId));
  assert.equal(taskDelivery?.presentation?.origin, 'dispatch');
  assert.equal(taskDelivery?.presentation?.displayText, 'durable-worker-task');
  assert.equal(taskDelivery?.presentation?.dispatchId, dispatchId);
  const workerToken = first.agents.peekAgentToken(worker.id);
  assert.ok(workerToken);
  const workerTool = createTeamTool({ workspaceId: workspace.id, agentId: worker.id, token: workerToken,
    hivePort: String(first.hive.port), cwd: directory });
  const messages = await workerTool.execute('messages-check', { args: ['messages', '--dispatch', dispatchId] },
    undefined, undefined, undefined as never);
  const inbox = JSON.parse((messages.content[0] as { text: string }).text) as { dispatch_id: string; required_seen_seq: number };
  assert.equal(inbox.dispatch_id, dispatchId);
  assert.ok(Number.isSafeInteger(inbox.required_seen_seq));
  await workerTool.execute('report-check', { args: ['report', 'explicit-worker-report', '--dispatch', dispatchId,
    '--seen', String(inbox.required_seen_seq), '--artifact', 'proof.txt'] }, undefined, undefined, undefined as never);
  const reported = await detail();
  assert.deepEqual(reported, (await dispatches()).find(dispatch => dispatch.id === dispatchId));
  assert.equal(reported?.state, 'reported');
  assert.equal(reported.report_text, 'explicit-worker-report');
  const delivery = queues(first.chats).get(a.id)!.queue.pending.find(item => item.presentation?.dispatchId === dispatchId);
  assert.ok(delivery);
  const report = { version: 1, origin: 'report', displayText: 'explicit-worker-report', dispatchId,
    workerName: 'sdk-worker', workerId: worker.id, artifacts: ['proof.txt'] };
  assert.deepEqual(delivery.presentation, report);
  const legacy = { role: 'custom', customType: 'hive-delivery', content: delivery.text,
    presentation: { version: 1, origin: 'system', deliveryId: delivery.id, displayText: delivery.text } };
  const restore = (message: typeof legacy, recipient: string) => restoreHistoricalReport(message,
    (id, payload) => first.hive.findReportPresentation(workspace.id, recipient, id, payload));
  assert.deepEqual(restore(legacy, agentId), { ...legacy, presentation: { ...report, deliveryId: delivery.id } });
  assert.equal(restore(legacy, worker.id), legacy);
  const edited = { ...legacy, content: delivery.text + 'edited', presentation: { ...legacy.presentation, displayText: delivery.text + 'edited' } };
  assert.equal(restore(edited, agentId), edited);
  const pasted = { ...legacy, presentation: { ...legacy.presentation, origin: 'user' } };
  assert.equal(restore(pasted, agentId), pasted);
  const pendingAtRestart = queues(first.chats).get(a.id)!.queue.pending.length;
  assert.equal(pendingAtRestart, 2);
  await first.close();
  runtime = undefined;

  const resumedStates: typeof stateEvents = [];
  runtime = await createDesktopRuntime(dataDir, event => { if (event.type === 'state') resumedStates.push(event); });
  assert.equal(queues(runtime.chats).size, 0, 'Restart should begin without loaded chats');
  await assert.rejects(runtime.chats.resume(b.id));
  assert.ok(!queues(runtime.chats).has(a.id), 'The persisted queue must prevent switching without relaunching the previous orchestrator');
  const reopenedA = await runtime.chats.open(a.id);
  assert.equal(reopenedA.agentId, agentId);
  assert.equal(reopenedA.title, '新会话');
  assert.equal(reopenedA.customTitle, true);
  assert.equal(reopenedA.pending, pendingAtRestart);
  const preview = [{ deliveryId: delivery.id, dispatchId, text: 'explicit-worker-report', workerName: 'sdk-worker', workerId: worker.id,
    artifacts: ['proof.txt'], processing: false }];
  assert.deepEqual(stateEvents.filter(event => event.sessionId === a.id).at(-1)?.pendingReports, preview);
  assert.deepEqual(reopenedA.pendingReports, preview);
  assert.ok(!reopenedA.completedDeliveryIds.includes(delivery.id));
  assert.deepEqual((await runtime.chats.open(b.id)).pendingReports, []);
  assert.equal(reopenedA.busy, false);
  assert.equal((await runtime.chats.open(b.id)).agentId, undefined);
  if (process.argv.includes('--live-report')) {
    assert.deepEqual([reopenedA.model?.provider, reopenedA.model?.id], ['openai', 'gpt-6.1-terra']);
    await runtime.chats.resume(a.id);
    const deadline = Date.now() + 240_000;
    while (!resumedStates.some(event => event.completedReportId === delivery.id)) {
      assert.ok(Date.now() < deadline, 'Expected a native completion marker for the report');
      await new Promise(resolve => setTimeout(resolve, 200));
    }
    assert.ok(resumedStates.some(event => event.pendingReports.some(report => report.deliveryId === delivery.id && report.processing)));
    await runtime.close();
    runtime = await createDesktopRuntime(dataDir, () => {});
    const finished = await runtime.chats.open(a.id);
    assert.ok(finished.completedDeliveryIds.includes(delivery.id));
    assert.ok(!finished.pendingReports.some(report => report.deliveryId === delivery.id));
    assert.equal(finished.history.filter(message => typeof message === 'object' && message !== null
      && 'presentation' in message && typeof message.presentation === 'object' && message.presentation !== null
      && 'deliveryId' in message.presentation && message.presentation.deliveryId === delivery.id).length, 1);
    console.log('Live report passed: processing, native completion and one history entry after restart.');
  } else {
    console.log('Team runtime passed: fresh-chat startup, CLI backend/model inheritance, actual dispatch/seen/report delivery, concurrent input isolation, persisted queue switch guard. No model request sent.');
  }
} finally {
  await runtime?.close();
  await new Promise<void>((resolve, reject) => modelServer.close(error => error ? reject(error) : resolve()));
  if (previousAgentDir === undefined) delete process.env.PI_CODING_AGENT_DIR;
  else process.env.PI_CODING_AGENT_DIR = previousAgentDir;
  if (previousOffline === undefined) delete process.env.PI_OFFLINE;
  else process.env.PI_OFFLINE = previousOffline;
  await rm(directory, { recursive: true, force: true });
  assert.equal(modelRequests, 0, 'Fixture model must never receive a request');
}

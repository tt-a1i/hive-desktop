/** Real Hive HTTP/SQLite + real Pi SDK + real team CLI integration; no model requests.
 * A timing barrier delays (but does not replace) one real HTTP submission.
 * Private queue access only pauses delivery to ensure the check cannot request a model.
 */
import assert from 'node:assert/strict';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createDesktopRuntime } from '../src/desktop-runtime.js';
import { createTeamTool } from '../src/team-tool.js';
import type { DeliveryQueue } from '../src/delivery-queue.js';
import type { Chats } from '../src/chats.js';

const directory = await mkdtemp(join(tmpdir(), 'hive-team-runtime-'));
type Runtime = Awaited<ReturnType<typeof createDesktopRuntime>>;
const queues = (chats: Chats) => (chats as unknown as { live: Map<string, { queue: DeliveryQueue }> }).live;
let runtime: Runtime | undefined;
try {
  const dataDir = join(directory, 'state');
  runtime = await createDesktopRuntime(dataDir, () => {});
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

  // Select an explicit native test model, without invoking it or modifying global defaults.
  const testModel = a.models.find(model => model.id === 'gpt-6.1-terra' && model.provider === 'openai');
  assert.ok(testModel, 'Native OpenAI/Codex GPT-6.1 Terra must be registered');
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
  const input = first.chats.prompt(a.id, 'persisted-only-for-A');
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
  assert.equal(first.chats.list(workspace.id).find(chat => chat.id === a.id)?.agentId, agentId);
  assert.ok(queues(first.chats).get(a.id)!.queue.pending.some(item => item.text.includes('persisted-only-for-A')));
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
  const submitted = (await dispatches()).find(dispatch => dispatch.id === dispatchId);
  assert.equal(submitted?.state, 'submitted');
  assert.equal(typeof submitted.submitted_at, 'number');
  assert.ok(queues(first.chats).get(workerChat.id)!.queue.pending.some(item =>
    item.text.includes(dispatchId) && item.text.includes('durable-worker-task')));
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
    '--seen', String(inbox.required_seen_seq)] }, undefined, undefined, undefined as never);
  const reported = (await dispatches()).find(dispatch => dispatch.id === dispatchId);
  assert.equal(reported?.state, 'reported');
  assert.equal(reported.report_text, 'explicit-worker-report');
  assert.ok(queues(first.chats).get(a.id)!.queue.pending.some(item => item.text.includes('explicit-worker-report')));
  const pendingAtRestart = queues(first.chats).get(a.id)!.queue.pending.length;
  assert.equal(pendingAtRestart, 2);
  await first.close();
  runtime = undefined;

  runtime = await createDesktopRuntime(dataDir, () => {});
  assert.equal(queues(runtime.chats).size, 0, 'Restart should begin without loaded chats');
  await assert.rejects(runtime.chats.resume(b.id));
  const reopenedA = await runtime.chats.open(a.id);
  assert.equal(reopenedA.agentId, agentId);
  assert.equal(reopenedA.pending, pendingAtRestart);
  assert.equal(reopenedA.busy, false);
  assert.equal((await runtime.chats.open(b.id)).agentId, undefined);
  console.log('Team runtime passed: fresh-chat startup, CLI backend/model inheritance, actual dispatch/seen/report delivery, concurrent input isolation, persisted queue switch guard. No model request sent.');
} finally {
  await runtime?.close();
  await rm(directory, { recursive: true, force: true });
}

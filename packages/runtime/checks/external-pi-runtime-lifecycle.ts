/** Actual external Pi RPC subprocess death + Hive HTTP/SQLite; delivery scheduling stays paused. */
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdir, mkdtemp, writeFile } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { createDesktopRuntime } from '../src/desktop-runtime.js';
import { HiveHttpError } from '../src/hive-host.js';
import type { Chats, ChatEvent } from '../src/chats.js';
import type { DeliveryQueue } from '../src/delivery-queue.js';
import type { PiSessionHandle } from '../src/agents/types.js';

await mkdir(resolve('../../.artifacts'), { recursive: true });
const directory = await mkdtemp(resolve('../../.artifacts/external-pi-lifecycle-'));
const events: ChatEvent[] = [];
const runtime = await createDesktopRuntime(join(directory, 'state'), event => events.push(event));
const live = (chats: Chats) => (chats as unknown as {
  live: Map<string, { handle: PiSessionHandle & { teamToolName: string }; queue: DeliveryQueue }>;
}).live;
function ownedPid(): number {
  const children = execFileSync('ps', ['-axo', 'pid=,ppid=,command='], { encoding: 'utf8' }).trim().split('\n').map(line => {
    const match = /^\s*(\d+)\s+(\d+)\s+(.*)$/.exec(line)!;
    return { pid: Number(match[1]), ppid: Number(match[2]), command: match[3]!.trim() };
  }).filter(child => child.ppid === process.pid);
  // Pi replaces process.title, so its RPC arguments are absent from ps.
  const matches = children.filter(child => child.command === 'pi' || child.command.includes('/pi ') || child.command.includes('pi-coding-agent'));
  assert.equal(matches.length, 1, 'Select only the owned external Pi RPC child for this directory');
  return matches[0]!.pid;
}
try {
  const workspace = await runtime.hive.request('POST', '/api/workspaces', {
    path: directory, name: 'External Pi process lifecycle', autostart_orchestrator: false,
  }) as { id: string };
  const chat = await runtime.chats.create(workspace.id, 'pi-rpc');
  await runtime.chats.setModel(chat.id, 'openai', 'gpt-6.1-terra');
  await runtime.chats.resume(chat.id);
  const agentId = `${workspace.id}:orchestrator`;
  const idle = live(runtime.chats).get(chat.id)!;
  const idleRun = runtime.agents.getActiveRunByAgentId(workspace.id, agentId)!;
  const idlePid = ownedPid();
  process.kill(idlePid, 'SIGKILL');
  assert.equal(await runtime.agents.waitForRunExit(idleRun.runId, 10_000), true);
  assert.equal(runtime.agents.peekAgentToken(agentId), undefined);
  assert.equal(runtime.agents.findLiveRun(idleRun.runId)?.status, 'error');
  const reopened = await runtime.chats.open(chat.id);
  assert.equal(reopened.runId, null);
  assert.equal(reopened.pending, 0);
  const current = live(runtime.chats).get(chat.id)!;
  assert.notEqual(current.handle, idle.handle);
  assert.notEqual(current.handle.teamToolName, idle.handle.teamToolName);
  assert.equal(current.handle.hasContext('hive-desktop-external'), true);
  assert.equal(current.handle.currentModel()?.id, 'gpt-6.1-terra');
  // A real startup recovery delivery may be enqueued; hold scheduling across ready().
  current.queue.resume = () => {};
  await runtime.chats.resume(chat.id);
  await current.queue.enqueue('durable pending external Pi work, never sent to a model');
  const pendingIds = current.queue.pending.map(item => item.id);
  const run = runtime.agents.getActiveRunByAgentId(workspace.id, agentId)!;
  const pendingPid = ownedPid();
  process.kill(pendingPid, 'SIGKILL');
  assert.equal(await runtime.agents.waitForRunExit(run.runId, 10_000), true);
  assert.equal(runtime.agents.getActiveRunByAgentId(workspace.id, agentId), undefined);
  assert.equal(runtime.agents.peekAgentToken(agentId), undefined);
  assert.throws(() => process.kill(pendingPid, 0));
  const restored = await runtime.chats.open(chat.id);
  assert.equal(restored.busy, false);
  assert.equal(restored.pending, pendingIds.length);
  assert.deepEqual(live(runtime.chats).get(chat.id)!.queue.pending.map(item => item.id), pendingIds);
  runtime.agents.configureAgentLaunch(workspace.id, agentId, {
    command: 'hive:pi', args: ['--provider', 'openai', '--model', 'gpt-6.1-terra'],
  });
  await assert.rejects(runtime.hive.request('POST', `/api/workspaces/${workspace.id}/agents/${encodeURIComponent(agentId)}/start`),
    error => error instanceof HiveHttpError && error.status >= 400);
  assert.equal(runtime.chats.list(workspace.id).find(record => record.id === chat.id)?.backend, 'pi-rpc');
  assert.ok(events.some(event => event.type === 'error' && event.sessionId === chat.id));
  await writeFile(join(directory, 'receipt.json'), JSON.stringify({ model: { provider: 'openai', id: 'gpt-6.1-terra' },
    noModelTurnScheduled: true, idlePid, pendingPid, reopenedPid: ownedPid(), pendingIds,
    backendMismatchRejected: true, restored }, null, 2));
  console.log(`EXTERNAL_PI_LIFECYCLE_OK ${directory}`);
} catch (error) {
  await writeFile(join(directory, 'failure.json'), JSON.stringify({ message: error instanceof Error ? error.message : String(error), stack: error instanceof Error ? error.stack : undefined }, null, 2));
  throw error;
} finally {
  await writeFile(join(directory, 'events.json'), JSON.stringify(events, null, 2));
  await runtime.close();
  console.log(`Evidence retained: ${directory}`);
}

/** Real Hive HTTP/SQLite, Pi SDK and owned installed Codex child. No model turns. */
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdtemp, mkdir, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { HiveHttpError } from '../src/hive-host.js';
import { createDesktopRuntime } from '../src/desktop-runtime.js';
import type { Chats, ChatEvent } from '../src/chats.js';
import type { DeliveryQueue } from '../src/delivery-queue.js';
import type { PiSessionHandle } from '../src/agents/types.js';

const directory = await mkdtemp(join(tmpdir(), 'hive-codex-lifecycle-'));
const saveEvidence = () => writeFile(join(directory, 'events.json'), JSON.stringify(events, null, 2));
const events: ChatEvent[] = [];
const runtime = await createDesktopRuntime(join(directory, 'state'), event => events.push(event));
const live = (chats: Chats) => (chats as unknown as {
  live: Map<string, { handle: PiSessionHandle; queue: DeliveryQueue }>;
}).live;
function ownedCodexPid(): number {
  const processes = execFileSync('ps', ['-axo', 'pid=,ppid=,command='], { encoding: 'utf8' }).trim().split('\n').map(line => {
    const match = /^\s*(\d+)\s+(\d+)\s+(.*)$/.exec(line)!;
    return { pid: Number(match[1]), ppid: Number(match[2]), command: match[3]! };
  });
  // The explicit installed executable is a native direct child. Pi's OAuth
  // helper may independently own a bundled app-server deeper in its process tree.
  const native = processes.filter(candidate => candidate.ppid === process.pid
    && /\/codex app-server --listen stdio:\/\//.test(candidate.command));
  assert.equal(native.length, 1, `Kill only this check process's single native app-server: ${JSON.stringify(native)}`);
  return native[0]!.pid;
}
try {
  let workspace = await runtime.hive.request('POST', '/api/workspaces', {
    path: directory, name: 'Runtime lifecycle regression', autostart_orchestrator: false,
  }) as { id: string };
  let agentId = `${workspace.id}:orchestrator`;
  const startPath = `/api/workspaces/${workspace.id}/agents/${encodeURIComponent(agentId)}/start`;
  const pi = await runtime.chats.create(workspace.id);
  await runtime.chats.setModel(pi.id, 'openai', 'gpt-6.1-terra');
  await runtime.chats.resume(pi.id);
  const piRun = runtime.agents.getActiveRunByAgentId(workspace.id, agentId)!;
  runtime.agents.stopAgentRun(piRun.runId);
  assert.equal(await runtime.agents.waitForRunExit(piRun.runId, 10_000), true);
  runtime.agents.configureAgentLaunch(workspace.id, agentId, { command: 'hive:codex', args: ['--provider', 'openai', '--model', 'gpt-6.1-terra'] });
  await assert.rejects(runtime.hive.request('POST', startPath), error => error instanceof HiveHttpError && error.status >= 400);
  assert.equal(runtime.agents.getActiveRunByAgentId(workspace.id, agentId), undefined);
  assert.equal(runtime.chats.list(workspace.id).find(record => record.id === pi.id)?.backend, 'pi');
  await runtime.chats.open(pi.id);
  // Startup may enqueue a real Hive recovery delivery. Disable only queue
  // scheduling so this lifecycle check cannot invoke any model.
  live(runtime.chats).get(pi.id)!.queue.resume = () => {};
  runtime.agents.configureAgentLaunch(workspace.id, agentId, { command: 'hive:pi', args: ['--provider', 'openai', '--model', 'gpt-6.1-terra'] });
  await runtime.hive.request('POST', startPath);
  assert.equal((await runtime.chats.open(pi.id)).model?.id, 'gpt-6.1-terra');
  const selectedRun = runtime.agents.getActiveRunByAgentId(workspace.id, agentId)!;
  runtime.agents.stopAgentRun(selectedRun.runId);
  assert.equal(await runtime.agents.waitForRunExit(selectedRun.runId, 10_000), true);
  runtime.agents.configureAgentLaunch(workspace.id, agentId, { command: 'hive:pi', args: ['--provider', 'openai', '--model', 'not-a-real-model'] });
  await assert.rejects(runtime.hive.request('POST', startPath), error => error instanceof HiveHttpError && error.status >= 400);
  assert.equal((await runtime.chats.open(pi.id)).model?.id, 'gpt-6.1-terra');

  await mkdir(join(directory, 'codex-project'));
  workspace = await runtime.hive.request('POST', '/api/workspaces', { path: join(directory, 'codex-project'), name: 'Codex process death', autostart_orchestrator: false }) as { id: string };
  agentId = `${workspace.id}:orchestrator`;
  const codex = await runtime.chats.create(workspace.id, 'codex');
  await runtime.chats.setModel(codex.id, 'openai', 'gpt-6.1-terra');
  await runtime.chats.resume(codex.id);
  const idle = live(runtime.chats).get(codex.id)!;
  const idleRun = runtime.agents.getActiveRunByAgentId(workspace.id, agentId)!;
  const idlePid = ownedCodexPid();
  process.kill(idlePid, 'SIGKILL');
  assert.equal(await runtime.agents.waitForRunExit(idleRun.runId, 10_000), true);
  assert.equal(runtime.agents.peekAgentToken(agentId), undefined);
  assert.equal((await runtime.chats.open(codex.id)).pending, 0);
  assert.notEqual(live(runtime.chats).get(codex.id)!.handle, idle.handle);
  live(runtime.chats).get(codex.id)!.queue.resume = () => {};
  await runtime.chats.resume(codex.id);
  const before = live(runtime.chats).get(codex.id)!;
  const run = runtime.agents.getActiveRunByAgentId(workspace.id, agentId)!;
  assert.ok(runtime.agents.peekAgentToken(agentId));
  await before.queue.pause();
  await before.queue.enqueue('durable pending delivery, never sent to a model');
  const pendingIds = before.queue.pending.map(item => item.id);
  const pending = before.queue.pending.at(-1)!;
  const pid = ownedCodexPid();
  process.kill(pid, 'SIGKILL');
  assert.equal(await runtime.agents.waitForRunExit(run.runId, 10_000), true);
  assert.equal(runtime.agents.getActiveRunByAgentId(workspace.id, agentId), undefined);
  assert.equal(runtime.agents.peekAgentToken(agentId), undefined);
  assert.equal(runtime.agents.findLiveRun(run.runId)?.status, 'error');
  assert.ok(events.some(event => event.type === 'error' && event.sessionId === codex.id && /Codex app-server/.test(event.message)));
  assert.throws(() => process.kill(pid, 0));
  const reopened = await runtime.chats.open(codex.id);
  assert.equal(reopened.runId, null);
  assert.equal(reopened.busy, false);
  assert.equal(reopened.pending, pendingIds.length);
  assert.notEqual(live(runtime.chats).get(codex.id)!.handle, before.handle);
  assert.deepEqual(live(runtime.chats).get(codex.id)!.queue.pending.map(item => item.id), pendingIds);
  assert.ok(live(runtime.chats).get(codex.id)!.queue.pending.some(item => item.id === pending.id));
  assert.notEqual(ownedCodexPid(), pid);
  await saveEvidence();
  await writeFile(join(directory, 'receipt.json'), JSON.stringify({
    noModelTurnScheduled: true, model: { provider: 'openai', id: 'gpt-6.1-terra' },
    pi: { id: pi.id, selectedModel: (await runtime.chats.open(pi.id)).model, backendSwitchRejected: true, invalidLaunchModelRejected: true },
    codex: { id: codex.id, selectedModel: reopened.model, idleKilledPid: idlePid, pendingKilledPid: pid, reopenedPid: ownedCodexPid(),
      pendingDeliveryIds: pendingIds, oldRunId: run.runId, oldRunStatus: runtime.agents.findLiveRun(run.runId)?.status },
  }, null, 2));
  console.log(`Lifecycle evidence: ${directory}`);
  console.log('Codex lifecycle passed: explicit backend switch rejected with transcript preserved, launch model applied/invalid model rejected, owned native child death revokes Hive run/token and reaps handle, durable pending delivery reopened without a model turn.');
} catch (error) {
  await saveEvidence();
  await writeFile(join(directory, 'failure.json'), JSON.stringify({ message: error instanceof Error ? error.message : String(error) }, null, 2));
  console.error(`Failed lifecycle evidence retained: ${directory}`);
  throw error;
} finally {
  await runtime.close();
}

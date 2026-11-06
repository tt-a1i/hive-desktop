/** Real Hive HTTP/SQLite + installed Codex app-server + team CLI, no model turns. */
import assert from 'node:assert/strict';
import { mkdtemp, readFile, rm, mkdir, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { ChatIndex } from '../src/chat-index.js';
import { createDesktopRuntime } from '../src/desktop-runtime.js';
import { createTeamCommand } from '../src/team-tool.js';
import type { Chats } from '../src/chats.js';
import type { DeliveryQueue } from '../src/delivery-queue.js';
import type { PiSessionHandle } from '../src/agents/types.js';

const directory = await mkdtemp(join(tmpdir(), 'hive-codex-runtime-'));
type Runtime = Awaited<ReturnType<typeof createDesktopRuntime>>;
const live = (chats: Chats) => (chats as unknown as {
  live: Map<string, { queue: DeliveryQueue; handle: PiSessionHandle }>;
}).live;
let runtime: Runtime | undefined;
try {
  const legacyDir = join(directory, 'legacy-index');
  await mkdir(legacyDir);
  await writeFile(join(legacyDir, 'index.json'), JSON.stringify([{ id: 'old', workspaceId: 'old-workspace', title: 'Old Pi', sessionFile: null, updatedAt: 1 }]));
  assert.equal(new ChatIndex(legacyDir).get('old').backend, 'pi');
  const dataDir = join(directory, 'state');
  runtime = await createDesktopRuntime(dataDir, () => {});
  const workspace = await runtime.hive.request('POST', '/api/workspaces', {
    path: directory, name: 'Codex runtime integration', autostart_orchestrator: false,
  }) as { id: string };
  const chat = await runtime.chats.create(workspace.id, 'codex');
  assert.equal(chat.backend, 'codex');
  assert.equal(chat.sessionFile, null);
  assert.equal(chat.codexThreadId, undefined, 'Fresh thread has no durable Codex rollout yet');
  const model = chat.models.find(candidate => candidate.id === 'gpt-6.1-terra');
  assert.ok(model, 'Installed Codex must offer the user-specified Terra model');
  await runtime.chats.setModel(chat.id, model.provider, model.id);
  await runtime.chats.resume(chat.id);
  const agentId = `${workspace.id}:orchestrator`;
  const handle = live(runtime.chats).get(chat.id)!.handle;
  assert.equal(handle.hasContext('hive-context'), true);
  assert.equal(handle.hasContext('hive-desktop'), true);
  assert.equal(handle.history().length, 0, 'Hidden context is independent from display projection');
  assert.equal(runtime.agents.peekAgentLaunchConfig(workspace.id, agentId)?.command, 'hive:codex');
  const tool = () => {
    const token = runtime!.agents.peekAgentToken(agentId);
    assert.ok(token);
    return createTeamCommand({ workspaceId: workspace.id, agentId, token,
      hivePort: String(runtime!.hive.port), cwd: directory });
  };
  await tool().execute('spawn', { args: ['spawn', 'coder', '--name', 'codex-worker'] });
  const members = await runtime.hive.request('GET', `/api/ui/workspaces/${workspace.id}/team`) as { id: string; name: string }[];
  const worker = members.find(member => member.name === 'codex-worker');
  assert.ok(worker);
  const launch = runtime.agents.peekAgentLaunchConfig(workspace.id, worker.id);
  assert.equal(launch?.command, 'hive:codex');
  assert.deepEqual(launch.args, ['--provider', model.provider, '--model', model.id]);
  await runtime.hive.request('POST', `/api/workspaces/${workspace.id}/agents/${encodeURIComponent(worker.id)}/start`);
  const workerChat = runtime.chats.list(workspace.id).find(candidate => candidate.agentId === worker.id);
  assert.ok(workerChat);
  assert.equal(workerChat.backend, 'codex');
  await live(runtime.chats).get(workerChat.id)!.queue.pause();
  await live(runtime.chats).get(chat.id)!.queue.pause();
  const sent = await tool().execute('send', { args: ['send', 'codex-worker', 'persisted codex task'] });
  const dispatchId = (JSON.parse(sent.content[0]!.text) as { dispatch_id: string }).dispatch_id;
  assert.ok(dispatchId);
  assert.equal(live(runtime.chats).get(workerChat.id)!.queue.pending[0]?.presentation?.dispatchId, dispatchId);
  const token = runtime.agents.peekAgentToken(worker.id);
  assert.ok(token);
  const workerTool = createTeamCommand({ workspaceId: workspace.id, agentId: worker.id, token,
    hivePort: String(runtime.hive.port), cwd: directory });
  const messages = await workerTool.execute('messages', { args: ['messages', '--dispatch', dispatchId] });
  const inbox = JSON.parse(messages.content[0]!.text) as { required_seen_seq: number };
  await workerTool.execute('report', { args: ['report', 'explicit Codex report', '--dispatch', dispatchId,
    '--seen', String(inbox.required_seen_seq)] });
  const dispatches = await runtime.hive.request('GET', `/api/ui/workspaces/${workspace.id}/dispatches`) as { id: string; state: string; report_text: string }[];
  const reported = dispatches.find(candidate => candidate.id === dispatchId);
  assert.equal(reported?.state, 'reported');
  assert.equal(reported?.report_text, 'explicit Codex report');
  assert.ok(live(runtime.chats).get(chat.id)!.queue.pending.some(item => item.text.includes('explicit Codex report')));
  await runtime.close();
  runtime = undefined;
  const index = JSON.parse(await readFile(join(dataDir, 'chats', 'index.json'), 'utf8')) as { backend: string; codexThreadId?: string; piSessionId?: string }[];
  assert.ok(index.every(record => record.backend === 'codex' && record.codexThreadId === undefined && record.piSessionId === undefined));
  runtime = await createDesktopRuntime(dataDir, () => {});
  const restored = await runtime.chats.open(chat.id);
  assert.equal(restored.backend, 'codex');
  assert.equal(restored.pending, 1);
  assert.equal(restored.busy, false);
  assert.equal(live(runtime.chats).get(chat.id)!.handle.hasContext('hive-context'), true);
  assert.equal(live(runtime.chats).get(chat.id)!.handle.history().length, 0);
  console.log('Codex runtime passed: real Hive HTTP/SQLite, fresh thread identity, hidden context restore, native CLI spawn/backend/model inheritance, durable send/seen/report delivery. No model turn sent.');
} finally {
  await runtime?.close();
  await rm(directory, { recursive: true, force: true });
}

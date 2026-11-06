/** Real Hive HTTP/SQLite + installed external Pi RPC + team CLI, no model turns. */
import assert from 'node:assert/strict';
import { mkdtemp, readFile, mkdir, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { ChatIndex } from '../src/chat-index.js';
import { createDesktopRuntime } from '../src/desktop-runtime.js';
import { createTeamCommand } from '../src/team-tool.js';
import type { Chats } from '../src/chats.js';
import type { DeliveryQueue } from '../src/delivery-queue.js';
import type { PiSessionHandle } from '../src/agents/types.js';

const directory = await mkdtemp(join(tmpdir(), 'hive-external-pi-runtime-'));
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
    path: directory, name: 'External Pi runtime integration', autostart_orchestrator: false,
  }) as { id: string };
  const chat = await runtime.chats.create(workspace.id, 'pi-rpc');
  assert.equal(chat.backend, 'pi-rpc');
  assert.ok(chat.externalPiSessionId);
  assert.equal(chat.piSessionId, undefined);
  assert.equal(chat.codexThreadId, undefined);
  assert.ok(chat.sessionFile?.includes('/external-pi/'));
  const toolName = (live(runtime.chats).get(chat.id)!.handle as PiSessionHandle & { teamToolName: string }).teamToolName;
  const model = chat.models.find(candidate => candidate.id === 'gpt-6.1-terra' && candidate.provider === 'openai');
  assert.ok(model, 'Installed external Pi must offer the user-specified Terra model');
  await runtime.chats.setModel(chat.id, model.provider, model.id);
  await runtime.chats.resume(chat.id);
  const agentId = `${workspace.id}:orchestrator`;
  const handle = live(runtime.chats).get(chat.id)!.handle;
  assert.equal(handle.hasContext('hive-context'), true);
  assert.equal(handle.hasContext('hive-desktop'), true);
  assert.equal(handle.hasContext('hive-desktop-external'), true);
  assert.equal(runtime.agents.peekAgentLaunchConfig(workspace.id, agentId)?.command, 'hive:pi-rpc');
  const tool = () => {
    const token = runtime!.agents.peekAgentToken(agentId);
    assert.ok(token);
    return createTeamCommand({ workspaceId: workspace.id, agentId, token,
      hivePort: String(runtime!.hive.port), cwd: directory });
  };
  await tool().execute('spawn', { args: ['spawn', 'coder', '--name', 'external-pi-worker'] });
  const members = await runtime.hive.request('GET', `/api/ui/workspaces/${workspace.id}/team`) as { id: string; name: string }[];
  const worker = members.find(member => member.name === 'external-pi-worker');
  assert.ok(worker);
  const launch = runtime.agents.peekAgentLaunchConfig(workspace.id, worker.id);
  assert.equal(launch?.command, 'hive:pi-rpc');
  assert.deepEqual(launch.args, ['--provider', model.provider, '--model', model.id]);
  await runtime.hive.request('POST', `/api/workspaces/${workspace.id}/agents/${encodeURIComponent(worker.id)}/start`);
  const workerChat = runtime.chats.list(workspace.id).find(candidate => candidate.agentId === worker.id);
  assert.ok(workerChat);
  assert.equal(workerChat.backend, 'pi-rpc');
  await live(runtime.chats).get(workerChat.id)!.queue.pause();
  await live(runtime.chats).get(chat.id)!.queue.pause();
  const sent = await tool().execute('send', { args: ['send', 'external-pi-worker', 'persisted external Pi task'] });
  const dispatchId = (JSON.parse(sent.content[0]!.text) as { dispatch_id: string }).dispatch_id;
  assert.ok(dispatchId);
  assert.equal(live(runtime.chats).get(workerChat.id)!.queue.pending[0]?.presentation?.dispatchId, dispatchId);
  const token = runtime.agents.peekAgentToken(worker.id);
  assert.ok(token);
  const workerTool = createTeamCommand({ workspaceId: workspace.id, agentId: worker.id, token,
    hivePort: String(runtime.hive.port), cwd: directory });
  const messages = await workerTool.execute('messages', { args: ['messages', '--dispatch', dispatchId] });
  const inbox = JSON.parse(messages.content[0]!.text) as { required_seen_seq: number };
  await workerTool.execute('report', { args: ['report', 'explicit external Pi report', '--dispatch', dispatchId,
    '--seen', String(inbox.required_seen_seq)] });
  const dispatches = await runtime.hive.request('GET', `/api/ui/workspaces/${workspace.id}/dispatches`) as { id: string; state: string; report_text: string }[];
  const reported = dispatches.find(candidate => candidate.id === dispatchId);
  assert.equal(reported?.state, 'reported');
  assert.equal(reported?.report_text, 'explicit external Pi report');
  assert.ok(live(runtime.chats).get(chat.id)!.queue.pending.some(item => item.text.includes('explicit external Pi report')));
  await runtime.close();
  runtime = undefined;
  const index = JSON.parse(await readFile(join(dataDir, 'chats', 'index.json'), 'utf8')) as { backend: string; codexThreadId?: string; piSessionId?: string; externalPiSessionId?: string }[];
  assert.ok(index.every(record => record.backend === 'pi-rpc' && record.codexThreadId === undefined && record.piSessionId === undefined && !!record.externalPiSessionId));
  runtime = await createDesktopRuntime(dataDir, () => {});
  const restored = await runtime.chats.open(chat.id);
  assert.equal(restored.backend, 'pi-rpc');
  assert.equal(restored.pending, 1);
  assert.equal(restored.busy, false);
  assert.equal(live(runtime.chats).get(chat.id)!.handle.hasContext('hive-context'), true);
  const restoredHandle = live(runtime.chats).get(chat.id)!.handle as PiSessionHandle & { teamToolName: string };
  assert.equal(restoredHandle.hasContext('hive-desktop-external'), true);
  assert.notEqual(restoredHandle.teamToolName, toolName, 'New subprocess must advertise its fresh team tool name');
  assert.equal(restored.externalPiSessionId, restoredHandle.sessionId);
  await writeFile(join(directory, 'receipt.json'), JSON.stringify({ backend: 'pi-rpc', model, chat, workerChat, reported, restored, oldToolName: toolName, currentToolName: restoredHandle.teamToolName }, null, 2));
  console.log('External Pi runtime passed: real Hive HTTP/SQLite, external session identity, current-process hidden tool guidance restore, native CLI spawn/backend/model inheritance, durable send/seen/report delivery. No model turn sent.');
} catch (error) {
  await writeFile(join(directory, 'failure.json'), JSON.stringify({ message: error instanceof Error ? error.message : String(error) }, null, 2));
  throw error;
} finally {
  await runtime?.close();
  console.log(`External Pi runtime evidence: ${directory}`);
}

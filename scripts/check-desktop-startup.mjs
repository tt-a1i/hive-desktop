import assert from 'node:assert/strict';
import { execFileSync, spawn } from 'node:child_process';
import { once } from 'node:events';
import { mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { DatabaseSync } from 'node:sqlite';
import { setTimeout as delay } from 'node:timers/promises';
import { fileURLToPath, pathToFileURL } from 'node:url';

const flag = process.argv.indexOf('--bundle');
assert.ok(flag >= 0 && process.argv[flag + 1], 'Pass --bundle followed by the extracted package directory');
const bundle = resolve(process.argv[flag + 1]);
const mac = process.platform === 'darwin';
const contents = join(bundle, 'Hive Desktop.app', 'Contents');
const resources = mac ? join(contents, 'Resources') : join(bundle, 'resources');
const executable = mac ? join(contents, 'MacOS', 'hive-desktop') : join(bundle, 'hive-desktop.exe');
const { createDesktopRuntime } = await import(pathToFileURL(join(resources, 'runtime/dist/desktop-runtime.js')).href);
const directory = await mkdtemp(join(tmpdir(), 'hive-native-restore-'));
const state = join(directory, 'state');
let child;
let exited;
let reader;
try {
  const runtime = await createDesktopRuntime(state, () => {});
  let workspaceId;
  let chatId;
  let leadChat;
  let workerChat;
  let workerFile;
  let dispatch;
  try {
    for (const name of ['Alpha', 'Beta']) {
      const path = join(directory, name);
      await mkdir(path);
      await runtime.hive.request('POST', '/api/workspaces', { path, name, autostart_orchestrator: false });
    }
    const projects = await runtime.hive.request('GET', '/api/workspaces');
    workspaceId = projects[1].id;
    const base = `/api/workspaces/${workspaceId}`;
    const member = await runtime.hive.request('POST', `${base}/workers`, { name: 'Reviewer', role: 'reviewer', autostart: false });
    await runtime.hive.request('POST', `${base}/agents/${member.id}/config`, { command: 'hive:pi', args: [] });
    await runtime.hive.request('POST', `${base}/agents/${member.id}/start`);
    const worker = runtime.chats.list(workspaceId).find(chat => chat.agentId === member.id);
    workerChat = worker?.id;
    workerFile = worker?.sessionFile;
    assert.ok(workerChat, 'Starting the worker must create its actual member session');
    // Pause both real delivery queues before dispatch so the fixture exercises
    // the ledger and the pending report without starting model inference.
    leadChat = (await runtime.chats.create(workspaceId)).id;
    await runtime.chats.resume(leadChat);
    await runtime.chats.live.get(leadChat).queue.pause();
    await runtime.chats.live.get(workerChat).queue.pause();
    const { createTeamCommand } = await import(pathToFileURL(join(resources, 'runtime/dist/team-tool.js')).href);
    const team = async (agentId, args) => {
      const token = runtime.agents.peekAgentToken(agentId);
      assert.ok(token);
      const cmd = createTeamCommand({ workspaceId, agentId, token, hivePort: String(runtime.hive.port), cwd: join(directory, 'Beta') });
      return (await cmd.execute('startup-check', { args })).content[0].text;
    };
    dispatch = JSON.parse(await team(`${workspaceId}:orchestrator`, ['send', 'Reviewer', '核对项目启动方式'])).dispatch_id;
    const tester = await runtime.hive.request('POST', `${base}/workers`, { name: 'Tester', role: 'tester', autostart: false });
    await runtime.hive.request('POST', `${base}/agents/${tester.id}/config`, { command: 'hive:pi', args: [] });
    await runtime.hive.request('POST', `${base}/agents/${tester.id}/start`);
    const testerChat = runtime.chats.list(workspaceId).find(chat => chat.agentId === tester.id);
    await runtime.chats.live.get(testerChat.id).queue.pause();
    const child = JSON.parse(await team(`${workspaceId}:orchestrator`, ['send', 'Tester', '核对测试入口', '--related-to', dispatch])).dispatch_id;
    const question = JSON.parse(await team(tester.id, ['message', '--dispatch', dispatch, '--from-dispatch', child, '--kind', 'question', '从哪个目录运行测试？'])).message;
    const answer = JSON.parse(await team(member.id, ['message', '--dispatch', child, '--from-dispatch', dispatch, '--kind', 'answer', '--reply-to', question.id, '从项目根目录运行。'])).message;
    for (const id of [dispatch, child]) {
      const path = `/api/ui/workspaces/${workspaceId}/dispatches/${id}/messages?scope=collaboration`;
      const history = await runtime.requestHive('GET', path);
      assert.deepEqual(new Set(history.related_dispatches.map(task => task.owner_name)), new Set(['Reviewer', 'Tester']));
      assert.equal(history.messages.find(m => m.id === answer.id)?.reply_to, question.id);
      const later = await runtime.requestHive('GET', `${path}&after_message_id=${encodeURIComponent(question.id)}`);
      assert.deepEqual(later.messages.map(m => m.id), [answer.id]);
    }
    const inbox = JSON.parse(await team(member.id, ['messages', '--dispatch', dispatch]));
    await writeFile(join(directory, 'Beta', 'review.txt'), 'pnpm dev');
    await team(member.id, ['report', '已核对启动方式。', '--dispatch', dispatch, '--seen', String(inbox.required_seen_seq), '--artifact', 'review.txt']);
    assert.equal((await runtime.chats.open(leadChat)).pendingReports[0].dispatchId, dispatch);
    chatId = (await runtime.chats.create(workspaceId)).id;
    assert.equal(runtime.chats.list(workspaceId)[0].id, chatId);
    runtime.hive.saveNavigation(JSON.stringify({ workspace: workspaceId, sessions: { [workspaceId]: 'removed-chat' } }));
  } finally {
    await runtime.close();
  }

  // A deterministic transcript in the isolated worker's real Pi file exercises
  // native rendering and disclosure controls without a model account in Windows CI.
  assert.ok(workerFile);
  const { createDesktopSessionManager } = await import(pathToFileURL(join(resources, 'runtime/dist/agents/pi-session-manager.js')).href);
  const history = createDesktopSessionManager(join(directory, 'Beta'), dirname(workerFile), workerFile);
  const prompt = '请检查这个项目的启动方式和测试入口。';
  history.appendCustomMessageEntry('hive-delivery', prompt, false,
    { version: 1, origin: 'user', displayText: prompt, deliveryId: 'fixture-input' });
  const assistant = (content, stopReason) => history.appendMessage({ role: 'assistant', content, timestamp: Date.now(),
    api: 'openai-responses', provider: 'openai', model: 'gpt-6.1-terra', stopReason,
    usage: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, totalTokens: 0,
      cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, total: 0 } } });
  const reads = [['README.md', '使用 pnpm dev 启动开发环境。'], ['package.json', '{"scripts":{"test":"node --test"}}']];
  assistant(reads.map(([path], index) => ({ type: 'toolCall', id: `read-${index}`, name: 'read', arguments: { path } })), 'toolUse');
  for (const [index, [, text]] of reads.entries()) {
    history.appendMessage({ role: 'toolResult', toolCallId: `read-${index}`, toolName: 'read',
      content: [{ type: 'text', text }], isError: false, timestamp: Date.now() });
  }
  assistant([{ type: 'text', text: '项目用 `pnpm dev` 启动。测试入口是 `pnpm test`，对应 Node 自带的测试运行器。' }], 'stop');
  const persisted = createDesktopSessionManager(join(directory, 'Beta'), dirname(workerFile), workerFile);
  assert.deepEqual(persisted.buildSessionContext().messages.filter(m => m.role === 'toolResult').map(m => m.toolCallId), ['read-0', 'read-1']);

  // The native window must select the second project, successfully open its chat
  // and replace the obsolete selection. Booting just the sidecar cannot do this.
  const windowsPreview = process.platform === 'win32' && process.env.HIVE_WINDOWS_PREVIEW_PATH;
  const existingDraft = '请保留这条跟进问题。';
  const draftDir = join(state, 'desktop-drafts');
  const workerDraft = join(draftDir, `session-${workerChat}.json`);
  if (windowsPreview) {
    await mkdir(draftDir, { recursive: true });
    await writeFile(workerDraft, JSON.stringify({ key: `session:${workerChat}`, input: existingDraft, failed: [] }));
  }
  const env = { ...process.env, HIVE_DESKTOP_DATA_DIR: state, HIVE_DESKTOP_TRACE_RPC: '1' };
  delete env.HIVE_DESKTOP_NODE;
  delete env.HIVE_DESKTOP_RUNTIME;
  child = spawn(executable, [], { env, cwd: directory, stdio: 'inherit' });
  exited = once(child, 'exit');
  reader = new DatabaseSync(join(state, 'hive/runtime.sqlite'), { readOnly: true });
  const read = reader.prepare("SELECT value FROM app_state WHERE key = 'desktop_navigation'");
  async function waitForSelection(expected) {
    const deadline = Date.now() + 60_000;
    while (Date.now() < deadline) {
      const selected = JSON.parse(read.get().value);
      if (selected.workspace === workspaceId && selected.sessions[workspaceId] === expected) return;
      if (child.exitCode !== null || child.signalCode !== null) break;
      await delay(200);
    }
    assert.fail(`Native window did not open chat ${expected} (exit ${child.exitCode}, signal ${child.signalCode})`);
  }
  await waitForSelection(chatId);
  if (windowsPreview) {
    const capture = fileURLToPath(new URL('./capture-windows-preview.ps1', import.meta.url));
    execFileSync('powershell.exe', ['-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', capture,
      '-AppProcessId', String(child.pid), '-WorkerFile', workerFile, '-WorkerPrompt', prompt, '-ExistingDraft', existingDraft, '-DispatchId', dispatch, '-TasksFile', join(directory, 'Beta', '.hive', 'tasks.md'), '-OutputPath', resolve(windowsPreview)],
    { stdio: 'inherit', timeout: 60_000 });
    await waitForSelection(leadChat);
    const commands = reader.prepare("SELECT value FROM app_state WHERE key = 'desktop_executables'").get();
    assert.equal(JSON.parse(commands.value).codexCommand, String.raw`C:\Fixture Programs\codex.exe`);
    assert.equal(JSON.parse(await readFile(workerDraft, 'utf8')).input, `> ${prompt}\n\n${existingDraft}`);
  }
  console.log('Native desktop opened the saved project and recovered its missing chat selection from the extracted bundle.');
} finally {
  reader?.close();
  if (child && child.exitCode === null && child.signalCode === null) { child.kill(); await exited; }
  await rm(directory, { recursive: true, force: true, maxRetries: 10, retryDelay: 250 });
}

import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { DatabaseSync } from 'node:sqlite';
import { setTimeout as delay } from 'node:timers/promises';
import { fileURLToPath, pathToFileURL } from 'node:url';

const flag = process.argv.indexOf('--bundle');
assert.ok(flag >= 0 && process.argv[flag + 1], 'Pass --bundle followed by the extracted package directory');
const bundle = resolve(process.argv[flag + 1]);
const languageFlag = process.argv.indexOf('--language');
const language = languageFlag < 0 ? undefined : process.argv[languageFlag + 1];
assert.ok(languageFlag < 0 || language === 'zh' || language === 'en', 'Use --language zh or --language en');
assert.ok(!(process.platform === 'win32' && process.env.HIVE_WINDOWS_PREVIEW_PATH && language === 'en'),
  'Windows preview controls require Chinese; use --language zh or omit HIVE_WINDOWS_PREVIEW_PATH for an English startup-only check');
const mac = process.platform === 'darwin';
const contents = join(bundle, 'Hive Desktop.app', 'Contents');
const resources = mac ? join(contents, 'Resources') : join(bundle, 'resources');
const executable = mac ? join(contents, 'MacOS', 'hive-desktop') : join(bundle, 'hive-desktop.exe');
const { createDesktopRuntime } = await import(pathToFileURL(join(resources, 'runtime/dist/desktop-runtime.js')).href);
const directory = await mkdtemp(join(tmpdir(), 'hive-native-restore-'));
const state = join(directory, 'state');
const windowsPreview = process.platform === 'win32' && process.env.HIVE_WINDOWS_PREVIEW_PATH;
const windowsHtml = windowsPreview && process.env.HIVE_WINDOWS_HTML_PREVIEW === '1';
const htmlPath = join(directory, 'Beta', 'review.html');
const canaryRequests = [];
const canary = windowsHtml ? createServer((request, response) => {
  canaryRequests.push(request.url);
  response.end('Loopback canary');
}) : undefined;
let child;
let exited;
let reader;
try {
  if (canary) {
    process.env.PI_CODING_AGENT_DIR = join(directory, 'offline-pi');
    process.env.PI_OFFLINE = '1';
    await mkdir(process.env.PI_CODING_AGENT_DIR);
    canary.listen(0, '127.0.0.1');
    await once(canary, 'listening');
  }
  const runtime = await createDesktopRuntime(state, () => {});
  let workspaceId;
  let chatId;
  let leadChat;
  let workerChat;
  let workerFile;
  let testerSessionId;
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
    assert.ok(testerChat?.id, 'Starting Tester must create its actual member session');
    testerSessionId = testerChat.id;
    await runtime.chats.live.get(testerChat.id).queue.pause();
    const child = JSON.parse(await team(`${workspaceId}:orchestrator`, ['send', 'Tester', '核对测试入口', '--related-to', dispatch])).dispatch_id;
    const originDeadline = Date.now() + 8000;
    let testerOrigin;
    while (!testerOrigin && Date.now() < originDeadline) {
      try { testerOrigin = await runtime.chats.dispatchSession(workspaceId, child, tester.id); }
      catch (error) {
        if (error.code !== 'DISPATCH_SOURCE_UNKNOWN') throw error;
        await delay(100);
      }
    }
    assert.equal(testerOrigin?.sourceSessionId, testerSessionId, 'Child dispatch must retain the real Tester session');
    assert.equal(testerOrigin?.sourceAgentId, tester.id);
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
    const artifacts = ['--artifact', 'review.txt'];
    if (canary) {
      const hostFile = join(directory, 'local-canary.txt');
      await writeFile(hostFile, 'Fixture file outside the project');
      const origin = 'http://127.0.0.1:' + canary.address().port;
      await writeFile(htmlPath, `<!doctype html><meta charset="utf-8">
        <button id="add">Increment packaged artifact</button><p id="state">Artifact has not run</p>
        <button id="picker">Check host file selection</button><input type="file" id="file"><p id="picker-result"></p>
        <button id="navigate">Check blocked navigation</button><p id="isolation">Checking artifact isolation</p>
        <p id="script-state">Artifact script not started</p><p id="navigation-result"></p><p id="increment-result"></p><div id="checks"></div>
        <script>
          document.querySelector('#script-state').textContent = 'Artifact script running';
          const describeError = error => String(error && (error.stack || error.message) || error);
          window.addEventListener('error', event => {
            document.querySelector('#script-state').textContent = 'Artifact script error: ' + describeError(event.error || event.message);
          });
          window.addEventListener('unhandledrejection', event => {
            document.querySelector('#script-state').textContent = 'Artifact promise error: ' + describeError(event.reason);
          });
          function report(name, result) {
            const line = document.createElement('p');
            line.textContent = name + ': ' + result;
            document.querySelector('#checks').append(line);
          }
          const canary = ${JSON.stringify(origin)};
          function denied(access) { try { access(); return false; } catch { return true; } }
          function check(name, verify) {
            try {
              const passed = verify();
              report(name, passed ? 'passed' : 'FAILED');
              return passed;
            } catch (error) {
              report(name, 'ERROR ' + describeError(error));
              return false;
            }
          }
          const isolated = [
            check('Parent document inaccessible', () => denied(() => parent.document.body)),
            check('Opaque-origin storage inaccessible', () => denied(() => localStorage.getItem('fixture'))),
            check('Realtime peer API absent', () => typeof RTCPeerConnection === 'undefined'),
            check('Host IPC absent', () => typeof window.ipc === 'undefined'),
            check('Popup rejected', () => window.open(canary + '/popup') == null),
          ].every(Boolean);
          const imageBlocked = new Promise(resolve => {
            const image = new Image();
            image.onload = () => resolve(false);
            image.onerror = () => resolve(true);
            image.src = canary + '/image';
          });
          const blocked = url => fetch(url).then(() => false, () => true);
          const observe = (name, promise) => {
            report(name, 'pending');
            return promise.then(passed => {
              report(name, passed ? 'passed' : 'FAILED');
              return passed;
            });
          };
          Promise.all([
            observe('Image request rejected', imageBlocked),
            observe('Fetch request rejected', blocked(canary + '/fetch')),
            observe('Local file request rejected', blocked(${JSON.stringify(pathToFileURL(hostFile).href)})),
          ]).then(results => {
            document.querySelector('#isolation').textContent =
              isolated && results.every(Boolean) ? 'Artifact isolation verified' : 'Artifact isolation failed';
            document.querySelector('#script-state').textContent = 'Artifact checks completed';
          });
          document.querySelector('#navigate').onclick = () => {
            document.querySelector('#navigation-result').textContent = 'Artifact navigation attempted';
            location.href = canary + '/navigate';
            setTimeout(() => document.querySelector('#navigation-result').textContent = 'Artifact retained after navigation', 250);
          };
          document.querySelector('#picker').onclick = () => {
            const input = document.querySelector('#file');
            const clickBlocked = !input.dispatchEvent(new MouseEvent('click', {bubbles: true, cancelable: true}));
            let pickerBlocked = typeof input.showPicker !== 'function';
            if (!pickerBlocked) {
              try { input.showPicker(); } catch (error) { pickerBlocked = error.name === 'NotAllowedError'; }
            }
            document.querySelector('#picker-result').textContent =
              clickBlocked && pickerBlocked && typeof showOpenFilePicker === 'undefined'
              ? 'Host file selection blocked' : 'Host file selection allowed';
          };
          const state = document.querySelector('#state');
          state.textContent = isolated ? 'Packaged artifact counter 0' : 'Artifact isolation failed';
          document.querySelector('#add').onclick = () => {
            document.querySelector('#increment-result').textContent = 'Increment handler ran';
            if (isolated) state.textContent = 'Packaged artifact counter 1';
          };
        </script>`);
      artifacts.push('--artifact', 'review.html');
    }
    await team(member.id, ['report', '已核对启动方式。', '--dispatch', dispatch, '--seen', String(inbox.required_seen_seq), ...artifacts]);
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
  assistant([{ type: 'thinking', thinking: '先核对原生会话的启动方式，再检查测试入口。' }], 'stop');
  const reads = [['README.md', '使用 pnpm dev 启动开发环境。'], ['package.json', '{"scripts":{"test":"node --test"}}']];
  assistant(reads.map(([path], index) => ({ type: 'toolCall', id: `read-${index}`, name: 'read', arguments: { path } })), 'toolUse');
  for (const [index, [, text]] of reads.entries()) {
    history.appendMessage({ role: 'toolResult', toolCallId: `read-${index}`, toolName: 'read',
      content: [{ type: 'text', text }], isError: false, timestamp: Date.now() });
  }
  assistant([{ type: 'text', text: '项目用 `pnpm dev` 启动。测试入口是 `pnpm test`，对应 Node 自带的测试运行器。' }], 'stop');
  const persisted = createDesktopSessionManager(join(directory, 'Beta'), dirname(workerFile), workerFile);
  assert.deepEqual(persisted.buildSessionContext().messages.filter(m => m.role === 'toolResult').map(m => m.toolCallId), ['read-0', 'read-1']);
  assert.ok(persisted.buildSessionContext().messages.some(m => m.role === 'assistant' && m.content.some(block => block.type === 'thinking' && block.thinking === '先核对原生会话的启动方式，再检查测试入口。')));

  // The native window must select the second project, successfully open its chat
  // and replace the obsolete selection. Booting just the sidecar cannot do this.
  const existingDraft = '请保留这条跟进问题。';
  const testerDraft = 'Tester 原成员会话的独立草稿。';
  const draftDir = join(state, 'desktop-drafts');
  const workerDraft = join(draftDir, `session-${workerChat}.json`);
  if (windowsPreview) {
    await mkdir(draftDir, { recursive: true });
    await writeFile(workerDraft, JSON.stringify({ key: `session:${workerChat}`, input: existingDraft, failed: [] }));
    await writeFile(join(draftDir, `session-${testerSessionId}.json`), JSON.stringify({ key: `session:${testerSessionId}`, input: testerDraft, failed: [] }));
    await writeFile(join(state, 'desktop-appearance.json'), JSON.stringify({
      mode: 'system', font_size: 16, code_font_size: 13, code_word_wrap: true,
    }));
  }
  const env = { ...process.env, HIVE_DESKTOP_DATA_DIR: state, HIVE_DESKTOP_TRACE_RPC: '1' };
  if (language || windowsHtml) await writeFile(join(state, 'desktop-language.json'), JSON.stringify(language ?? 'zh'));
  delete env.HIVE_DESKTOP_NODE;
  delete env.HIVE_DESKTOP_RUNTIME;
  function launch() {
    child = spawn(executable, [], { env, cwd: directory, stdio: 'inherit' });
    exited = once(child, 'exit');
  }
  launch();
  reader = new DatabaseSync(join(state, 'hive/runtime.sqlite'), { readOnly: true });
  const readProfiles = reader.prepare("SELECT value FROM app_state WHERE key = 'desktop_agent_profiles'");
  const profilesBefore = JSON.parse(readProfiles.get().value);
  const codexBefore = profilesBefore.profiles.find(profile => profile.id === 'default-codex');
  assert.ok(codexBefore);
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
    async function captureWindow(reopenOnly) {
      const captureEnv = {...process.env, HIVE_WINDOWS_HTML_REOPEN_ONLY: reopenOnly ? '1' : '0'};
      const probe = spawn('powershell.exe', ['-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', capture,
        '-AppProcessId', String(child.pid), '-WorkerFile', workerFile, '-WorkerPrompt', prompt, '-ExistingDraft', existingDraft, '-TesterDraft', testerDraft, '-DispatchId', dispatch, '-DataDirectory', state, '-TasksFile', join(directory, 'Beta', '.hive', 'tasks.md'), '-OutputPath', resolve(windowsPreview)],
      { env: captureEnv, stdio: 'inherit', timeout: reopenOnly ? 85_000 : 150_000 });
      assert.deepEqual(await once(probe, 'exit'), [0, null]);
    }
    await captureWindow(false);
    await waitForSelection(leadChat);
    const renamedLead = JSON.parse(await readFile(join(state, 'chats/index.json'), 'utf8')).find(session => session.id === leadChat);
    assert.equal(renamedLead.title, '启动方式核验');
    assert.equal(renamedLead.customTitle, true);
    const appearance = JSON.parse(await readFile(join(state, 'desktop-appearance.json'), 'utf8'));
    assert.equal(appearance.mode, 'system');
    assert.equal(appearance.font_size, 16);
    assert.equal(appearance.code_font_size, 13, 'Changing the theme must preserve the code font size');
    assert.equal(appearance.code_word_wrap, true, 'Changing the theme must preserve code wrapping');
    const leadDraft = join(draftDir, `session-${leadChat}.json`);
    // The native restart flushes the unsubmitted input before replacing its sidecar.
    assert.equal(JSON.parse(await readFile(leadDraft, 'utf8')).input, '后台恢复核验草稿');
    // The canonical profile row publishes defaults and compatibility commands atomically.
    const profilesAfter = JSON.parse(readProfiles.get().value);
    const codexAfter = profilesAfter.profiles.find(profile => profile.id === 'default-codex');
    assert.ok(codexAfter.currentRevision > codexBefore.currentRevision);
    assert.deepEqual(codexAfter.revisions.find(revision => revision.revision === codexBefore.currentRevision),
      codexBefore.revisions.find(revision => revision.revision === codexBefore.currentRevision));
    assert.equal(codexAfter.revisions.find(revision => revision.revision === codexAfter.currentRevision).launch.command,
      profilesAfter.desktopExecutables.codexCommand);
    assert.equal(profilesAfter.desktopExecutables.codexCommand, String.raw`C:\Fixture Programs\codex.exe`);
    assert.equal(JSON.parse(await readFile(workerDraft, 'utf8')).input, `> ${prompt}\n\n${existingDraft}`);
    if (windowsHtml) {
      async function confirmNormalExit() {
        let timer;
        try {
          const result = await Promise.race([exited, new Promise((_, reject) => {
            timer = setTimeout(() => reject(new Error('Native desktop did not exit after closing its window')), 15_000);
          })]);
          assert.deepEqual(result, [0, null]);
          return child.pid;
        } finally { clearTimeout(timer); }
      }
      const firstPid = await confirmNormalExit();
      assert.deepEqual(canaryRequests, []);
      await rm(htmlPath);
      launch();
      await captureWindow(true);
      const secondPid = await confirmNormalExit();
      assert.notEqual(secondPid, firstPid);
      assert.deepEqual(canaryRequests, []);
      await writeFile(join(dirname(resolve(windowsPreview)), 'windows-html-preview-receipt.json'), JSON.stringify({
        ok: true, windowsWebView2: true, sourceRemovedBeforeColdReopen: true,
        loopbackRequests: canaryRequests, exitedPids: [firstPid, secondPid],
      }, null, 2) + '\n');
    }
  }
  console.log('Native desktop opened the saved project and recovered its missing chat selection from the extracted bundle.');
} finally {
  reader?.close();
  if (child && child.exitCode === null && child.signalCode === null) { child.kill(); await exited; }
  if (canary?.listening) {
    canary.closeAllConnections();
    await new Promise(resolve => canary.close(resolve));
  }
  await rm(directory, { recursive: true, force: true, maxRetries: 10, retryDelay: 250 });
}

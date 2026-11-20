// Run against the extracted bundle's production runtime and isolated data.
import assert from 'node:assert/strict';
import { mkdir, readFile, realpath, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const [root, resources, phase = 'seed'] = process.argv.slice(2);
const state = join(root, 'state');
const project = join(root, 'project');
const info = join(root, 'session-modes-seed.json');
const acpRoot = join(root, 'acp-peer');
const load = module => import(pathToFileURL(join(resources, 'runtime', 'dist', module)).href);
const peer = path => fileURLToPath(new URL('../../packages/runtime/checks/' + path, import.meta.url));
const draftFile = id => join(state, 'desktop-drafts', 'session-' + id + '.json');
const readJson = async path => JSON.parse(await readFile(path, 'utf8'));
const readWire = async path => (await readFile(path, 'utf8')).trim().split('\n').filter(Boolean).map(JSON.parse);
const nativeThread = 'modes-thread';
const historyText = 'Stored Codex mode history 49A';
const draftText = 'Unsent Codex mode draft 49B';
const piDraft = 'Separate Pi draft 49C';
const environment = { APPDATA: join(root, 'appdata'), LOCALAPPDATA: join(root, 'localappdata') };
const { ChatIndex } = await load('chat-index.js');

if (phase === 'seed') {
  await mkdir(project);
  await mkdir(join(root, 'offline-pi'));
  for (const directory of Object.values(environment)) await mkdir(directory);
  Object.assign(process.env, environment);
  await writeFile(join(project, 'modes-control.json'), JSON.stringify({
    modes: [{ mode: 'default' }, { mode: 'plan' }],
    requirements: { allowedSandboxModes: ['read-only', 'workspace-write'] },
    resumeMode: 'default',
    turns: [{
      id: 'stored-turn', status: 'completed',
      items: [{ id: 'stored-message', type: 'agentMessage', text: historyText }],
    }],
    rejectTurn: true,
  }));
  const { createDesktopRuntime } = await load('desktop-runtime.js');
  const runtime = await createDesktopRuntime(state, () => {});
  const sessions = {};
  let workspaceId;
  try {
    const workspace = await runtime.hive.request('POST', '/api/workspaces', {
      path: await realpath(project), name: 'Mode acceptance QA', autostart_orchestrator: false,
    });
    workspaceId = workspace.id;
    for (const [driver, title, args] of [
      ['codex', 'Codex mode QA', [peer('codex-modes-peer.cjs')]],
      ['acp', 'ACP mode QA', [peer('fixtures/acp-runtime-peer.cjs'), acpRoot, 'config-modes']],
      ['pi', 'Pi mode QA', null],
    ]) {
      const options = {};
      if (args) {
        const { profile } = await runtime.profiles.create({
          driver, label: title, launch: { command: process.execPath, args },
        }, (await runtime.profiles.list()).storeVersion);
        options.profile = profile.ref;
      }
      const chat = await runtime.chats.createRecord(workspaceId, driver, options);
      runtime.chats.rename(chat.id, title);
      sessions[driver] = chat.id;
    }
    runtime.hive.saveNavigation(JSON.stringify({ workspace: workspaceId, sessions: { [workspaceId]: sessions.codex } }));
  } finally {
    await runtime.close();
  }
  const index = new ChatIndex(join(state, 'chats'));
  index.save({
    ...index.get(sessions.codex), codexThreadId: nativeThread,
    identityState: 'known', originAgentId: workspaceId + ':orchestrator',
    nativeMode: { backend: 'codex', mode: 'native-default', permissionMode: 'read-only' },
  });
  await mkdir(join(state, 'desktop-drafts'), { recursive: true });
  for (const [id, input] of [[sessions.codex, draftText], [sessions.pi, piDraft]]) {
    await writeFile(draftFile(id), JSON.stringify({ key: 'session:' + id, input, failed: [] }));
  }
  await writeFile(join(state, 'desktop-language.json'), '"en"');
  await writeFile(info, JSON.stringify({
    project, workspaceId, sessions, environment,
  }));
} else {
  assert.ok(['select', 'reopen'].includes(phase));
  const { sessions } = await readJson(info);
  const index = new ChatIndex(join(state, 'chats'));
  const codex = index.get(sessions.codex);
  const acp = index.get(sessions.acp);
  assert.equal(codex.codexThreadId, nativeThread);
  assert.deepEqual(codex.nativeMode, { backend: 'codex', mode: 'native-plan', permissionMode: 'workspace-write' });
  assert.ok(acp.acpSessionId);
  assert.equal(acp.nativeMode, undefined, 'ACP mode state belongs to the peer');
  assert.equal(index.get(sessions.pi).nativeMode, undefined, 'Pi must have no saved modes');
  assert.equal((await readJson(draftFile(sessions.codex))).input, draftText);
  assert.equal((await readJson(draftFile(sessions.pi))).input, piDraft);
  const codexWire = await readWire(join(project, 'modes-wire.jsonl'));
  assert.ok(!codexWire.some(({ method }) => method === 'thread/start' || method.startsWith('turn/')));
  const resumes = codexWire.filter(({ method }) => method === 'thread/resume');
  assert.ok(resumes.length && resumes.every(({ params }) => params.threadId === nativeThread));
  assert.equal(resumes[0].params.sandbox, 'read-only');
  if (phase === 'reopen') {
    assert.equal(resumes.at(-1).params.sandbox, 'workspace-write');
    assert.equal(resumes.at(-1).params.approvalPolicy, 'on-request');
  }
  const acpWire = await readWire(join(acpRoot, 'protocol.jsonl'));
  assert.ok(!acpWire.some(({ method }) => method === 'session/prompt' || method === 'session/set_mode'));
  assert.equal(acpWire.filter(({ method }) => method === 'session/new').length, 1);
  assert.deepEqual(
    acpWire.filter(({ method }) => method === 'session/set_config_option')
      .map(({ sessionId, configId, value }) => [sessionId, configId, value]),
    [[acp.acpSessionId, 'collaboration_mode', 'plan'], [acp.acpSessionId, 'mode', 'write']],
  );
  const acpLoads = acpWire.filter(({ method }) => method === 'session/load');
  assert.ok(acpLoads.every(({ sessionId }) => sessionId === acp.acpSessionId));
  if (phase === 'select') await writeFile(join(root, 'first-resume-count'), String(resumes.length));
  if (phase === 'reopen') {
    assert.ok(resumes.length > Number(await readFile(join(root, 'first-resume-count'), 'utf8')));
    assert.ok(acpLoads.length > 0, 'Cold reconnect did not load the original ACP session');
  }
  await writeFile(join(root, 'session-modes-' + phase + '.json'), JSON.stringify({
    codexThread: nativeThread, codexResumeSandboxes: resumes.map(({ params }) => params.sandbox),
    codexMode: codex.nativeMode, acpSession: acp.acpSessionId, acpLoads: acpLoads.length,
    draftsPreserved: true, noInference: true,
  }, null, 2));
}

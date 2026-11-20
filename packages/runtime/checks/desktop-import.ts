/** Production RPC plus isolated native transcripts. No account or model access. */
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { once } from 'node:events';
import { appendFile, chmod, mkdir, mkdtemp, readFile, realpath, rename, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { createInterface } from 'node:readline';
import { setTimeout as delay } from 'node:timers/promises';
import { fileURLToPath } from 'node:url';
import { ChatIndex } from '../src/chat-index.js';
import { defaultAgentProfileId } from '../src/agent-profiles.js';

const directory = await realpath(await mkdtemp(join(tmpdir(), 'hive-desktop-import-')));
const dataDir = join(directory, 'desktop');
const project = join(directory, 'project');
const cwd = join(project, 'nested');
const folderOnly = join(directory, 'folder-only');
const codexHome = join(directory, 'codex');
const claudeHome = join(directory, 'claude');
const piHome = join(directory, 'pi');
const codexId = randomUUID(), claudeId = randomUUID(), piId = randomUUID(), duplicateId = randomUUID();
const nativeDate = new Date().toISOString();
const projectKey = cwd.replace(/[^a-zA-Z0-9]/g, '-');
const codexPath = (id: string) => join(codexHome, 'sessions', '2026', '11', '20', `rollout-2026-11-20T12-00-00-${id}.jsonl`);
const claudePath = join(claudeHome, 'projects', projectKey, `${claudeId}.jsonl`);
const piPath = join(piHome, 'sessions', 'encoded-project', `2026-11-20T12-00-00-000Z_${piId}.jsonl`);
const codexLaunchLog = join(directory, 'wire.jsonl');
const claudeLaunchLog = join(directory, 'claude-wire.jsonl');
const piLaunchLog = join(directory, 'pi-wire.log');
const claudeControl = join(directory, 'claude-control.json');
const entry = process.env.HIVE_DESKTOP_TEST_RUNTIME ?? fileURLToPath(new URL('../dist/main.js', import.meta.url));
const file = async (path: string, contents: string) => {
  await mkdir(dirname(path), {recursive:true});
  await writeFile(path, contents);
};
const transcript = (path: string, rows: unknown[]) => file(path, rows.map(row => JSON.stringify(row)).join('\n') + '\n');
async function log(path: string) {
  return (await readFile(path, 'utf8').catch((error: NodeJS.ErrnoException) => {
    if (error.code === 'ENOENT') return '';
    throw error;
  })).split('\n').filter(Boolean);
}
function startRuntime(env: NodeJS.ProcessEnv) {
  const child = spawn(process.execPath, [entry], {cwd:directory, stdio:['pipe','pipe','pipe'], env});
  const exited = once(child, 'exit');
  const pending = new Map<number, (response: any) => void>();
  let sequence = 0, diagnostics = '';
  child.stderr.on('data', line => { diagnostics += String(line); });
  const lines = createInterface({input:child.stdout});
  lines.on('line', line => {
    const response = JSON.parse(line);
    const resolve = pending.get(response.id);
    if (resolve) { pending.delete(response.id); resolve(response); }
  });
  child.on('exit', () => {
    for (const resolve of pending.values()) resolve({error:{message:`Runtime stopped: ${diagnostics}`}});
    pending.clear();
  });
  async function rpc(method: string, params: Record<string, unknown> = {}) {
    const id = ++sequence;
    const response = await new Promise<any>((resolve, reject) => {
      const timer = setTimeout(() => { pending.delete(id); reject(new Error(`Timed out: ${method}\n${diagnostics}`)); }, 20_000);
      pending.set(id, result => { clearTimeout(timer); resolve(result); });
      child.stdin.write(JSON.stringify({id, method, params}) + '\n');
    });
    if (response.error) throw new Error(response.error.message);
    return response.result;
  }
  return {rpc, async close() { await rpc('shutdown'); assert.equal((await exited)[0], 0, diagnostics); lines.close(); },
    async terminate() { if (child.exitCode === null && child.signalCode === null) { child.kill(); await exited; } lines.close(); }};
}
async function waitFor(description: string, condition: () => Promise<boolean>) {
  const until = Date.now() + 10_000;
  while (!await condition()) {
    if (Date.now() >= until) throw new Error(`Timed out waiting for ${description}`);
    await delay(25);
  }
}
async function peerCommand(name: string, target: string) {
  const path = join(directory, process.platform === 'win32' ? `${name}.cmd` : name);
  await writeFile(path, process.platform === 'win32'
    ? `@"${process.execPath}" "${target}" %*\r\n`
    : `#!/bin/sh\nexec "${process.execPath}" "${target}" "$@"\n`);
  await chmod(path, 0o700);
  return path;
}

let runtime: ReturnType<typeof startRuntime> | undefined;
try {
  await mkdir(join(project, '.git'), {recursive:true});
  await mkdir(cwd);
  await mkdir(folderOnly);
  const codexRows = (id: string, input: string) => [
    {timestamp:nativeDate, type:'session_meta', payload:{id, cwd, source:'cli'}},
    {timestamp:nativeDate, type:'event_msg', payload:{type:'user_message', message:input}},
    {timestamp:nativeDate, type:'event_msg', payload:{type:'agent_message', message:'Saved Codex reply'}},
  ];
  const codexEvent = (payload: unknown) => ({timestamp:nativeDate, type:'event_msg', payload});
  await transcript(codexPath(codexId), [
    ...codexRows(codexId, 'Existing Codex question'),
    ...Array.from({length:18}, () => ({type:'response_item', payload:{type:'function_call_output', output:'x'.repeat(512 * 1024)}})),
    codexEvent({type:'task_started'}),
    codexEvent({type:'user_message', message:'Discarded legacy question'}),
    codexEvent({type:'user_message', message:'Discarded legacy follow-up'}),
    codexEvent({type:'task_complete'}),
    codexEvent({type:'thread_rolled_back', num_turns:1}),
    codexEvent({type:'task_started', turn_id:'rolled-back'}),
    codexEvent({type:'item_completed', turn_id:'rolled-back', item:{type:'UserMessage', id:'rolled-user', content:[{type:'text', text:'Rolled-back question'}]}}),
    codexEvent({type:'task_complete', turn_id:'rolled-back'}),
    codexEvent({type:'thread_rolled_back', num_turns:1}),
    codexEvent({type:'task_started', turn_id:'recent'}),
    codexEvent({type:'item_completed', turn_id:'recent', item:{type:'UserMessage', id:'recent-user', content:[{type:'text', text:'Recent Codex question'}]}}),
    codexEvent({type:'item_completed', turn_id:'recent', item:{type:'AgentMessage', id:'recent-assistant', content:[{type:'Text', text:'Recent Codex reply past 8 MiB'}]}}),
    codexEvent({type:'task_complete', turn_id:'recent'}),
  ]);
  await transcript(codexPath(duplicateId), codexRows(duplicateId, 'Hive already owns this'));
  await file(join(codexHome, '.codex-global-state.json'), JSON.stringify({'local-projects':{'recent':{rootPaths:[folderOnly, 'relative'], updatedAt:1}}}));
  const user = randomUUID(), assistant = randomUUID();
  await transcript(claudePath, [
    {type:'user', uuid:user, parentUuid:null, isSidechain:false, sessionId:claudeId, cwd, timestamp:nativeDate, message:{role:'user', content:'Existing Claude question'}},
    {type:'assistant', uuid:assistant, parentUuid:user, isSidechain:false, sessionId:claudeId, cwd, timestamp:nativeDate,
      message:{id:'msg_fixture', type:'message', model:'gpt-6.1-terra', role:'assistant', content:[{type:'text', text:'Saved Claude reply'}]}},
  ]);
  await transcript(piPath, [
    {type:'session', version:3, id:piId, timestamp:nativeDate, cwd},
    {type:'message', id:'user0001', parentId:null, timestamp:nativeDate, message:{role:'user', content:'Existing Pi question', timestamp:Date.now()}},
    {type:'message', id:'reply001', parentId:'user0001', timestamp:nativeDate, message:{role:'assistant', content:[{type:'text', text:'Saved Pi reply'}], timestamp:Date.now()}},
  ]);
  await file(join(directory, 'version'), '0.169.0');
  await file(join(directory, `${codexId}.json`), JSON.stringify({id:codexId, path:codexPath(codexId), historyMode:'paginated', turns:[
    {id:'saved-turn', status:'completed', items:[
      {id:'native-user', type:'userMessage', content:[{type:'text', text:'Existing Codex question'}]},
      {id:'native-assistant', type:'agentMessage', text:'Saved Codex reply'},
    ]},
  ]}));
  await file(claudeControl, JSON.stringify({completeQueries:true}));
  const codexPeer = await peerCommand('codex-peer', fileURLToPath(new URL('./fixtures/codex-desktop-view-peer.cjs', import.meta.url)));
  const claudePeer = await peerCommand('claude-peer', fileURLToPath(new URL('./fixtures/claude-modes-peer.cjs', import.meta.url)));
  const piCommand = await peerCommand('pi-peer', fileURLToPath(new URL('./fixtures/pi-import-peer.cjs', import.meta.url)));
  const env = {...process.env, HIVE_DESKTOP_DATA_DIR:dataDir, CODEX_HOME:codexHome, CLAUDE_CONFIG_DIR:claudeHome,
    PI_CODING_AGENT_DIR:piHome, HIVE_CODEX_COMMAND:codexPeer, HIVE_CLAUDE_COMMAND:claudePeer, HIVE_PI_COMMAND:piCommand, PI_OFFLINE:'1',
    HIVE_PI_IMPORT_WIRE:piLaunchLog, HIVE_VIEW_FIXTURE_DIRECTORY:directory, HIVE_CLAUDE_MODES_WIRE:claudeLaunchLog, HIVE_CLAUDE_MODES_CONTROL:claudeControl,
  };
  delete env.CLAUDE_CODE_PROJECT_DIR_NAME;
  const getWorkspaces = () => runtime!.rpc('hive.request', {method:'GET', path:'/api/workspaces'});
  runtime = startRuntime(env);
  await runtime.rpc('initialize');
  const workspace = await runtime.rpc('hive.request', {method:'POST', path:'/api/workspaces', body:{path:project, name:'Existing Hive project', autostart_orchestrator:false}});
  const seeded = await runtime.rpc('chat.createRecord', {workspaceId:workspace.id, backend:'codex'});
  await runtime.close();
  const index = new ChatIndex(join(dataDir,'chats'));
  index.save({...index.get(seeded.id),codexThreadId:duplicateId});
  runtime = startRuntime(env);
  await runtime.rpc('initialize');
  await assert.rejects(runtime.rpc('desktop.import.apply', {projectIds:['f'.repeat(64)]}), /selection expired/);
  const discover = await runtime.rpc('desktop.import.discover');
  assert.equal(discover.limited, false);
  const importProject = discover.projects.find((row: any) => row.path === project);
  const importFolder = discover.projects.find((row: any) => row.path === folderOnly);
  assert.equal(importProject?.workspaceId, workspace.id);
  assert.equal(importProject.sessionCount, 4);
  assert.deepEqual([...importProject.backends].sort(), ['claude','codex','pi-rpc']);
  assert.equal(importFolder?.sessionCount, 0);
  assert.equal((await getWorkspaces()).length, 1);
  assert.equal((await runtime.rpc('chat.list', {workspaceId:workspace.id})).length, 1);

  const unchanged = new Map();
  for (const path of [codexPath(codexId), codexPath(duplicateId), claudePath, piPath]) unchanged.set(path, await readFile(path));
  const apply = (...projectIds: string[]) => runtime!.rpc('desktop.import.apply', {projectIds});
  await rename(piPath, `${piPath}.moved`);
  const first = (await apply(importProject.id)).results[0];
  assert.equal(first.imported, 2);
  assert.equal(first.alreadyImported, 1);
  assert.equal(first.skipped, 1);
  assert.match(first.error, /changed or became unavailable/);
  assert.equal(first.workspace.id, workspace.id);
  assert.equal((await getWorkspaces()).length, 1, 'Unselected folder must stay out of Hive');
  await rename(`${piPath}.moved`, piPath);
  const second = (await apply(importProject.id, importFolder.id)).results;
  assert.equal(second[0].imported, 1);
  assert.equal(second[0].alreadyImported, 3);
  assert.equal(second[0].skipped, 0);
  assert.equal(second[1].workspace.path, folderOnly);
  assert.equal(second[1].imported, 0);
  for (const [path, before] of unchanged) assert.deepEqual(await readFile(path), before);

  const records = await runtime.rpc('chat.list', {workspaceId:workspace.id});
  assert.equal(records.length, 4);
  const codex = records.find((record:any) => record.codexThreadId === codexId);
  const claude = records.find((record:any) => record.nativeclaudeSessionId === claudeId);
  const pi = records.find((record:any) => record.externalPiSessionId === piId);
  for (const record of [codex, claude, pi]) {
    assert.equal(record.profile.id, defaultAgentProfileId(record.backend));
    assert.equal(record.profile.revision, 1);
    assert.equal(record.identityState, 'new');
    assert.equal(record.nativeImport.cwd, cwd);
    assert.equal(record.agentId, undefined);
  }
  for (const [record, label] of [[codex, 'Codex'], [claude, 'Claude'], [pi, 'Pi']]) {
    const saved = await runtime.rpc('chat.open', {sessionId:record.id, historyLimit:50});
    assert.equal(saved.passive, true);
    assert.equal(saved.executionPath, cwd);
    assert.equal(saved.readOnly, false);
    assert.match(JSON.stringify(saved.history), new RegExp(`Saved ${label} reply`));
  }
  const tail = await runtime.rpc('chat.open', {sessionId:codex.id, historyLimit:1});
  assert.deepEqual(tail.historyMessageIds, ['recent-assistant']);
  assert.match(JSON.stringify(tail.history), /Recent Codex reply past 8 MiB/);
  assert.equal(tail.historyIncomplete, true);
  assert.equal(tail.historyPage.totalCount, 4);
  assert.ok(tail.historyPage.snapshot);
  const older = await runtime.rpc('chat.historyPage', {sessionId:codex.id, beforeIndex:tail.historyPage.nextBeforeIndex,
    beforeMessageId:tail.historyPage.nextBeforeMessageId, snapshot:tail.historyPage.snapshot, limit:3});
  assert.match(JSON.stringify(older.history), /Existing Codex question/);
  assert.doesNotMatch(JSON.stringify(older.history), /Rolled-back question/);
  const anchor = await runtime.rpc('chat.open', {sessionId:codex.id, historyLimit:1, historyAroundMessageId:older.historyMessageIds[0]});
  assert.match(JSON.stringify(anchor.history), /Existing Codex question/);
  await appendFile(codexPath(codexId), JSON.stringify(codexEvent({type:'agent_message', message:'Native file changed'})) + '\n');
  await assert.rejects(runtime.rpc('chat.historyPage', {sessionId:codex.id, beforeIndex:tail.historyPage.nextBeforeIndex,
    beforeMessageId:tail.historyPage.nextBeforeMessageId, snapshot:tail.historyPage.snapshot, limit:3}), /Saved history changed/);
  await writeFile(codexPath(codexId), unchanged.get(codexPath(codexId)));
  for (const path of [codexLaunchLog, claudeLaunchLog, piLaunchLog]) assert.deepEqual(await log(path), []);
  for (const [path, before] of unchanged) assert.deepEqual(await readFile(path), before);
  await runtime.close();
  runtime = startRuntime(env);
  await runtime.rpc('initialize');
  const cold = await runtime.rpc('desktop.import.discover');
  const retried = (await apply(cold.projects.find((row:any) => row.path === project).id)).results[0];
  assert.equal(retried.imported, 0);
  assert.equal(retried.alreadyImported, 4);
  const profiles = await runtime.rpc('agent.profiles.list');
  const futurePiConfigDir = join(directory, 'future-pi');
  await mkdir(futurePiConfigDir);
  await runtime.rpc('agent.profiles.update', {id:pi.profile.id, expectedStoreVersion:profiles.storeVersion,
    launch:{command:piCommand, args:[], configDir:futurePiConfigDir}});
  const savedPi = await runtime.rpc('chat.open', {sessionId:pi.id, viewRevision:1, historyLimit:1});
  assert.equal(savedPi.savedHistoryPassive, true);
  assert.deepEqual(await log(piLaunchLog), []);
  const connectedPi = await runtime.rpc('chat.open', {sessionId:pi.id, viewRevision:2, historyLimit:1, historyAroundMessageId:'reply001', preferSavedHistory:false});
  assert.equal(connectedPi.savedHistoryPassive, undefined);
  assert.match(JSON.stringify(connectedPi.history), /Saved Pi reply/);
  assert.deepEqual((await log(piLaunchLog)).map(line => JSON.parse(line)), [{event:'start', sessionId:piId, sessionFile:piPath, cwd}]);
  const savedCodex = await runtime.rpc('chat.open', {sessionId:codex.id, viewRevision:3, historyLimit:1});
  assert.equal(savedCodex.savedHistoryPassive, true);
  assert.deepEqual(await log(codexLaunchLog), []);
  const connectedCodex = await runtime.rpc('chat.open', {sessionId:codex.id, viewRevision:4, historyLimit:1, preferSavedHistory:false});
  assert.equal(connectedCodex.savedHistoryPassive, undefined);
  const codexConnection = (await log(codexLaunchLog)).map(line => JSON.parse(line));
  assert.equal(codexConnection.find(row => row.method === 'thread/resume')?.deferGoalContinuation, true);
  assert.equal(codexConnection.some(row => row.method === 'turn/start'), false);
  const explicit = 'Explicit continuation through the same imported session';
  await runtime.rpc('chat.prompt', {sessionId:codex.id, text:explicit});
  await waitFor('Codex continuation', async () => (await log(codexLaunchLog)).some(line => line.includes(explicit)));
  const codexWire = (await log(codexLaunchLog)).map(line => JSON.parse(line));
  assert.deepEqual(codexWire.filter(row => row.method === 'thread/resume').map(row => row.threadId), [codexId]);
  assert.equal(codexWire.filter(row => row.method === 'turn/start').length, 1);
  await waitFor('Codex completion', async () => !(await runtime!.rpc('chat.open', {sessionId:codex.id})).busy);
  await runtime.rpc('chat.prompt', {sessionId:claude.id, text:explicit});
  await waitFor('Claude continuation', async () => (await log(claudeLaunchLog)).some(line => line.includes(explicit)));
  const claudeWire = (await log(claudeLaunchLog)).map(line => JSON.parse(line));
  assert.equal(claudeWire.find(row => row.event === 'start')?.sid, claudeId);
  assert.equal(claudeWire.filter(row => row.event === 'query').length, 1);
  await waitFor('Claude completion', async () => !(await runtime!.rpc('chat.open', {sessionId:claude.id})).busy);
  await runtime.rpc('chat.prompt', {sessionId:pi.id, text:explicit});
  await waitFor('Pi continuation', async () => (await log(piLaunchLog)).some(line => line.includes(explicit)));
  const piWire = (await log(piLaunchLog)).map(line => JSON.parse(line));
  assert.deepEqual(piWire.find(row => row.event === 'start'), {event:'start', sessionId:piId, sessionFile:piPath, cwd});
  assert.deepEqual(piWire.filter(row => row.event === 'query'), [{event:'query', text:explicit}]);
  await waitFor('Pi completion', async () => {
    const state = await runtime!.rpc('chat.open', {sessionId:pi.id});
    return !state.busy && !state.pending && JSON.stringify(state.history).includes('Controlled Pi continued');
  });
  const continuedPi = await readFile(piPath);
  assert.deepEqual(continuedPi.subarray(0, unchanged.get(piPath).length), unchanged.get(piPath));
  await runtime.close();
  runtime = undefined;
  console.log('Native batch import passed over the desktop wire. Codex, Claude and Pi continued the same native session using local protocol peers.');
} finally {
  await runtime?.terminate();
  await rm(directory, {recursive:true, force:true});
}

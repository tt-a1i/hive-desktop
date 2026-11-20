/** Actual Chats.open + managed Git worktrees. Pi uses its real SDK/SessionManager;
 * external transports and the official Claude SDK launch explicitly controlled
 * subprocess peers. The single Codex turn is synthetic materialization only. */
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { mkdir, mkdtemp, readFile, realpath, rename, rm, writeFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import { once } from 'node:events';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createDesktopRuntime } from '../src/desktop-runtime.js';
import type { ChatBackend, ChatRecord } from '../src/chat-index.js';
import type { Chats } from '../src/chats.js';
import type { PiSessionHandle } from '../src/agents/types.js';

assert.ok(!process.argv.includes('--live'), 'This check must never run native vendor CLIs or models');
const directory = await realpath(await mkdtemp(join(tmpdir(), 'hive-backend-worktree-cwd-')));
const repo = join(directory, 'repo'), project = join(repo, 'sub project'), data = join(directory, 'state');
const keys = ['PI_CODING_AGENT_DIR', 'PI_OFFLINE', 'CLAUDE_CONFIG_DIR', 'HIVE_CWD_FIXTURE_WIRE', 'HIVE_CWD_FIXTURE_MODEL_UNAVAILABLE', 'HIVE_CWD_FIXTURE_MODEL_MISSING', 'HIVE_CWD_FIXTURE_MODE_MISSING', 'HIVE_CWD_FIXTURE_MODE_MISMATCH', 'HIVE_CWD_FIXTURE_EFFORT_MISSING', 'HIVE_CWD_FIXTURE_EFFORT_MISMATCH', 'GIT_CONFIG_GLOBAL', 'GIT_CONFIG_NOSYSTEM'] as const;
const previous = Object.fromEntries(keys.map(key => [key, process.env[key]]));
let runtime: Awaited<ReturnType<typeof createDesktopRuntime>> | undefined;
let requests = 0;
const server = createServer((_request, response) => { requests++; response.writeHead(500).end('Unexpected model request'); });
const live = (chats: Chats) => (chats as unknown as {live: Map<string, {handle: PiSessionHandle}>}).live;
const records = new Map<ChatBackend, {record: ChatRecord; root: string; nativeId: string; historyIds: string[]; file?: string}>();
type Wire = {backend: ChatBackend; cwd: string; event: string; args?: string[]; method?: string; params?: Record<string, unknown>; type?: string; action?: string; shouldQuery?: boolean; effectiveModel?: string; effectiveMode?: string; effectiveEffort?: string};
try {
  await mkdir(project, {recursive: true}); await mkdir(join(directory, 'pi')); await writeFile(join(directory, 'gitconfig'), '');
  process.env.GIT_CONFIG_GLOBAL = join(directory, 'gitconfig'); process.env.GIT_CONFIG_NOSYSTEM = '1';
  process.env.PI_CODING_AGENT_DIR = join(directory, 'pi'); process.env.PI_OFFLINE = '1';
  process.env.CLAUDE_CONFIG_DIR = join(directory, 'claude'); process.env.HIVE_CWD_FIXTURE_WIRE = join(directory, 'wire.jsonl');
  server.listen(0, '127.0.0.1'); await once(server, 'listening');
  const address = server.address(); assert.ok(address && typeof address !== 'string');
  await writeFile(join(directory, 'pi', 'models.json'), JSON.stringify({providers: {openai: {api: 'openai-responses', baseUrl: `http://127.0.0.1:${address.port}/v1`, apiKey: 'fixture-only', models: ['fixture-cwd', 'fixture-inherited'].map(id => ({id, name: id, input: ['text']}))}}}));
  await writeFile(join(directory, 'pi', 'settings.json'), JSON.stringify({defaultProvider: 'openai', defaultModel: 'fixture-cwd'}));
  const git = (...args: string[]) => execFileSync('git', ['-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid', '-c', 'commit.gpgSign=false', ...args], {cwd: repo, encoding: 'utf8', stdio: 'pipe'});
  git('init', '--template='); await writeFile(join(project, 'file.txt'), 'committed project file'); git('add', '.'); git('commit', '-qm', 'base');
  const peer = join(directory, 'controlled-peer.cjs');
  await writeFile(peer, await readFile(new URL('./fixtures/worktree-backend-cwd-peer.cjs', import.meta.url)), {mode: 0o700});
  const options = {executableEnvironment: {HIVE_CODEX_COMMAND: peer, HIVE_PI_COMMAND: peer, HIVE_CLAUDE_COMMAND: peer}};
  runtime = await createDesktopRuntime(data, () => {}, options);
  const workspace = await runtime.hive.request('POST', '/api/workspaces', {path: project, name: 'Backend cwd fixture', autostart_orchestrator: false}) as {id: string};
  const backends: ChatBackend[] = ['pi', 'codex', 'pi-rpc', 'claude'];
  for (const backend of backends) {
    const record = await runtime.chats.createRecord(workspace.id, backend, {isolation: 'worktree'});
    runtime.chats.rename(record.id, `cwd-${backend}-title`);
    const execution = await runtime.chats.sessionExecution(record.id);
    assert.equal(execution.kind, 'worktree'); assert.notEqual(execution.path, project); assert.ok(execution.path.endsWith('/sub project'));
    const opened = await runtime.chats.open(record.id);
    assert.equal(opened.executionPath, execution.path); assert.equal(opened.execution?.path, execution.path); assert.equal(opened.workspaceId, workspace.id);
    const handle: PiSessionHandle = live(runtime.chats).get(record.id)!.handle;
    assert.equal(handle.hasContext('hive-desktop-execution'), true);
    if (backend === 'codex') {
      // Production materialization callback must save the native ID. No injected
      // ChatIndex native IDs or mocked createCodexSession are used here.
      await handle.prompt('Controlled protocol fixture: materialize this local thread without a model');
      assert.ok(runtime.chats.list(workspace.id).find(item => item.id === record.id)!.codexThreadId);
    }
    const current: ChatRecord = runtime.chats.list(workspace.id).find(item => item.id === record.id)!;
    const nativeId: string | undefined = backend === 'pi' ? current.piSessionId : backend === 'pi-rpc' ? current.externalPiSessionId : backend === 'codex' ? current.codexThreadId : current.nativeclaudeSessionId;
    assert.ok(nativeId); assert.equal(nativeId, handle.sessionId);
    if (backend === 'pi' || backend === 'pi-rpc') {
      assert.ok(current.sessionFile);
      const header = JSON.parse((await readFile(current.sessionFile, 'utf8')).split('\n')[0]!);
      assert.equal(header.cwd, execution.path); assert.equal(header.id, nativeId);
    }
    if (backend === 'claude') {
      const transcript = join(process.env.CLAUDE_CONFIG_DIR!, 'projects', execution.path.replace(/[^a-zA-Z0-9]/g, '-'), nativeId + '.jsonl');
      const rows = (await readFile(transcript, 'utf8')).trim().split('\n').map(line => JSON.parse(line));
      assert.ok(rows.length > 0); assert.ok(rows.every(row => row.cwd === execution.path && row.shouldQuery === false));
      assert.ok(rows.some(row => row.message.content.includes(project) && row.message.content.includes(execution.path)));
    }
    records.set(backend, {record: current, root: execution.path, nativeId, historyIds: [...handle.snapshot().historyMessageIds], ...(current.sessionFile && {file: current.sessionFile})});
  }
  const source = await runtime.hive.request('GET', '/api/workspaces') as {id: string; path: string}[];
  assert.equal(source.find(item => item.id === workspace.id)!.path, project);
  assert.equal(source.length, 1, 'Managed worktrees never create additional Hive workspaces');
  await runtime.close(); runtime = await createDesktopRuntime(data, () => {}, options);
  for (const backend of backends) {
    const saved = records.get(backend)!;
    const reopened = await runtime.chats.open(saved.record.id);
    assert.equal(reopened.executionPath, saved.root); assert.equal(reopened.execution?.path, saved.root); assert.equal(reopened.workspaceId, workspace.id);
    const handle: PiSessionHandle = live(runtime.chats).get(saved.record.id)!.handle;
    assert.equal(handle.sessionId, saved.nativeId); assert.equal(handle.hasContext('hive-desktop-execution'), true);
    assert.deepEqual(handle.snapshot().historyMessageIds.slice(0, saved.historyIds.length), saved.historyIds, `${backend} restores the original native history rather than only reusing an ID`);
    if (saved.file) assert.equal(reopened.sessionFile, saved.file);
    if (backend === 'pi') assert.equal(reopened.piSessionId, saved.nativeId);
    if (backend === 'codex') assert.equal(reopened.codexThreadId, saved.nativeId);
    if (backend === 'pi-rpc') assert.equal(reopened.externalPiSessionId, saved.nativeId);
    if (backend === 'claude') assert.equal(reopened.nativeclaudeSessionId, saved.nativeId);
  }
  const wire = (await readFile(process.env.HIVE_CWD_FIXTURE_WIRE!, 'utf8')).trim().split('\n').map(line => JSON.parse(line) as Wire);
  const argument = (args: string[], name: string) => args.find(value => value.startsWith(name + '='))?.slice(name.length + 1) ?? (args.includes(name) ? args[args.indexOf(name) + 1] : undefined);
  assert.equal(wire.some(row => row.event === 'fixture-error'), false);
  for (const backend of ['codex', 'pi-rpc', 'claude'] as const) {
    const saved = records.get(backend)!;
    const starts = wire.filter(row => row.backend === backend && row.event === 'start');
    assert.equal(starts.length, 2, `${backend} must launch once in each runtime`);
    assert.ok(starts.every(row => row.cwd === saved.root && row.cwd !== project));
    if (backend === 'pi-rpc') {
      assert.equal(starts[0]!.args!.includes('--session'), false);
      const at = starts[1]!.args!.indexOf('--session'); assert.ok(at >= 0); assert.equal(starts[1]!.args![at + 1], saved.file);
    }
    if (backend === 'claude') {
      assert.equal(argument(starts[0]!.args!, '--session-id'), saved.nativeId); assert.equal(argument(starts[1]!.args!, '--resume'), saved.nativeId);
      assert.equal(wire.some(row => row.backend === backend && row.type === 'user' && row.shouldQuery !== false), false);
    }
    if (backend === 'codex') {
      assert.equal(wire.find(row => row.backend === backend && row.method === 'thread/start')!.params!.cwd, saved.root);
      assert.equal(wire.find(row => row.backend === backend && row.method === 'thread/resume')!.params!.threadId, saved.nativeId);
      assert.equal(wire.filter(row => row.backend === backend && row.method === 'turn/start').length, 1, 'Exactly one synthetic fixture turn materializes the ID');
    }
  }
  // A missing checkout restricts execution, while the saved transcript and
  // other title hits remain available without relaunching any backend.
  await runtime.close(); runtime = await createDesktopRuntime(data, () => {}, options);
  const missing = records.get('pi')!; await rename(missing.root, missing.root + '.missing');
  try {
    const result = await runtime.chats.search('cwd-', false, 0, workspace.id);
    assert.ok(!result.incompleteHistory.some(item => item.sessionId === missing.record.id));
    assert.ok(result.matches.some(item => item.session.id === records.get('codex')!.record.id));
    const snapshot = await runtime.chats.open(missing.record.id);
    assert.ok('readOnly' in snapshot && snapshot.readOnly);
    assert.equal(snapshot.executionPath, missing.root);
    await assert.rejects(runtime.chats.sessionExecution(missing.record.id), /worktree|工作树|ENOENT/i);
  } finally { await rename(missing.root + '.missing', missing.root); }
  assert.equal((await readFile(process.env.HIVE_CWD_FIXTURE_WIRE!, 'utf8')).trim().split('\n').filter(line => JSON.parse(line).event === 'start').length,
    wire.filter(row => row.event === 'start').length, 'Search and read-only open never launch another child');
  const after = await runtime.hive.request('GET', '/api/workspaces') as {id: string; path: string}[];
  assert.equal(after.length, 1); assert.equal(after[0]!.path, project); assert.equal(requests, 0);

  // Use the real native model picker operation on both sources, then exercise
  // inherited first turns with a controlled app-server after a cold reopen.
  const rootSource = await runtime.chats.createRecord(workspace.id, 'codex');
  const isolatedSource = records.get('codex')!.record;
  const chosen = 'fixture-inherited';
  await runtime.chats.setModel(rootSource.id, 'openai', chosen);
  await runtime.chats.setModel(isolatedSource.id, 'openai', chosen);
  await runtime.chats.mode(rootSource.id, 'native-plan');
  await runtime.chats.mode(isolatedSource.id, 'native-default');
  await runtime.chats.mode(rootSource.id, 'read-only', true);
  await runtime.chats.mode(isolatedSource.id, 'workspace-write', true);
  const rootOptions = {profile: rootSource.profile, deferFirstConnection: true, sourceSessionId: rootSource.id};
  const neutralDraft = await runtime.chats.createRecord(workspace.id, 'codex', rootOptions);
  assert.equal(neutralDraft.inheritedDraftReasoning, undefined);
  await runtime.chats.reasoning(rootSource.id, 'high');
  await runtime.chats.reasoning(isolatedSource.id, 'low');
  await assert.rejects(runtime.chats.createRecord(workspace.id, 'codex', {...rootOptions, deferFirstConnection: false}), {code: 'FRESH_DRAFT_SOURCE_INVALID'});
  const rootDraft = await runtime.chats.createRecord(workspace.id, 'codex', rootOptions);
  const staleDraft = await runtime.chats.createRecord(workspace.id, 'codex', rootOptions);
  const missingModelDraft = await runtime.chats.createRecord(workspace.id, 'codex', rootOptions);
  const missingModeDraft = await runtime.chats.createRecord(workspace.id, 'codex', rootOptions);
  const unconfirmedModeDraft = await runtime.chats.createRecord(workspace.id, 'codex', rootOptions);
  const missingEffortDraft = await runtime.chats.createRecord(workspace.id, 'codex', rootOptions);
  const unconfirmedEffortDraft = await runtime.chats.createRecord(workspace.id, 'codex', rootOptions);
  const worktreeRequestId = randomUUID();
  const worktreeDraft = await runtime.chats.createFreshWorktreeDraftRecord(workspace.id, isolatedSource.id, worktreeRequestId);
  const inheritedControlDrafts = [];
  for (const backend of ['pi', 'pi-rpc', 'claude'] as const) {
    const source = records.get(backend)!.record;
    await runtime.chats.setModel(source.id, backend === 'claude' ? 'claude' : 'openai', chosen);
    if (backend === 'claude') {
      await runtime.chats.mode(source.id, 'plan');
      await runtime.chats.reasoning(source.id, 'high');
    }
    inheritedControlDrafts.push(await runtime.chats.createFreshWorktreeDraftRecord(workspace.id, source.id, randomUUID()));
  }
  const getWire = async () => (await readFile(process.env.HIVE_CWD_FIXTURE_WIRE!, 'utf8')).trim().split('\n').map(line => JSON.parse(line) as Wire);
  const interruptedRequestId = randomUUID();
  const indexFile = join(data, 'chats', 'index.json');
  await rename(indexFile, indexFile + '.retained');
  await mkdir(indexFile);
  try {
    await assert.rejects(runtime.chats.createFreshWorktreeDraftRecord(workspace.id, isolatedSource.id, interruptedRequestId), /工作树已保留/);
  } finally {
    await rm(indexFile, {recursive: true});
    await rename(indexFile + '.retained', indexFile);
  }
  const wireBeforeBrowse = (await getWire()).length;
  await runtime.close(); runtime = await createDesktopRuntime(data, () => {}, options);
  assert.deepEqual(await runtime.chats.createFreshWorktreeDraftRecord(workspace.id, isolatedSource.id, worktreeRequestId), worktreeDraft);
  const recoveredWorktree = await runtime.chats.createFreshWorktreeDraftRecord(workspace.id, isolatedSource.id, interruptedRequestId);
  assert.equal(recoveredWorktree.id, interruptedRequestId);
  assert.equal(recoveredWorktree.inheritedDraftReasoning, 'low');
  assert.equal(recoveredWorktree.inheritedDraftWorkMode, 'native-default');
  assert.equal(recoveredWorktree.model?.id, chosen);
  for (const draft of [rootDraft, worktreeDraft, staleDraft, ...inheritedControlDrafts]) {
    const passive = await runtime.chats.open(draft.id);
    assert.ok('passive' in passive && passive.passive);
    assert.equal(passive.model?.id, chosen);
    assert.equal(passive.inheritedDraftModel, true);
    assert.equal(passive.inheritedDraftWorkMode, draft.id === worktreeDraft.id ? 'native-default' : draft.backend === 'codex' ? 'native-plan' : undefined);
    assert.equal(passive.inheritedDraftReasoning, draft.id === worktreeDraft.id ? 'low' : (draft.backend === 'codex' || draft.backend === 'claude') ? 'high' : undefined);
    assert.equal(passive.nativeMode?.permissionMode, undefined);
    if (draft.backend === 'claude') assert.equal(passive.nativeMode, undefined, 'Claude plan is a permission mode');
    assert.equal(passive.codexThreadId, undefined);
  }
  assert.equal((await getWire()).length, wireBeforeBrowse, 'passive cold opens send no native requests');
  for (const draft of inheritedControlDrafts) {
    const connected = await runtime.chats.open(draft.id, true, true, {limit: 5}, 'connect');
    assert.equal(connected.model?.id, chosen);
    assert.equal(connected.inheritedDraftModel, true);
    if (draft.backend === 'claude') assert.equal((await runtime.chats.reasoning(draft.id)).level, 'high');
  }
  const send = async (id: string, text: string, cwd: string, model: string, mode?: string, effort = 'medium') => {
    const before = (await getWire()).length;
    const accepted = await runtime!.chats.prompt(id, text);
    assert.equal(accepted.session.model?.id, model);
    assert.equal(accepted.session.inheritedDraftModel, undefined);
    assert.equal(accepted.session.inheritedDraftReasoning, undefined);
    const deadline = Date.now() + 8_000;
    while (Date.now() < deadline) {
      const rows = (await getWire()).slice(before);
      const turn = rows.find(row => row.method === 'turn/start' && JSON.stringify(row.params?.input).includes(text));
      if (turn && !(await runtime!.chats.open(id)).busy) {
        assert.equal(turn.effectiveModel, model);
        assert.equal(turn.cwd, cwd);
        assert.equal(turn.effectiveMode, mode);
        assert.equal(turn.effectiveEffort, effort);
        assert.equal(Reflect.get((turn.params?.collaborationMode ?? {}) as object, 'mode'), mode);
        if (mode) {
          const settings = Reflect.get(turn.params?.collaborationMode as object, 'settings');
          assert.equal(settings.reasoning_effort, effort);
        }
        for (const row of rows.filter(row => ['thread/start', 'thread/settings/update', 'turn/start'].includes(row.method ?? ''))) {
          assert.equal(row.params?.approvalPolicy, undefined);
          assert.equal(row.params?.sandbox, undefined);
          assert.equal(row.params?.sandboxPolicy, undefined);
        }
        if (mode) assert.ok(rows.slice(0, rows.indexOf(turn)).some(row => row.method === 'thread/settings/updated'
          && row.params?.threadId === turn.params?.threadId
          && Reflect.get((Reflect.get((row.params?.threadSettings ?? {}) as object, 'collaborationMode') ?? {}) as object, 'mode') === mode));
        return;
      }
      await new Promise(resolve => setTimeout(resolve, 25));
    }
    throw new Error('Controlled first turn did not settle');
  };
  await send(rootDraft.id, 'first root draft input', project, chosen, 'plan', 'high');
  await send(worktreeDraft.id, 'first isolated draft input', worktreeDraft.executionPath, chosen, 'default', 'low');
  for (const [key, draft] of [['HIVE_CWD_FIXTURE_EFFORT_MISSING', missingEffortDraft], ['HIVE_CWD_FIXTURE_EFFORT_MISMATCH', unconfirmedEffortDraft]] as const) {
    process.env[key] = '1';
    const before = (await getWire()).length;
    await assert.rejects(runtime.chats.prompt(draft.id, 'do not send with unconfirmed effort'), {code: 'FRESH_DRAFT_REASONING_UNAVAILABLE'});
    const rows = (await getWire()).slice(before);
    assert.ok(rows.some(row => row.method === 'thread/read'));
    assert.equal(rows.some(row => row.method === 'thread/settings/update' && row.params?.effort === 'high'), draft.id === unconfirmedEffortDraft.id);
    assert.ok(rows.every(row => row.method !== 'turn/start'));
    delete process.env[key];
  }

  process.env.HIVE_CWD_FIXTURE_MODE_MISSING = '1';
  const beforeMissingMode = (await getWire()).length;
  await assert.rejects(runtime.chats.prompt(missingModeDraft.id, 'do not send with unavailable mode'), {code: 'FRESH_DRAFT_WORK_MODE_UNAVAILABLE'});
  const missingMode = (await getWire()).slice(beforeMissingMode);
  assert.ok(missingMode.some(row => row.method === 'collaborationMode/list'));
  assert.ok(missingMode.every(row => !['thread/start', 'turn/start'].includes(row.method ?? '')));
  delete process.env.HIVE_CWD_FIXTURE_MODE_MISSING;
  process.env.HIVE_CWD_FIXTURE_MODE_MISMATCH = '1';
  const beforeMismatchedMode = (await getWire()).length;
  await assert.rejects(runtime.chats.prompt(unconfirmedModeDraft.id, 'do not send with unconfirmed mode'), {code: 'FRESH_DRAFT_WORK_MODE_UNAVAILABLE'});
  const mismatched = (await getWire()).slice(beforeMismatchedMode);
  assert.ok(mismatched.some(row => row.method === 'thread/settings/updated'));
  assert.ok(mismatched.every(row => row.method !== 'turn/start'));
  delete process.env.HIVE_CWD_FIXTURE_MODE_MISMATCH;

  // The peer still echoes an explicitly requested model from thread/start.
  // A complete catalog without that model must stop the draft before launch.
  process.env.HIVE_CWD_FIXTURE_MODEL_MISSING = '1';
  const beforeMissingModel = (await getWire()).length;
  await assert.rejects(runtime.chats.prompt(missingModelDraft.id, 'do not send without catalog approval'),
    {code: 'FRESH_DRAFT_MODEL_UNAVAILABLE'});
  const missingRequests = (await getWire()).slice(beforeMissingModel);
  assert.ok(missingRequests.some(row => row.method === 'model/list'));
  assert.ok(missingRequests.every(row => row.method !== 'thread/start' && row.method !== 'turn/start'));
  delete process.env.HIVE_CWD_FIXTURE_MODEL_MISSING;

  process.env.HIVE_CWD_FIXTURE_MODEL_UNAVAILABLE = '1';
  await assert.rejects(runtime.chats.prompt(staleDraft.id, 'must wait for model repair'), {code: 'FRESH_DRAFT_MODEL_UNAVAILABLE'});
  await runtime.close(); runtime = await createDesktopRuntime(data, () => {}, options);
  const unavailable = await runtime.chats.open(staleDraft.id);
  assert.ok('unavailableModel' in unavailable && unavailable.unavailableModel?.id === chosen);
  assert.equal(unavailable.agentId, undefined);
  assert.equal((await runtime.chats.open(unconfirmedModeDraft.id)).unavailableWorkMode, 'native-plan');
  await assert.rejects(runtime.chats.resetFreshDraftWorkMode(workspace.id, unconfirmedModeDraft.id, 'native-default'), {code: 'FRESH_DRAFT_WORK_MODE_CHANGED'});
  const beforeWorkModeReset = (await getWire()).length;
  const resetMode = await runtime.chats.resetFreshDraftWorkMode(workspace.id, unconfirmedModeDraft.id, 'native-plan');
  assert.equal(resetMode.inheritedDraftWorkMode, undefined);
  assert.equal(resetMode.inheritedDraftModel, true);
  assert.equal(resetMode.model?.id, chosen);
  assert.equal(resetMode.inheritedDraftReasoning, 'high');
  assert.equal((await getWire()).length, beforeWorkModeReset, 'work-mode reset stays passive');
  const failedConnect = await runtime.chats.open(staleDraft.id, true, true, {limit: 5}, 'connect');
  assert.ok('passive' in failedConnect && failedConnect.passive && !failedConnect.readOnly);
  assert.ok('identityError' in failedConnect && failedConnect.identityError?.code === 'FRESH_DRAFT_MODEL_UNAVAILABLE');
  const claudeDraft = inheritedControlDrafts.find(draft => draft.backend === 'claude')!;
  const nativeBeforeReset = runtime.chats.list(workspace.id).find(record => record.id === claudeDraft.id)?.nativeclaudeSessionId;
  assert.ok(nativeBeforeReset, 'Claude picker materialized an unbound native conversation before the restart');
  const staleClaude = await runtime.chats.open(claudeDraft.id, true, true, {limit: 5}, 'connect');
  assert.ok('unavailableModel' in staleClaude && staleClaude.unavailableModel?.id === chosen);
  await runtime.chats.resetFreshDraftModel(workspace.id, claudeDraft.id, 'claude', chosen);
  const resetClaude = await runtime.chats.open(claudeDraft.id);
  assert.ok('passive' in resetClaude && resetClaude.passive);
  assert.equal(resetClaude.nativeclaudeSessionId, undefined);
  assert.equal(resetClaude.model, undefined);
  assert.ok(!(await getWire()).some(row => row.method === 'turn/start' && JSON.stringify(row.params?.input).includes('must wait for model repair')));
  await assert.rejects(runtime.chats.resetFreshDraftModel(workspace.id, staleDraft.id, 'openai', 'wrong'), {code: 'FRESH_DRAFT_MODEL_CHANGED'});
  const wireBeforeReset = (await getWire()).length;
  const reset = await runtime.chats.resetFreshDraftModel(workspace.id, staleDraft.id, 'openai', chosen);
  assert.equal(reset.model, undefined);
  assert.equal(reset.inheritedDraftModel, undefined);
  assert.equal(reset.inheritedDraftWorkMode, 'native-plan');
  assert.equal(reset.inheritedDraftReasoning, undefined, 'effort was bound to the reset model');
  assert.equal((await getWire()).length, wireBeforeReset, 'explicit reset needs no running provider');
  await send(staleDraft.id, 'send after explicit model reset', project, 'fixture-cwd', 'plan');
  delete process.env.HIVE_CWD_FIXTURE_MODEL_UNAVAILABLE;
  await send(unconfirmedModeDraft.id, 'send after explicit work mode reset', project, chosen, undefined, 'high');
  assert.equal((await runtime.chats.open(unconfirmedEffortDraft.id)).unavailableReasoning, 'high');
  await assert.rejects(runtime.chats.resetFreshDraftReasoning(workspace.id, unconfirmedEffortDraft.id, 'low'), {code:'FRESH_DRAFT_REASONING_CHANGED'});
  const beforeEffortReset = (await getWire()).length;
  const resetEffort = await runtime.chats.resetFreshDraftReasoning(workspace.id, unconfirmedEffortDraft.id, 'high');
  assert.equal(resetEffort.inheritedDraftReasoning, undefined);
  assert.equal(resetEffort.inheritedDraftWorkMode, 'native-plan');
  assert.equal(resetEffort.model?.id, chosen);
  assert.equal((await getWire()).length, beforeEffortReset, 'reasoning reset stays passive');
  await send(unconfirmedEffortDraft.id, 'send after explicit reasoning reset', project, chosen, 'plan');
  const claudeEffortDraft = await runtime.chats.createFreshWorktreeDraftRecord(workspace.id, records.get('claude')!.record.id, randomUUID());
  process.env.HIVE_CWD_FIXTURE_EFFORT_MISMATCH = '1';
  const failedClaudeEffort = await runtime.chats.open(claudeEffortDraft.id, true, true, {limit: 5}, 'connect');
  assert.ok('passive' in failedClaudeEffort && failedClaudeEffort.passive);
  assert.equal(failedClaudeEffort.unavailableReasoning, 'high');
  assert.equal(failedClaudeEffort.model?.id, chosen);
  delete process.env.HIVE_CWD_FIXTURE_EFFORT_MISMATCH;
  assert.equal(requests, 0);
  console.log('FRESH_DRAFT_MODEL_OK: project and worktree first turns use the persisted selected model after restart; unavailable selection prevents a turn until explicit reset.');
  console.log('FRESH_DRAFT_WORK_MODE_OK: root/worktree modes confirmed before their first turns; approvals excluded, failures retain passive drafts, model/work-mode resets are independent.');
  console.log('FRESH_DRAFT_REASONING_OK: root/worktree first turns use independently verified effort after restart; unsupported and clamped effort prevent admission until explicit reset.');
  console.log('WORKTREE_BACKEND_CWD_OK: four actual Chats.open adapters, real Pi SDK header, controlled Codex/Pi-RPC child cwd+restore args and official Claude SDK transcript; cold native identity/path retained, one unchanged Hive workspace, read-only open of missing checkout. Synthetic protocol only; zero model requests.');
} finally {
  await runtime?.close();
  if (server.listening) { server.close(); await once(server, 'close'); }
  for (const key of keys) { if (previous[key] === undefined) delete process.env[key]; else process.env[key] = previous[key]; }
  await rm(directory, {recursive: true, force: true, maxRetries: 5});
}

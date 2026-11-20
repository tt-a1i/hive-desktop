/** Production JSONL, Hive HTTP/SQLite and a controlled Codex app-server. */
import assert from 'node:assert/strict';
import { execFileSync, spawn } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { once } from 'node:events';
import { chmod, mkdir, mkdtemp, readFile, realpath, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createInterface } from 'node:readline';
import { setTimeout as delay } from 'node:timers/promises';
import { fileURLToPath } from 'node:url';
import { ChatIndex } from '../src/chat-index.js';

const directory = await realpath(await mkdtemp(join(tmpdir(), 'hive-codex-desktop-view-')));
const project = join(directory, 'project');
const data = join(directory, 'state');
const agent = join(directory, 'agent');
await mkdir(project);
await mkdir(agent);
await writeFile(join(directory, 'version'), '0.169.0');
const peer = fileURLToPath(new URL('./fixtures/codex-desktop-view-peer.cjs', import.meta.url));
const command = join(directory, process.platform === 'win32' ? 'peer.cmd' : 'peer');
const quote = (value: string) => `'${value.replaceAll("'", "'\\''")}'`;
await writeFile(command, process.platform === 'win32'
  ? `@"${process.execPath}" "${peer}" %*\r\n`
  : `#!/bin/sh\nexec ${quote(process.execPath)} ${quote(peer)} "$@"\n`);
await chmod(command, 0o700);
const entry = process.env.HIVE_DESKTOP_TEST_RUNTIME ?? fileURLToPath(new URL('../dist/main.js', import.meta.url));

function startRuntime() {
  const child = spawn(process.execPath, [entry], { cwd: directory, stdio: ['pipe', 'pipe', 'pipe'],
    env: { ...process.env, HIVE_DESKTOP_DATA_DIR: data, HIVE_CODEX_COMMAND: command,
      HIVE_VIEW_FIXTURE_DIRECTORY: directory, PI_CODING_AGENT_DIR: agent, PI_OFFLINE: '1' } });
  const exited = once(child, 'exit');
  const lines = createInterface({ input: child.stdout });
  const pending = new Map<number, { resolve(value: any): void; reject(error: Error): void }>();
  let sequence = 0;
  let diagnostics = '';
  child.stderr.on('data', chunk => { diagnostics += String(chunk); });
  lines.on('line', line => {
    const response = JSON.parse(line);
    const request = pending.get(response.id);
    if (!request) return;
    pending.delete(response.id);
    request.resolve(response);
  });
  child.on('exit', () => {
    for (const request of pending.values()) request.reject(new Error(`Runtime exited before reply: ${diagnostics}`));
    pending.clear();
  });
  async function rpc(method: string, params: Record<string, unknown> = {}) {
    const response = await new Promise<any>((resolve, reject) => {
      const id = ++sequence;
      const timer = setTimeout(() => {
        pending.delete(id);
        reject(new Error(`Timed out: ${method}\n${diagnostics}`));
      }, 15_000);
      pending.set(id, { resolve: value => { clearTimeout(timer); resolve(value); },
        reject: error => { clearTimeout(timer); reject(error); } });
      child.stdin.write(JSON.stringify({ id, method, params }) + '\n');
    });
    if (response.error) throw Object.assign(new Error(response.error.message), {code: response.error.code});
    return response.result;
  }
  return { rpc, nextRequestId: () => sequence + 1,
    async close() {
      await rpc('shutdown');
      assert.equal((await exited)[0], 0, diagnostics);
      lines.close();
    },
    async terminate() {
      if (child.exitCode === null && child.signalCode === null) { child.kill(); await exited; }
      lines.close();
    },
  };
}

type PeerRequest = { method: string; pid: number; threadId?: string; cwd?: string; input?: { type: string; text?: string }[]; additionalContext?: unknown; deferGoalContinuation?: boolean };
async function requests(): Promise<PeerRequest[]> {
  const wire = await readFile(join(directory, 'wire.jsonl'), 'utf8').catch((error: NodeJS.ErrnoException) => {
    if (error.code !== 'ENOENT') throw error;
    return '';
  });
  return wire.split('\n').filter(Boolean).map(line => JSON.parse(line));
}
function alive(pid: number): boolean {
  try { process.kill(pid, 0); return true; }
  catch (error) { assert.equal((error as NodeJS.ErrnoException).code, 'ESRCH'); return false; }
}
async function waitFor<T>(description: string, test: () => T | Promise<T>): Promise<NonNullable<T>> {
  const deadline = Date.now() + 10_000;
  for (;;) {
    const result = await test();
    if (result) return result as NonNullable<T>;
    assert.ok(Date.now() < deadline, `Timed out waiting for ${description}`);
    await delay(25);
  }
}
const peers = async () => (await requests()).filter(row => row.method === 'process/start').map(row => row.pid);
const lastRequest = async (method: string, threadId: string, after = 0) =>
  (await requests()).slice(after).findLast(row => row.method === method && row.threadId === threadId);
const hold = (threadId: string, phase: string) => writeFile(join(directory, `${threadId}.hold-${phase}`), '');
const release = (threadId: string, phase: string) => rm(join(directory, `${threadId}.hold-${phase}`));
const historyIds = (threadId: string) => [`${threadId}-image`, `${threadId}-marker`];

let runtime = startRuntime();
try {
  await runtime.rpc('initialize');
  const workspace = await runtime.rpc('hive.request', { method: 'POST', path: '/api/workspaces',
    body: { path: project, name: 'Codex desktop browse fixture', autostart_orchestrator: false } });
  const seed = async () => (await runtime.rpc('chat.createRecord', { workspaceId: workspace.id, backend: 'codex' })).id as string;
  const [a, b, old] = [await seed(), await seed(), await seed()];
  await runtime.close();
  const index = new ChatIndex(join(data, 'chats'));
  for (const [id, thread] of [[a, 'thread-a'], [b, 'thread-b'], [old, 'thread-legacy']] as const) {
    index.save({ ...index.get(id), identityState: 'known', originAgentId: `${workspace.id}:orchestrator`, codexThreadId: thread });
  }

  runtime = startRuntime();
  const { rpc } = runtime;
  await rpc('initialize');
  const open = (sessionId: string, viewRevision: number) => rpc('chat.open', { sessionId, viewRevision, deferHistoryImages: true });
  const clear = (viewRevision: number) => rpc('chat.clearView', { viewRevision });

  await assert.rejects(open(a, 0));
  await assert.rejects(clear(Number.MAX_SAFE_INTEGER + 1));
  assert.deepEqual(await peers(), []);
  const first = await open(a, 1);
  assert.equal(first.codexThreadId, 'thread-a');
  assert.deepEqual(first.historyMessageIds, historyIds('thread-a'));
  assert.match(JSON.stringify(first.history[1]), /Saved reply for thread-a/);
  assert.ok(first.models.some((model: { id: string }) => model.id === 'gpt-6.1-terra'));
  assert.ok((await rpc('chat.mode', { sessionId: a, viewRevision: 1 })).options.some((mode: { id: string }) => mode.id === 'native-plan'));
  assert.ok((await rpc('chat.permissionMode', { sessionId: a, viewRevision: 1 })).options.some((mode: { id: string }) => mode.id === 'read-only'));
  const originalPeer = (await lastRequest('thread/resume', 'thread-a'))!;
  assert.equal(originalPeer.deferGoalContinuation, true);
  assert.deepEqual(await peers(), [originalPeer.pid]);

  await hold('thread-a', 'read');
  const beforeRead = (await requests()).length;
  const reading = rpc('chat.reasoning', { sessionId: a, viewRevision: 1 }).then(value => ({ value }), error => ({ error }));
  await waitFor('in-flight reasoning read', () => lastRequest('thread/read', 'thread-a', beforeRead));
  await hold('thread-b', 'resume');
  const openingB = open(b, 2).then(value => ({ value }), error => ({ error }));
  const stalePeer = await waitFor('slow B resume', () => lastRequest('thread/resume', 'thread-b'));
  assert.equal(alive(originalPeer.pid), true, 'Switching away must preserve the running read');
  await assert.rejects(rpc('chat.mode', { sessionId: a, viewRevision: 1 }));
  const reopeningA = open(a, 3);
  await assert.rejects(rpc('chat.mode', { sessionId: b, viewRevision: 2 }));
  assert.equal(alive(originalPeer.pid), true, 'The superseded read still holds its original process');
  await release('thread-a', 'read');
  await release('thread-b', 'resume');
  const reopened = await reopeningA;
  assert.equal(reopened.codexThreadId, first.codexThreadId);
  assert.deepEqual(reopened.historyMessageIds, first.historyMessageIds);
  assert.deepEqual(reopened.history, first.history);
  assert.ok('error' in await reading, 'The delayed reasoning reply belongs to the previous view');
  assert.ok('error' in await openingB, 'The delayed open belongs to the previous view');
  await waitFor('reaping the superseded B process', () => !alive(stalePeer.pid));
  const liveA = (await lastRequest('thread/resume', 'thread-a'))!;
  const beforeClear = (await peers()).length;
  await clear(4);
  await waitFor('reaping the last selected browse process', () => !alive(liveA.pid));
  await assert.rejects(open(b, 2));
  await assert.rejects(rpc('chat.permissionMode', { sessionId: a, viewRevision: 3 }));
  const image = await rpc('chat.historyImage', { workspaceId: workspace.id, sessionId: a, messageId: historyIds('thread-a')[0], imageIndex: 0 });
  assert.equal(image.data, 'aGl2ZSB2aWV3IGZpeHR1cmU=');
  assert.equal((await peers()).length, beforeClear, 'A delayed RPC or a saved image must not start a process');
  assert.equal((await requests()).filter(row => row.method === 'turn/start').length, 0);

  await open(a, 5);
  const executionPeer = (await lastRequest('thread/resume', 'thread-a'))!;
  await hold('thread-a', 'turn');
  const accepted = await rpc('chat.prompt', { sessionId: a, text: 'Keep the running turn while browsing another conversation.', requireIdle: true });
  assert.equal(accepted.session.id, a);
  assert.equal(accepted.accepted, true);
  await waitFor('first native turn', () => lastRequest('turn/start', 'thread-a'));
  await assert.rejects(rpc('chat.prompt', { sessionId: a, text: 'Do not queue this shortcut while busy.', requireIdle: true }), /conversation became busy/);
  await rpc('chat.prompt', { sessionId: a, text: 'Preserve this queued delivery.' });
  const busy = await rpc('chat.open', {
    sessionId: a, viewRevision: 6, historyLimit: 1,
    historyAroundMessageId: historyIds('thread-a')[0], deferHistoryImages: true,
  });
  assert.equal(busy.busy, true);
  assert.ok(busy.runId);
  assert.ok(busy.pendingUserInputs.some((item: { text: string }) => item.text === 'Preserve this queued delivery.'));
  assert.deepEqual(busy.historyMessageIds, [historyIds('thread-a')[0]]);
  assert.equal(busy.historyPage.hasNewer, true);
  const nextParams = {
    sessionId: a, viewRevision: 6, limit: 1,
    afterIndex: busy.historyPage.nextAfterIndex,
    afterMessageId: busy.historyPage.nextAfterMessageId,
  };
  const next = await rpc('chat.historyPage', nextParams);
  assert.deepEqual(next.historyMessageIds, [historyIds('thread-a')[1]]);
  assert.equal(next.historyPage.hasOlder, true);
  const previous = await rpc('chat.historyPage', {
    sessionId: a, viewRevision: 6, limit: 1,
    beforeIndex: next.historyPage.nextBeforeIndex,
    beforeMessageId: next.historyPage.nextBeforeMessageId,
    deferHistoryImages: true,
  });
  assert.deepEqual(previous.historyMessageIds, busy.historyMessageIds);
  assert.deepEqual(previous.history, busy.history);
  await assert.rejects(rpc('chat.historyPage', {...nextParams, beforeIndex: 0}), /either a before or an after/);
  const draft = await rpc('chat.createRecord', {workspaceId: workspace.id, backend: 'codex', profile: accepted.session.profile, deferFirstConnection: true});
  const draftPeers = await peers();
  const draftView = await rpc('chat.open', {sessionId: draft.id, viewRevision: 7, historyLimit: 1});
  assert.equal(draftView.savedHistoryPassive, true);
  assert.equal(draftView.readOnly, false);
  assert.deepEqual(draftView.history, []);
  await assert.rejects(rpc('chat.prompt', {sessionId: draft.id, text: 'Do not move the running Agent into the draft.'}), /\[SESSION_ORIGIN_BUSY\]/);
  assert.deepEqual(await peers(), draftPeers, 'Opening the draft cannot start a second Agent');
  assert.equal(alive(executionPeer.pid), true, 'The original turn keeps running in the background');
  await open(b, 8);
  await assert.rejects(rpc('chat.historyPage', nextParams), /Stale desktop conversation view/);
  const activeB = (await lastRequest('thread/resume', 'thread-b'))!;
  await clear(9);
  await waitFor('reaping B without affecting A', () => !alive(activeB.pid));
  assert.equal(alive(executionPeer.pid), true);
  await release('thread-a', 'turn');
  await waitFor('both queued deliveries on the original process', async () =>
    (await requests()).filter(row => row.method === 'turn/start' && row.pid === executionPeer.pid).length === 2);
  let revision = 9;
  const settled = await waitFor('completed delivery queue', async () => {
    const state = await open(a, ++revision);
    return !state.busy && state.pending === 0 && state;
  });
  assert.equal(settled.runId, busy.runId);
  assert.match(JSON.stringify(settled.history), /Preserve this queued delivery/);
  const liveMatch = await rpc('chat.searchMessages', {sessionId: a, viewRevision: revision, query: 'preserve this queued delivery'});
  const queuedId = busy.pendingUserInputs.find((item: {text: string}) => item.text === 'Preserve this queued delivery.').deliveryId;
  assert.ok(liveMatch.matches.some((match: {messageId: string}) => match.messageId === queuedId), 'Live search returns a delivery row ID');
  await clear(++revision);
  assert.equal(alive(executionPeer.pid), true, 'The idle Hive run still owns this process');

  await writeFile(join(directory, 'version'), '0.168.0');
  const legacyConnected = await rpc('chat.open', {sessionId: old, viewRevision: ++revision, historyLimit: 1, preferSavedHistory: false});
  assert.equal(legacyConnected.savedHistoryPassive, undefined);
  assert.ok((await rpc('chat.mode', {sessionId: old, viewRevision: revision})).options.length);
  const legacy = (await lastRequest('thread/resume', 'thread-legacy'))!;
  assert.equal(legacy.deferGoalContinuation, undefined);
  await clear(++revision);
  await writeFile(join(directory, 'version'), '0.169.0');
  await open(b, ++revision);
  const finalB = (await lastRequest('thread/resume', 'thread-b'))!;
  // Use a supported peer to establish a completed reclamation before checking legacy retention.
  assert.equal(finalB.deferGoalContinuation, true);
  await clear(++revision);
  await waitFor('reaping a supported peer', () => !alive(finalB.pid));
  assert.equal(alive(legacy.pid), true, 'A peer without deferred continuation must retain the existing lifecycle');

  const peersBeforeSavedBrowse = await peers();
  const turnsBeforeSavedBrowse = (await requests()).filter(row => row.method === 'turn/start').length;
  const cold = await rpc('chat.open', {sessionId: b, viewRevision: ++revision, historyLimit: 1});
  assert.equal(cold.readOnly, false);
  assert.equal(cold.savedHistoryPassive, true);
  assert.equal(cold.passive, true);
  assert.equal(typeof cold.historyPage.snapshot, 'string');
  assert.equal(cold.history.length, 1);
  assert.deepEqual((await rpc('chat.mode', {sessionId: b, viewRevision: revision})).options, []);
  assert.deepEqual((await rpc('chat.permissionMode', {sessionId: b, viewRevision: revision})).options, []);
  assert.deepEqual((await rpc('chat.reasoning', {sessionId: b, viewRevision: revision})).levels, []);
  const query = {sessionId: b, viewRevision: revision, query: 'saved reply for thread-b'};
  assert.deepEqual((await rpc('chat.searchMessages', query)).matches, [{messageId: historyIds('thread-b')[1]}]);
  const requestId = runtime.nextRequestId();
  const cancelled = assert.rejects(rpc('chat.searchMessages', query), {code: 'SEARCH_CANCELLED'});
  assert.deepEqual(await rpc('chat.search.cancel', {requestId}), {cancelled: true});
  await cancelled;
  assert.deepEqual(await peers(), peersBeforeSavedBrowse, 'Cold browse and its metadata requests leave the Agent stopped');
  const explicit = await rpc('chat.open', {sessionId: b, viewRevision: ++revision, historyLimit: 1, preferSavedHistory: false});
  assert.equal(explicit.savedHistoryPassive, undefined);
  assert.equal(explicit.historyPage.snapshot, undefined);
  assert.ok((await rpc('chat.mode', {sessionId: b, viewRevision: revision})).options.length);
  assert.equal((await requests()).filter(row => row.method === 'turn/start').length, turnsBeforeSavedBrowse);
  await hold('thread-b', 'settings');
  const setting = rpc('chat.setModel', { sessionId: b, provider: 'openai', modelId: 'fixture-alternate' });
  const settingPeer = await waitFor('pending native model change', () => lastRequest('thread/settings/update', 'thread-b'));
  await clear(++revision);
  assert.ok(alive(settingPeer.pid), 'Switching away must preserve the native model change');
  await release('thread-b', 'settings');
  assert.equal((await setting).ok, true);
  assert.equal((await open(b, ++revision)).model.id, 'fixture-alternate');

  const started = await peers();
  await runtime.close();
  assert.ok(started.every(pid => !alive(pid)), 'Shutdown reaps every owned process');
  runtime = startRuntime();
  await runtime.rpc('initialize');
  const reopenedDraft = await runtime.rpc('chat.open', {sessionId: draft.id, viewRevision: 1, historyLimit: 1});
  assert.equal(reopenedDraft.savedHistoryPassive, true);
  assert.deepEqual(reopenedDraft.history, []);
  assert.deepEqual(await peers(), started, 'A blank draft stays disconnected after restart');
  const draftText = 'First explicit input for the new draft.';
  const draftMessage = await runtime.rpc('chat.prompt', {sessionId: draft.id, text: draftText, requireIdle: true});
  assert.equal(draftMessage.session.id, draft.id);
  assert.equal(draftMessage.session.deferFirstConnection, undefined);
  const draftTurn = await waitFor('first explicit draft delivery', async () => (await requests()).find(row =>
    row.method === 'turn/start' && row.input?.some(item => item.text?.split('\n', 1)[0] === draftText)));
  assert.notEqual(draftTurn.threadId, 'thread-a');
  const openedDraft = await runtime.rpc('chat.open', {sessionId: draft.id, viewRevision: 2, historyLimit: 1});
  assert.equal(openedDraft.codexThreadId, draftTurn.threadId);

  const nested = join(project, 'shared worktree scope');
  await mkdir(nested);
  const git = (...args: string[]) => execFileSync('git', ['-c', 'user.name=Fixture',
    '-c', 'user.email=fixture@example.test', '-c', 'commit.gpgSign=false', ...args], {cwd: project, stdio: 'pipe'});
  git('init', '--template=');
  await writeFile(join(nested, 'marker.txt'), 'shared physical file');
  git('add', '.');
  git('commit', '-qm', 'Shared folder fixture');
  const sharedWorkspace = await runtime.rpc('hive.request', {method: 'POST', path: '/api/workspaces',
    body: {path: nested, name: 'Shared physical checkout', autostart_orchestrator: false}});
  const owner = await runtime.rpc('chat.createRecord', {workspaceId: sharedWorkspace.id, backend: 'codex', isolation: 'worktree'});
  const turnWith = (text: string) => waitFor('synthetic turn', async () => (await requests()).find(row =>
    row.method === 'turn/start' && row.input?.some(item => item.text?.split('\n', 1)[0] === text)));
  const waitForSharedTurnToSettle = (sessionId: string) => waitFor('settled shared worktree input', async () => {
    const snapshot = await runtime.rpc('chat.open', {sessionId});
    return !snapshot.busy && snapshot.pending === 0;
  });
  const ownerOnly = 'Owner-only recent prompt ' + randomUUID();
  assert.ok((await runtime.rpc('chat.prompt', {sessionId: owner.id, text: ownerOnly})).accepted);
  const ownerTurn = await turnWith(ownerOnly);
  await waitForSharedTurnToSettle(owner.id);
  await runtime.close();

  runtime = startRuntime();
  await runtime.rpc('initialize');
  const share = {workspaceId: sharedWorkspace.id, sourceSessionId: owner.id, requestId: randomUUID()};
  const beforeSharedOpen = await peers();
  const shared = await runtime.rpc('chat.createInWorktreeRecord', share);
  const sharedOpen = await runtime.rpc('chat.open', {sessionId: shared.id, viewRevision: 1, historyLimit: 1});
  assert.equal(sharedOpen.savedHistoryPassive, true);
  assert.deepEqual(sharedOpen.history, []);
  assert.deepEqual(await peers(), beforeSharedOpen);
  const sharedOnly = 'Shared session first prompt ' + randomUUID();
  assert.ok((await runtime.rpc('chat.prompt', {sessionId: shared.id, text: sharedOnly})).accepted);
  const sharedTurn = await turnWith(sharedOnly);
  const sharedStart = (await requests()).find(row => row.method === 'thread/start' && row.pid === sharedTurn.pid);
  assert.notEqual(sharedTurn.threadId, ownerTurn.threadId);
  assert.equal(sharedStart?.cwd, owner.execution.path);
  assert.equal(shared.execution.ownerSessionId, owner.id);
  const sharedContext = JSON.stringify(sharedTurn.additionalContext ?? null);
  assert.ok(sharedContext.includes('Hive team workspace'));
  assert.ok(!sharedContext.includes('Conversation with the user in the last hour'));
  assert.ok(!sharedContext.includes(ownerOnly));
  await waitForSharedTurnToSettle(shared.id);
  await runtime.close();
  console.log('CODEX_DESKTOP_VIEW_OK: stale selections, saved history and image bytes, durable queue, detached draft and cold reopen, model settings, Hive and legacy ownership');
} finally {
  await runtime.terminate();
  await rm(directory, { recursive: true, force: true });
}

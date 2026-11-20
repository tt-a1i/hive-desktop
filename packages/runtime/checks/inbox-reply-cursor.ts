/** Exact inbox reply cursor through the production wire and a controlled ACP peer. */
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {randomUUID} from 'node:crypto';
import {once} from 'node:events';
import {mkdir, mkdtemp, readFile, rm, writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {createInterface} from 'node:readline';
import {DatabaseSync} from 'node:sqlite';
import {fileURLToPath} from 'node:url';
import {setTimeout as delay} from 'node:timers/promises';
import type {ChatRecord} from '../src/chat-index.js';
import {DispatchExecutionOrigins} from '../src/dispatch-execution-origins.js';

const root = await mkdtemp(join(tmpdir(), 'hive-inbox-replies-'));
const dataDir = join(root, 'desktop');
const peerDir = join(root, 'peer');
const peer = fileURLToPath(new URL('./fixtures/acp-runtime-peer.cjs', import.meta.url));
const entry = fileURLToPath(new URL('../dist/main.js', import.meta.url));

function launch() {
  const events: Array<{type: string; sessionId: string; messageId?: string; message?: {role: string}; inboxReply?: {workspaceId: string; revision: number; receivedAt?: number}}> = [];
  const processHandle = spawn(process.execPath, [entry], {
    env: {...process.env, HIVE_DESKTOP_DATA_DIR: dataDir, PI_CODING_AGENT_DIR: join(root, 'pi'), PI_OFFLINE: '1'},
    stdio: ['pipe', 'pipe', 'pipe'],
  });
  let stderr = '';
  processHandle.stderr.on('data', chunk => {stderr += String(chunk);});
  const exit = once(processHandle, 'exit');
  const reader = createInterface({input: processHandle.stdout});
  const pending = new Map<number, (frame: {result?: unknown; error?: unknown}) => void>();
  let sequence = 0;
  reader.on('line', line => {
    const frame = JSON.parse(line);
    if (frame.event === 'agent') events.push(frame.data);
    pending.get(frame.id)?.(frame);
  });
  function request<T>(method: string, params: object = {}): Promise<T> {
    return new Promise((resolve, reject) => {
      const id = ++sequence;
      const timer = setTimeout(() => {
        pending.delete(id);
        reject(new Error(`${method} timed out: ${stderr}`));
      }, 10_000);
      pending.set(id, frame => {
        clearTimeout(timer);
        pending.delete(id);
        if (frame.error !== undefined) reject(new Error(JSON.stringify(frame.error)));
        else resolve(frame.result as T);
      });
      processHandle.stdin.write(JSON.stringify({id, method, params}) + '\n');
    });
  }
  async function close() {
    try { await request('shutdown'); }
    finally {
      processHandle.stdin.end();
      const stopped = await Promise.race([exit.then(() => true), delay(2_000, false)]);
      if (!stopped) processHandle.kill('SIGKILL');
      await exit;
    }
  }
  return {request, events, close};
}

async function reply(wire: ReturnType<typeof launch>, workspaceId: string, sessionId: string, revision: number) {
  const sentAt = Date.now();
  await wire.request('chat.prompt', {sessionId, text: `Test reply ${revision}`});
  for (let count = 0; count < 120; count++) {
    const event = wire.events.find(event => event.type === 'message_end'
      && event.sessionId === sessionId && event.inboxReply?.revision === revision);
    if (event) {
      assert.equal(event.message?.role, 'assistant');
      assert.equal(event.inboxReply?.workspaceId, workspaceId);
      assert.ok(event.messageId);
      const receivedAt = event.inboxReply?.receivedAt;
      assert.ok(receivedAt !== undefined && receivedAt >= sentAt && receivedAt <= Date.now());
      return {revision, messageId: event.messageId, receivedAt};
    }
    await delay(30);
  }
  assert.fail('Expected a finalized assistant reply with its exact revision');
}

let wire: ReturnType<typeof launch> | undefined;
try {
  await mkdir(join(root, 'pi'));
  wire = launch();
  await wire.request('initialize');
  const projectPath = join(root, 'project');
  await mkdir(projectPath);
  const workspace = await wire.request<{id: string}>('hive.request', {
    method: 'POST', path: '/api/workspaces', body: {path: projectPath, name: 'Inbox reply cursor', autostart_orchestrator: false},
  });
  const profileList = await wire.request<{storeVersion: number}>('agent.profiles.list');
  const {profile} = await wire.request<{profile: {ref: {id: string; revision: number}}}>('agent.profiles.create', {
    driver: 'acp', label: 'Controlled inbox peer', expectedStoreVersion: profileList.storeVersion,
    launch: {command: process.execPath, args: [peer, peerDir]},
  });
  const create = (current: ReturnType<typeof launch>) => current.request<{id: string}>('chat.createRecord', {
    workspaceId: workspace.id, backend: 'acp', profile: profile.ref,
  });
  const {id} = await create(wire);
  const other = await create(wire);
  const sessions = async (current: ReturnType<typeof launch>) => current.request<ChatRecord[]>('chat.list', {workspaceId: workspace.id});
  assert.deepEqual((await sessions(wire)).map(chat => chat.inboxReply), [undefined, undefined]);
  const first = await reply(wire, workspace.id, id, 1);
  assert.equal(wire.events.filter(event => event.inboxReply !== undefined && event.sessionId === id).length, 1);
  assert.ok(wire.events.some(event => event.type === 'message_end' && event.message?.role === 'user' && !event.inboxReply));
  await wire.request('chat.rename', {sessionId: id, title: 'Renamed'});
  assert.deepEqual((await sessions(wire)).find(chat => chat.id === id)?.inboxReply, first);
  await wire.close();
  wire = undefined;
  const protocol = join(peerDir, 'protocol.jsonl');
  const before = await readFile(protocol, 'utf8');

  wire = launch();
  await wire.request('initialize');
  const cold = await sessions(wire);
  assert.equal(cold.find(chat => chat.id === id)?.title, 'Renamed');
  assert.deepEqual(cold.find(chat => chat.id === id)?.inboxReply, first);
  assert.equal(cold.find(chat => chat.id === other.id)?.inboxReply, undefined);
  assert.equal(await readFile(protocol, 'utf8'), before, 'Listing cold chats must not launch a provider');
  const second = await reply(wire, workspace.id, id, 2);
  assert.notEqual(first.messageId, second.messageId);
  const final = await sessions(wire);
  assert.deepEqual(final.find(chat => chat.id === id)?.inboxReply, second);
  assert.equal(final.find(chat => chat.id === other.id)?.inboxReply, undefined);
  await assert.rejects(wire.request('chat.archive', {sessionId: id, archived: true,
    inbox: {workspaceId: workspace.id, replyRevision: first.revision}}), /ARCHIVE_INBOX_STALE/);
  assert.equal(Boolean((await sessions(wire)).find(chat => chat.id === id)?.archived), false);

  await wire.close();
  wire = undefined;
  const index = join(dataDir, 'chats', 'index.json');
  const archived: ChatRecord[] = JSON.parse(await readFile(index, 'utf8'));
  for (const record of archived) {
    if (record.id === id || record.id === other.id) {
      record.updatedAt = first.receivedAt - 2 * 86_400_000;
      record.archived = true;
    }
  }
  await writeFile(index, JSON.stringify(archived));
  wire = launch();
  const {hivePort} = await wire.request<{hivePort: number}>('initialize');
  const query = {query: '', archived: true, navigationOnly: true, workspaceId: workspace.id,
    backend: 'acp', limit: 1, activitySinceMs: second.receivedAt};
  const page = await wire.request<{total: number; matches: {session: ChatRecord}[]}>('chat.search', query);
  assert.equal(page.total, 1);
  assert.deepEqual(page.matches.map(({session}) => session.id), [id]);
  assert.deepEqual(page.matches[0]?.session.inboxReply, {revision: second.revision, receivedAt: second.receivedAt});
  const afterBoundary = await wire.request<{total: number}>('chat.search', {...query, activitySinceMs: second.receivedAt + 1});
  assert.equal(afterBoundary.total, 0);
  console.log('INBOX_REPLY_CURSOR_OK: exact finalized ACP replies, cold activity timestamps and archive filtering at the inclusive boundary');

  const early = await wire.request<{id: string}>('team.member.create', {
    workspaceId: workspace.id, name: 'First reviewer', role: 'reviewer', backend: 'pi',
  });
  const later = await wire.request<{id: string}>('team.member.create', {
    workspaceId: workspace.id, name: 'Fourth reviewer', role: 'reviewer', backend: 'pi',
  });
  const foreignPath = join(root, 'other-project');
  await mkdir(foreignPath);
  const foreign = await wire.request<{id: string}>('hive.request', {
    method: 'POST', path: '/api/workspaces', body: {path: foreignPath, name: 'Other project', autostart_orchestrator: false},
  });
  const historical = await wire.request<{id: string}>('chat.createRecord', {workspaceId: workspace.id, backend: 'codex'});
  const waiting = Array.from({length: 4}, () => randomUUID());
  new DispatchExecutionOrigins(join(dataDir, 'chats', 'dispatch-execution-origins')).put({
    workspaceId: workspace.id, dispatchId: waiting[3]!, sourceSessionId: historical.id, sourceAgentId: later.id,
  });
  const sqlite = new DatabaseSync(join(dataDir, 'hive', 'runtime.sqlite'));
  try {
    const insert = sqlite.prepare(`INSERT INTO dispatches
      (id, workspace_id, from_agent_id, to_agent_id, text, status, created_at, submitted_at, root_dispatch_id, label)
      VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`);
    const now = Date.now();
    function seed(workspaceId: string, member: string, dispatchId: string, status: string, time: number,
        label: string | null = null, body = 'Private fixture dispatch', createdAt = time) {
      insert.run(dispatchId, workspaceId, `${workspaceId}:orchestrator`, member, body, status, createdAt, time, dispatchId, label);
    }
    const titles = ['Audit migration', null, 'Audit migration', '  '];
    const bodies = ['Do not include body with label', `  ${'🐝'.repeat(103)}\nprivate body`, 'Duplicate task label', ' \n '];
    waiting.forEach((dispatchId, index) => seed(workspace.id, index === 3 ? later.id : early.id,
      dispatchId, 'submitted', now - 720_000, titles[index]!, bodies[index]!, now - 720_000 + index));
    seed(workspace.id, later.id, randomUUID(), 'submitted', now);
    seed(workspace.id, later.id, randomUUID(), 'reported', now - 720_000);
    seed(workspace.id, later.id, randomUUID(), 'cancelled', now - 720_000);
    seed(foreign.id, `${foreign.id}:orchestrator`, randomUUID(), 'submitted', now - 720_000);
  } finally {
    sqlite.close();
  }
  type Attention = {
    kind: string; dispatch_id?: string; message_id?: string; worker_id?: string; worker_name?: string;
    message_sender_id?: string; message_created_at?: number; message_preview?: string | null;
    dispatch_backend?: string | null; severity?: string; submitted_at?: number; minutes_ago?: number; task_title?: string | null;
  };
  type Cursor = {through: number; nextAfter: number | null};
  type Center = {summary: {waiting_reports: number}; attention: Attention[]; recent_dispatch_messages: {dispatch_id: string}[]; inbox_actionable_cursor?: Cursor};
  const actionCenter = `/api/ui/workspaces/${workspace.id}/action-center`;
  const base = `http://127.0.0.1:${hivePort}`;
  const rawUnauthorized = await fetch(base + actionCenter);
  assert.ok(!rawUnauthorized.ok);
  await rawUnauthorized.body?.cancel();
  const session = await fetch(base + '/api/ui/session');
  const cookie = session.headers.get('set-cookie')?.split(';', 1)[0];
  await session.body?.cancel();
  assert.ok(cookie);
  const raw = await fetch(base + actionCenter, {headers: {cookie, origin: base}});
  assert.ok(raw.ok);
  const upstream: Center = await raw.json();
  const desktop = await wire.request<Center>('hive.request', {method: 'GET', path: actionCenter});
  const reportRows = (center: Center) => center.attention.filter(row => row.kind === 'dispatch_waiting_report');
  assert.equal(upstream.summary.waiting_reports, 4);
  assert.deepEqual(reportRows(upstream).map(row => row.dispatch_id), waiting.slice(0, 3));
  assert.equal(desktop.summary.waiting_reports, 4);
  assert.deepEqual(reportRows(desktop).map(row => row.dispatch_id), waiting);
  assert.ok(reportRows(upstream).every(row => !('task_title' in row)));
  assert.deepEqual(reportRows(desktop).map(row => row.task_title), ['Audit migration', `${'🐝'.repeat(96)}…`, 'Audit migration', null]);
  assert.equal(new Set(reportRows(desktop).map(row => row.submitted_at)).size, 1);
  const scoped = reportRows(desktop).filter(row => row.worker_id === later.id && row.dispatch_backend === 'codex');
  assert.deepEqual(scoped.map(row => [row.dispatch_id, row.worker_name]), [[waiting[3], 'Fourth reviewer']]);
  assert.deepEqual(Object.keys(scoped[0]!).sort(),
    ['dispatch_backend', 'dispatch_id', 'kind', 'minutes_ago', 'severity', 'submitted_at', 'task_title', 'worker_id', 'worker_name']);
  console.log('INBOX_WAITING_REPORTS_OK: production wire includes the fourth exact member/backend while the authenticated HTTP preview remains capped');
  console.log('INBOX_WAITING_REPORT_TITLES_OK: scoped task titles, same-time duplicates, blank fallback, bounded Unicode and desktop-only evidence');

  const watched = waiting[3]!;
  const unrelated = waiting[0]!;
  const stamp = Date.now() - 1000;
  function insertInputs(withNewInput: boolean) {
    const db = new DatabaseSync(join(dataDir, 'hive', 'runtime.sqlite'));
    try {
      const add = db.prepare(`INSERT INTO dispatch_messages
        (id, workspace_id, dispatch_id, sequence, from_agent_id, recipient_agent_id, kind, text, created_at)
        VALUES (?, ?, ?, ?, ?, ?, 'note', 'Private fixture message', ?)`);
      const message = (dispatch: string, sequence: number, time: number) => {
        const id = randomUUID();
        add.run(id, workspace.id, dispatch, sequence, later.id, `${workspace.id}:orchestrator`, time);
        return id;
      };
      if (withNewInput) {
        // Clock rollback can make a newer sequence older by wall clock. The
        // time-only anchor must still detect the next eligible input by time.
        message(watched, 8, stamp + 50);
        message(watched, 9, stamp + 25);
      } else {
        message(watched, 7, stamp);
        const cancelled = message(watched, 10, stamp + 100);
        db.prepare("INSERT INTO dispatch_message_outbox (message_id, state) VALUES (?, 'cancelled')").run(cancelled);
      }
      for (let i = 1; i <= 12; i++) message(unrelated, i + (withNewInput ? 12 : 0), stamp + 200 + i);
    } finally {
      db.close();
    }
  }
  type DispatchInput = {dispatchId: string; sequence: number | null; createdAt: number | null};
  type InputBatch = {workspaceId: string; inputs: DispatchInput[]};
  const queryInput = (workspaceId: string, time = true) => wire!.request<InputBatch>('inbox.dispatchInput', {
    workspaceId, dispatchIds: [watched], ...(time && { timestampDispatchIds: [watched] }),
  });
  insertInputs(false);
  assert.deepEqual(await queryInput(workspace.id), {workspaceId: workspace.id, inputs: [{dispatchId: watched, sequence: 7, createdAt: stamp}]});
  assert.deepEqual(await queryInput(foreign.id), {workspaceId: foreign.id, inputs: [{dispatchId: watched, sequence: null, createdAt: null}]});
  insertInputs(true);
  const buried = await wire.request<Center>('hive.request', {method: 'GET', path: actionCenter});
  assert.equal(buried.recent_dispatch_messages.length, 12);
  assert.ok(buried.recent_dispatch_messages.every(message => message.dispatch_id === unrelated));
  assert.deepEqual(await queryInput(workspace.id), {workspaceId: workspace.id, inputs: [{dispatchId: watched, sequence: 9, createdAt: stamp + 50}]});
  assert.deepEqual(await queryInput(workspace.id, false), {workspaceId: workspace.id, inputs: [{dispatchId: watched, sequence: 9, createdAt: null}]});
  await assert.rejects(wire.request('inbox.dispatchInput', {workspaceId: workspace.id, dispatchIds: [watched], timestampDispatchIds: [unrelated]}));
  await wire.close();
  wire = undefined;
  wire = launch();
  const {hivePort: restartedHivePort} = await wire.request<{hivePort: number}>('initialize');
  assert.deepEqual(await queryInput(workspace.id), {workspaceId: workspace.id, inputs: [{dispatchId: watched, sequence: 9, createdAt: stamp + 50}]});
  console.log('INBOX_DISPATCH_INPUT_OK: exact eligible input behind workspace preview, ask scope isolation, and cold restart');

  const detailed = waiting[2]!;
  const questions = [randomUUID(), randomUUID()];
  const cancelledQuestion = randomUUID();
  const detailSqlite = new DatabaseSync(join(dataDir, 'hive', 'runtime.sqlite'));
  try {
    const add = detailSqlite.prepare(`INSERT INTO dispatch_messages
      (id, workspace_id, dispatch_id, sequence, from_agent_id, recipient_agent_id, kind, text, created_at)
      VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`);
    const insert = (id: string, sequence: number, kind: string, text: string) =>
      add.run(id, workspace.id, detailed, sequence, early.id, `${workspace.id}:orchestrator`, kind, text, stamp + sequence);
    for (let i = 1; i <= 100; i++) insert(randomUUID(), i, 'note', `Older note ${i}`);
    for (const [index, id] of questions.entries()) insert(id, 101 + index, 'question', `Exact question ${index + 1}\nPrivate second line`);
    insert(cancelledQuestion, 103, 'question', 'Cancelled question');
    detailSqlite.prepare("INSERT INTO dispatch_message_outbox (message_id, state) VALUES (?, 'cancelled')").run(cancelledQuestion);
    for (let i = 25; i <= 37; i++) {
      add.run(randomUUID(), workspace.id, unrelated, i, later.id, `${workspace.id}:orchestrator`,
        'note', `Unrelated recent note ${i}`, stamp + 300 + i);
    }
  } finally {
    detailSqlite.close();
  }
  const askCenter = await wire.request<Center>('hive.request', {method: 'GET', path: actionCenter});
  assert.equal(askCenter.recent_dispatch_messages.length, 12);
  assert.ok(askCenter.recent_dispatch_messages.every(row => row.dispatch_id === unrelated));
  assert.deepEqual(askCenter.attention.filter(row => row.kind === 'question_waiting_answer')
    .map(row => [row.message_id, row.message_sender_id, row.message_preview, row.message_created_at]),
    questions.map((id, index) => [id, early.id, `Exact question ${index + 1}`, stamp + 101 + index]));
  const history = await wire.request<{messages: {id: string; kind: string}[]; next_after_message_id: string}>(
    'hive.request', {method: 'GET', path: `/api/ui/workspaces/${workspace.id}/dispatches/${detailed}/messages?scope=collaboration`});
  assert.equal(history.messages.length, 100);
  assert.ok(history.next_after_message_id);
  assert.ok(history.messages.every(message => message.kind === 'note'));
  const readSelected = (messageId: string, workspaceId = workspace.id, dispatchId = detailed) => wire!.request<{
    id: string; workspace_id: string; dispatch_id: string; kind: string; text: string;
  } | null>('inbox.dispatchMessage', {workspaceId, dispatchId, messageId});
  for (const index of [1, 0]) {
    const id = questions[index]!;
    const message = await readSelected(id);
    assert.ok(message);
    assert.equal(message.id, id);
    assert.equal(message.dispatch_id, detailed);
    assert.equal(message.workspace_id, workspace.id);
    assert.equal(message.kind, 'question');
    assert.equal(message.text, `Exact question ${index + 1}\nPrivate second line`);
  }
  assert.equal(await readSelected(questions[0]!, foreign.id), null);
  assert.equal(await readSelected(questions[0]!, workspace.id, unrelated), null);
  assert.equal(await readSelected(randomUUID()), null);
  assert.equal(await readSelected(cancelledQuestion), null);
  console.log('INBOX_EXACT_QUESTION_OK: two questions past the 100-row history, exact full text and workspace/dispatch isolation');

  const newestQuestion = randomUUID();
  const foreignDispatch = randomUUID();
  const foreignQuestion = randomUUID();
  const queuedNote = randomUUID();
  const deliveringNoise = randomUUID();
  const lateQuestion = randomUUID();
  function addInboxBacklog(late = false) {
    const db = new DatabaseSync(join(dataDir, 'hive', 'runtime.sqlite'));
    try {
      const add = db.prepare(`INSERT INTO dispatch_messages
        (id, workspace_id, dispatch_id, sequence, from_agent_id, recipient_agent_id, kind, text, created_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`);
      const insert = (id: string, dispatch: string, sequence: number, kind: string, body = 'Private fixture content') =>
        add.run(id, workspace.id, dispatch, sequence, later.id, `${workspace.id}:orchestrator`, kind, body, stamp + sequence);
      if (late) {
        insert(lateQuestion, waiting[3]!, 12, 'question');
        return;
      }
      insert(queuedNote, detailed, 104, 'note');
      insert(deliveringNoise, detailed, 105, 'note');
      const outbox = db.prepare('INSERT INTO dispatch_message_outbox (message_id, state) VALUES (?, ?)');
      outbox.run(queuedNote, 'queued');
      outbox.run(deliveringNoise, 'delivering');
      for (let i = 0; i < 98; i++) insert(randomUUID(), detailed, 106 + i, 'question');
      insert(newestQuestion, waiting[3]!, 11, 'question', `\r\n Review\t status ${'🐝'.repeat(500)}\nPrivate remainder`);
      for (let i = 38; i <= 50; i++) insert(randomUUID(), unrelated, i, 'note');
      db.prepare(`INSERT INTO dispatches
        (id, workspace_id, status, text, to_agent_id, created_at, root_dispatch_id)
        VALUES (?, ?, 'submitted', 'Foreign dispatch', ?, ?, ?)`).run(foreignDispatch, foreign.id, `${foreign.id}:orchestrator`, stamp, foreignDispatch);
      add.run(foreignQuestion, foreign.id, foreignDispatch, 1, `${foreign.id}:orchestrator`,
        `${foreign.id}:orchestrator`, 'question', 'Foreign fixture content', stamp);
    } finally {
      db.close();
    }
  }
  addInboxBacklog();
  type ActionablePage = Cursor & {workspaceId: string; questionsOnly: boolean; rows: Attention[]};
  const readTail = (page: Cursor, questionsOnly: boolean) => wire!.request<ActionablePage>('inbox.actionablePage',
    {workspaceId: workspace.id, after: page.nextAfter, through: page.through, questionsOnly});
  const initial = await wire.request<Center>('hive.request', {method: 'GET', path: actionCenter});
  const initialRows = initial.attention.filter(row => ['question_waiting_answer', 'message_delivery_pending'].includes(row.kind));
  const initialCursor = initial.inbox_actionable_cursor;
  assert.ok(initialCursor?.nextAfter);
  assert.ok(initial.recent_dispatch_messages.every(row => row.dispatch_id === unrelated));
  assert.equal(initialRows.length, 100);
  assert.ok(initialRows.some(row => row.message_id === queuedNote && row.kind === 'message_delivery_pending'));
  assert.ok(!initialRows.some(row => [deliveringNoise, cancelledQuestion, foreignQuestion, newestQuestion].includes(row.message_id!)));
  assert.equal(initialRows[0]?.message_sender_id, early.id);
  assert.equal(initialRows[0]?.message_created_at, stamp + 101);
  const firstBadge = await wire.request<ActionablePage[]>('inbox.actionableQuestionPages', {workspaceIds: [workspace.id, foreign.id]});
  const exactBadge = firstBadge.find(page => page.workspaceId === workspace.id);
  const foreignBadge = firstBadge.find(page => page.workspaceId === foreign.id);
  assert.ok(exactBadge?.nextAfter);
  assert.equal(exactBadge.rows.length, 100);
  assert.ok(exactBadge.rows.every(row => Object.keys(row).sort().join() === 'dispatch_id,kind,message_id'));
  assert.deepEqual(foreignBadge?.rows.map(row => row.message_id), [foreignQuestion]);
  assert.equal(foreignBadge?.nextAfter, null);
  addInboxBacklog(true);
  const openTail = await readTail(initialCursor, false);
  assert.equal(openTail.nextAfter, null);
  assert.equal(openTail.rows.length, 2);
  assert.deepEqual(openTail.rows.filter(row => row.message_id === newestQuestion)
    .map(row => [row.worker_id, row.message_sender_id, row.dispatch_backend, row.message_created_at, row.message_preview]),
    [[`${workspace.id}:orchestrator`, later.id, 'codex', stamp + 11, `Review status ${'🐝'.repeat(82)}…`]]);
  assert.ok(!openTail.rows.some(row => row.message_id === lateQuestion || 'text' in row || 'message_sample' in row));
  const badgeTail = await readTail(exactBadge, true);
  assert.deepEqual(badgeTail.rows.map(row => row.message_id), [newestQuestion]);
  assert.ok(badgeTail.rows.every(row => Object.keys(row).sort().join() === 'dispatch_id,kind,message_id'));
  assert.equal(badgeTail.nextAfter, null);
  await assert.rejects(wire.request('inbox.actionablePage', {workspaceId: randomUUID(), after: 1, through: exactBadge.through}));
  await assert.rejects(wire.request('inbox.actionablePage', {workspaceId: workspace.id, after: exactBadge.through, through: exactBadge.through}));
  const refreshedBadge = (await wire.request<ActionablePage[]>('inbox.actionableQuestionPages', {workspaceIds: [workspace.id]}))[0]!;
  assert.deepEqual((await readTail(refreshedBadge, true)).rows.map(row => row.message_id), [newestQuestion, lateQuestion]);
  const legacy = await wire.request<{messageId: string}[]>('inbox.actionableQuestions', {workspaceIds: [workspace.id]});
  assert.ok(!legacy.some(row => row.messageId === newestQuestion));
  const restartedBase = `http://127.0.0.1:${restartedHivePort}`;
  const restartedSession = await fetch(restartedBase + '/api/ui/session');
  const restartedCookie = restartedSession.headers.get('set-cookie')?.split(';', 1)[0];
  await restartedSession.body?.cancel();
  assert.ok(restartedCookie);
  const rawBacklog = await fetch(restartedBase + actionCenter, {headers: {cookie: restartedCookie, origin: restartedBase}});
  assert.ok(rawBacklog.ok);
  const upstreamBacklog: Center = await rawBacklog.json();
  assert.ok(!('inbox_actionable_cursor' in upstreamBacklog));
  assert.ok(!upstreamBacklog.attention.some(row => row.message_id === newestQuestion));
  assert.ok(upstreamBacklog.attention.every(row => !('message_preview' in row)));
  console.log('INBOX_ACTIONABLE_PAGES_OK: initial and next workspace pages, badge isolation, queued classification, stable boundary and 101st question');
  console.log('INBOX_ACTIONABLE_PREVIEWS_OK: exact old question snippets and sender outside the 12-message preview, including bounded Unicode on page two');

  const workflowDb = new DatabaseSync(join(dataDir, 'hive', 'runtime.sqlite'));
  const workflowTime = Date.now() - 500;
  const runName = 'Nightly audit';
  const seededFailures: string[] = [];
  try {
    const insert = workflowDb.prepare(`INSERT INTO workflow_runs
      (id, workspace_id, script_path, name, status, started_at, created_at, finished_at, error, parent_run_id)
      VALUES (?, ?, 'fixture-workflow', ?, ?, ?, ?, ?, ?, ?)`);
    const seedWorkflow = (workspaceId: string, step: number, {name = runName, status = 'failed', settled = true, parent = null, startedStep = step}: {
      name?: string; status?: string; settled?: boolean; parent?: string | null; startedStep?: number;
    } = {}) => {
      const runId = randomUUID();
      const time = workflowTime + step;
      const startedAt = workflowTime + startedStep;
      insert.run(runId, workspaceId, name, status, startedAt, startedAt, settled ? time + 1 : null, `Run error ${step}`, parent);
      return runId;
    };
    for (let step = 0; step < 21; step++) {
      seededFailures.push(seedWorkflow(workspace.id, step, step === 17 ? {name: `${'🐝'.repeat(120)}\nother line`}
        : step === 20 ? {startedStep: -500} : {}));
    }
    seedWorkflow(workspace.id, 21, {parent: seededFailures[20]!});
    seedWorkflow(workspace.id, 22, {status: 'completed'});
    seedWorkflow(workspace.id, 23, {status: 'interrupted'});
    seedWorkflow(workspace.id, 24, {settled: false});
    seedWorkflow(foreign.id, 25);
    const log = workflowDb.prepare('INSERT INTO workflow_run_logs (run_id, ts, message) VALUES (?, ?, ?)');
    log.run(seededFailures[18]!, workflowTime + 18, 'Exact older failure log');
    log.run(seededFailures[20]!, workflowTime + 20, 'Newest same-name failure log');
  } finally { workflowDb.close(); }

  const readFailures = (workspaceId: string) => wire!.request<{
    workspaceId: string; runs: {id: string; name: string | null; finishedAt: number}[]; limited: boolean;
  }>('inbox.workflowFailures', {workspaceId});
  const failures = await readFailures(workspace.id);
  assert.equal(failures.workspaceId, workspace.id);
  assert.equal(failures.limited, true);
  assert.deepEqual(failures.runs.map(run => run.id), seededFailures.slice(1).reverse());
  assert.deepEqual(failures.runs.find(run => run.id === seededFailures[17]), {
    id: seededFailures[17], name: `${'🐝'.repeat(96)}…`, finishedAt: workflowTime + 18,
  });
  assert.deepEqual(failures.runs.filter(run => run.name === runName).slice(0, 2).map(run => run.id),
    [seededFailures[20], seededFailures[19]]);
  const foreignFailures = await readFailures(foreign.id);
  assert.equal(foreignFailures.runs.length, 1);
  assert.equal(foreignFailures.limited, false);
  await assert.rejects(readFailures(randomUUID()), /Unknown Inbox workspace/);
  const exactRun = failures.runs.find(run => run.id === seededFailures[18])!;
  const details = await wire.request<{run: {id: string; workspace_id: string; error: string}}>('hive.request', {
    method: 'GET', path: `/api/workflows/runs/${exactRun.id}`,
  });
  assert.deepEqual([details.run.id, details.run.workspace_id, details.run.error], [exactRun.id, workspace.id, 'Run error 18']);
  const exactLogs = await wire.request<{logs: {message: string}[]}>('hive.request', {
    method: 'GET', path: `/api/workflows/runs/${exactRun.id}/logs`,
  });
  assert.deepEqual(exactLogs.logs.map(log => log.message), ['Exact older failure log']);
  const full = await wire.request<{runs: {id: string}[]}>('hive.request', {
    method: 'GET', path: `/api/workspaces/${workspace.id}/workflows/runs`,
  });
  assert.ok(full.runs.some(run => run.id === seededFailures[0]), 'Older runs remain in Workflows');
  assert.deepEqual(await readFailures(workspace.id), failures, 'Read-only inspection keeps failures intact');
  console.log('INBOX_WORKFLOW_FAILURES_OK: bounded current workspace settled root failures, exact same-name run error and logs, older history preserved');
  const compactWorkflowFailures = (workspaceIds: string[]) => wire!.request<{
    workspaceId: string; runIds: string[];
  }[]>('inbox.workflowFailureBadge', {workspaceIds});
  assert.deepEqual(await compactWorkflowFailures([workspace.id, foreign.id, workspace.id, randomUUID()]), [
    {workspaceId: workspace.id, runIds: seededFailures.slice(1).reverse()},
    {workspaceId: foreign.id, runIds: foreignFailures.runs.map(run => run.id)},
  ]);
  await assert.rejects(compactWorkflowFailures(Array.from({length:33}, () => randomUUID())));
  const thirdPath = join(root, 'without-workflow-failures');
  await mkdir(thirdPath);
  const without = await wire.request<{id: string}>('hive.request', {
    method: 'POST', path: '/api/workspaces', body: {path: thirdPath, name: 'No failures', autostart_orchestrator: false},
  });
  assert.deepEqual(await compactWorkflowFailures([without.id, randomUUID()]), [{workspaceId: without.id, runIds: []}]);
  assert.deepEqual(await readFailures(foreign.id), foreignFailures);
  console.log('INBOX_WORKFLOW_BADGE_OK: compact capped identities for both workspaces, verified empty versus omitted, no writes');
} finally {
  await wire?.close();
  await rm(root, {recursive: true, force: true});
}

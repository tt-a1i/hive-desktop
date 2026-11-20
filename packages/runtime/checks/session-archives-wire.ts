/** Production desktop JSONL, real Hive and isolated Pi configuration; no vendor requests. */
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createInterface } from 'node:readline';

const directory = await mkdtemp(join(tmpdir(), 'hive-archives-wire-'));
const agentDir = join(directory, 'pi');
await mkdir(agentDir);
const settings = JSON.stringify({ defaultProvider: 'openai', defaultModel: 'gpt-6.1-terra' });
await writeFile(join(agentDir, 'settings.json'), settings);
const child = spawn(process.execPath, [process.env.HIVE_DESKTOP_TEST_RUNTIME ?? fileURLToPath(new URL('../dist/main.js', import.meta.url))], {
  cwd: process.cwd(),
  env: {
    ...process.env,
    PI_CODING_AGENT_DIR: agentDir,
    PI_OFFLINE: '1',
    HIVE_DESKTOP_DATA_DIR: join(directory, 'desktop'),
  },
  stdio: ['pipe', 'pipe', 'pipe'],
});
const exited = once(child, 'exit');
let stderr = '';
child.stderr.on('data', (chunk) => {
  stderr += String(chunk);
});
const notifications: any[] = [];
let sequence = 0;
const pending = new Map<number, { resolve(value: any): void; reject(error: Error): void }>();
const lines = createInterface({ input: child.stdout });
lines.on('line', (line) => {
  const response = JSON.parse(line);
  if (response.event === 'agent') notifications.push(response.data);
  const request = pending.get(response.id);
  if (request) {
    pending.delete(response.id);
    request.resolve(response);
  }
});
child.on('exit', () => {
  for (const request of pending.values()) request.reject(new Error('Runtime exited before reply: ' + stderr));
  pending.clear();
});
function rpc(method: string, params = {}) {
  return new Promise<any>((resolve, reject) => {
    const id = ++sequence;
    const timeout = setTimeout(() => {
      pending.delete(id);
      reject(new Error('Runtime archive request timed out: ' + method));
    }, 10_000);
    pending.set(id, {
      resolve: (value) => {
        clearTimeout(timeout);
        resolve(value);
      },
      reject: (error) => {
        clearTimeout(timeout);
        reject(error);
      },
    });
    child.stdin.write(JSON.stringify({ id, method, params }) + '\n');
  });
}
try {
  assert.ok((await rpc('initialize')).result.hivePort);
  const created = await rpc('hive.request', {
    method: 'POST',
    path: '/api/workspaces',
    body: { path: directory, name: '归档协议', autostart_orchestrator: false },
  });
  assert.equal(created.error, undefined);
  const workspaceId = created.result.id;
  const ids: string[] = [];
  for (const backend of ['pi', 'codex', 'pi-rpc', 'claude']) {
    const chat = await rpc('chat.createRecord', { workspaceId, backend });
    assert.equal(chat.error, undefined);
    ids.push(chat.result.id);
    assert.equal(
      (await rpc('chat.pin', { sessionId: chat.result.id, pinned: true })).result.session.pinned,
      true,
    );
    const archived = await rpc('chat.archive', { sessionId: chat.result.id, archived: true });
    assert.equal(archived.error, undefined);
    assert.equal(archived.result.session.archived, true);
    assert.equal(archived.result.session.backend, backend);
    assert.equal(archived.result.session.sessionFile, null);
    assert.ok((await rpc('chat.archive', { sessionId: chat.result.id, archived: 'true' })).error);
  }
  assert.equal((await rpc('chat.search', { query: '' })).result.total, 0);
  const archive = await rpc('chat.search', { query: '', archived: true });
  assert.equal(archive.result.total, 4);
  const navigation = await rpc('chat.search', { query: '', archived: true, navigationOnly: true });
  assert.equal(navigation.error, undefined);
  assert.deepEqual(navigation.result, {
    ...archive.result,
    matches: archive.result.matches.map((hit: any) => {
      const {sessionFile, ...session} = hit.session;
      assert.equal(sessionFile, null);
      return {...hit, session};
    }),
  });
  const reference = {workspaceId, sessionId: ids[1]};
  const resolvedArchive = await rpc('chat.reference.resolve', reference);
  assert.equal(resolvedArchive.error, undefined);
  assert.deepEqual(resolvedArchive.result, navigation.result.matches.find((hit: any) => hit.session.id === ids[1]).session);
  assert.ok(!Object.hasOwn(resolvedArchive.result, 'sessionFile'));
  assert.equal((await rpc('chat.reference.resolve', {...reference, workspaceId: 'other'})).result, null);
  assert.equal((await rpc('chat.reference.resolve', {...reference, sessionId: 'missing'})).result, null);
  assert.ok((await rpc('chat.reference.resolve', {...reference, prompt: 'unexpected'})).error);
  assert.ok((await rpc('chat.search', {query: '', navigationOnly: 'true'})).error);
  assert.equal((await rpc('chat.search', { query: '', archived: true, workspaceId })).result.total, 4);
  assert.equal((await rpc('chat.search', { query: '', archived: true, workspaceId: 'unknown-workspace' })).result.total, 0);
  assert.ok((await rpc('chat.search', { query: '', workspaceId: null })).error);
  assert.ok((await rpc('chat.search', { query: '', workspaceId: '' })).error);
  assert.deepEqual(new Set(archive.result.matches.map((hit: any) => hit.session.id)), new Set(ids));
  assert.equal((await rpc('chat.search', { query: '', archived: true, offset: 1 })).result.matches.length, 3);
  assert.ok((await rpc('chat.search', { query: '', archived: true, offset: -1 })).error);
  assert.deepEqual((await rpc('chat.pinned')).result, []);
  const restored = await rpc('chat.archive', { sessionId: ids[0], archived: false });
  assert.equal(restored.result.session.archived, false);
  assert.equal((await rpc('chat.search', { query: '' })).result.total, 1);
  assert.equal((await rpc('chat.pinned')).result[0].session.id, ids[0]);
  const indexFile = join(directory, 'desktop/chats/index.json');
  const beforeRepeat = await readFile(indexFile, 'utf8');
  assert.equal((await rpc('chat.archive', { sessionId: ids[0], archived: false })).result.session.archived, false);
  assert.equal(await readFile(indexFile, 'utf8'), beforeRepeat);
  assert.deepEqual(
    notifications.filter((event) => event.sessionId === ids[0]).map((event) => ({
      sequence: event.sequence, archived: event.archived, pinned: event.pinned,
    })),
    [
      { sequence: 1, archived: true, pinned: true },
      { sequence: 2, archived: false, pinned: true },
      { sequence: 3, archived: false, pinned: true },
    ],
  );
  assert.ok(notifications.every((event) => event.type === 'session_navigation'));
  const stored = JSON.parse(await readFile(join(directory, 'desktop/chats/index.json'), 'utf8'));
  assert.equal(stored.filter((chat: any) => chat.archived).length, 3);
  assert.ok(stored.every((chat: any) => chat.sessionFile === null));

  const otherPath = join(directory, 'other-project');
  await mkdir(otherPath);
  const other = await rpc('hive.request', {
    method: 'POST', path: '/api/workspaces',
    body: { path: otherPath, name: 'Other project', autostart_orchestrator: false },
  });
  assert.equal(other.error, undefined);
  const selectedIds = new Set<string>();
  for (const [project, count] of [[workspaceId, 101], [other.result.id, 105]] as const) {
    for (let i = 0; i < count; i++) {
      const createdChat = await rpc('chat.createRecord', { workspaceId: project });
      assert.equal(createdChat.error, undefined);
      const sessionId = createdChat.result.id as string;
      assert.equal((await rpc('chat.rename', { sessionId, title: 'Wire pagination ' + i })).error, undefined);
      if (project === workspaceId) selectedIds.add(sessionId);
    }
  }
  const legacy = await rpc('chat.search', { query: 'Wire pagination' });
  assert.equal(legacy.error, undefined);
  assert.equal(legacy.result.total, 206);
  assert.equal(legacy.result.matches.length, 100);
  const first = await rpc('chat.search', { query: 'Wire pagination', workspaceId, archived: false, offset: 0 });
  const second = await rpc('chat.search', { query: 'Wire pagination', workspaceId, offset: 100 });
  for (const page of [first, second]) {
    assert.equal(page.error, undefined);
    assert.equal(page.result.total, 101);
    assert.ok(page.result.matches.every((hit: any) => hit.session.workspaceId === workspaceId));
  }
  assert.equal(first.result.matches.length, 100);
  assert.equal(second.result.matches.length, 1);
  const pageIds = [...first.result.matches, ...second.result.matches].map((hit: any) => hit.session.id);
  assert.equal(new Set(pageIds).size, 101);
  assert.deepEqual(new Set(pageIds), selectedIds);
  const sameTitle = await rpc('chat.search', {query: 'Wire pagination 100', navigationOnly: true});
  const similarTitles = sameTitle.result.matches.filter((hit: any) => hit.session.title === 'Wire pagination 100');
  assert.equal(similarTitles.length, 2);
  const selected = similarTitles.find((hit: any) => hit.session.workspaceId === other.result.id).session;
  const selectedReference = {workspaceId: selected.workspaceId, sessionId: selected.id};
  const indexBeforeReference = await readFile(indexFile, 'utf8');
  const exactSession = await rpc('chat.reference.resolve', selectedReference);
  assert.equal(exactSession.result.id, selected.id);
  assert.equal(exactSession.result.workspaceId, other.result.id);
  assert.equal((await rpc('chat.reference.resolve', {...selectedReference, workspaceId})).result, null);
  assert.equal(await readFile(indexFile, 'utf8'), indexBeforeReference);
  assert.equal((await rpc('chat.rename', {sessionId: selected.id, title: 'Exact reference renamed'})).error, undefined);
  assert.equal((await rpc('chat.reference.resolve', selectedReference)).result.title, 'Exact reference renamed');

  const matching = similarTitles.find((hit: any) => hit.session.workspaceId === workspaceId).session;
  await rpc('chat.rename', {sessionId: matching.id, title: 'Exact reference renamed'});
  await rpc('chat.pin', {sessionId: selected.id, pinned: true});
  await rpc('chat.pin', {sessionId: matching.id, pinned: true});
  const previousOrder = (await rpc('chat.pinned')).result.map((hit: any) => hit.session.id);
  const inbox = {workspaceId: other.result.id, replyRevision: 0};
  const beforeWrongArchive = await readFile(indexFile, 'utf8');
  assert.match((await rpc('chat.archive', {sessionId: selected.id, archived: true,
    inbox: {...inbox, workspaceId}})).error.message, /ARCHIVE_INBOX_STALE/);
  assert.match((await rpc('chat.archive', {sessionId: matching.id, archived: true, inbox})).error.message, /ARCHIVE_INBOX_STALE/);
  assert.equal(await readFile(indexFile, 'utf8'), beforeWrongArchive);
  const firstArchive = await rpc('chat.archive', {sessionId: selected.id, archived: true, inbox});
  const receipt = {workspaceId: other.result.id, version: firstArchive.result.mutation.version};
  assert.equal(firstArchive.result.mutation.eventSequence, notifications.findLast(e => e.sessionId === selected.id).sequence);
  assert.equal((await rpc('chat.archive', {sessionId: selected.id, archived: true})).result.mutation, undefined);
  assert.match((await rpc('chat.archive', {sessionId: selected.id, archived: true, inbox})).error.message, /ARCHIVE_INBOX_STALE/);
  assert.match((await rpc('chat.archive', {sessionId: selected.id, archived: false, undo: {...receipt, workspaceId}})).error.message, /ARCHIVE_UNDO_STALE/);
  assert.match((await rpc('chat.archive', {sessionId: matching.id, archived: false, undo: receipt})).error.message, /ARCHIVE_UNDO_STALE/);
  const undone = await rpc('chat.archive', {sessionId: selected.id, archived: false, undo: receipt});
  assert.equal(undone.result.session.archived, false);
  assert.equal(undone.result.session.pinned, true);
  assert.equal(undone.result.session.updatedAt, selected.updatedAt);
  assert.deepEqual((await rpc('chat.pinned')).result.map((hit: any) => hit.session.id), previousOrder);
  const nextArchive = await rpc('chat.archive', {sessionId: selected.id, archived: true});
  assert.ok(nextArchive.result.mutation.version > receipt.version);
  assert.match((await rpc('chat.archive', {sessionId: selected.id, archived: false, undo: receipt})).error.message, /ARCHIVE_UNDO_STALE/);
  assert.equal((await rpc('chat.reference.resolve', selectedReference)).result.archived, true);
  const unaffected = (await rpc('chat.reference.resolve', {sessionId: matching.id, workspaceId})).result;
  assert.equal(unaffected.id, matching.id);
  assert.equal(Boolean(unaffected.archived), false);
  const query = {query: '', archived: true, navigationOnly: true, limit: 2, offset: 0};
  const completed = (await rpc('chat.search', query)).result;
  assert.equal(completed.matches.length, 2);
  assert.match(completed.archiveSnapshot, /^[a-f0-9]{64}$/);
  const nextCompleted = (await rpc('chat.search', {...query, offset: 2, archiveSnapshot: completed.archiveSnapshot})).result;
  assert.equal(nextCompleted.archiveSnapshot, completed.archiveSnapshot);
  assert.ok(nextCompleted.matches.every((hit: any) =>
    !completed.matches.some((old: any) => old.session.id === hit.session.id)));
  assert.ok(!completed.matches.some((hit: any) => hit.session.id === ids[1]));
  const codexPage = (await rpc('chat.search', {...query, backend: 'codex', workspaceId, limit: 1})).result;
  assert.equal(codexPage.total, 1);
  assert.deepEqual(codexPage.matches.map((hit: any) => [hit.session.id, hit.session.backend]), [[ids[1], 'codex']]);
  assert.equal((await rpc('chat.search', {...query, backend: 'codex', workspaceId: other.result.id})).result.total, 0);
  const externalPi = (await rpc('chat.search', {...query, backend: 'pi-rpc'})).result;
  assert.deepEqual(externalPi.matches.map((hit: any) => hit.session.id), [ids[2]]);
  assert.ok((await rpc('chat.search', {...query, backend: 'external-pi'})).error);
  assert.ok((await rpc('chat.search', {...query, backend: 'pi', navigationOnly: false})).error);
  assert.ok((await rpc('chat.search', {...query, memberId: 'reviewer'})).error);
  assert.ok((await rpc('chat.search', {...query, limit: 101})).error);
  assert.ok((await rpc('chat.search', {...query, query: 'search transcript'})).error);
  assert.match((await rpc('chat.archive', {sessionId: selected.id, archived: false,
    restoreWorkspaceId: workspaceId})).error.message, /ARCHIVE_INBOX_STALE/);
  const restore = await rpc('chat.archive', {sessionId: selected.id, archived: false,
    restoreWorkspaceId: other.result.id});
  assert.equal(restore.result.session.id, selected.id);
  assert.equal(restore.result.session.workspaceId, other.result.id);
  assert.equal(restore.result.session.archived, false);
  const oldArchiveOrder = (await rpc('chat.search', {...query, offset: 2, archiveSnapshot: completed.archiveSnapshot})).result;
  assert.equal(oldArchiveOrder.archiveSnapshotChanged, true);
  assert.deepEqual(oldArchiveOrder.matches, []);
  const refreshedPrefix = (await rpc('chat.search', {...query, limit: 3})).result;
  assert.equal(refreshedPrefix.matches.length, 3);
  assert.ok(refreshedPrefix.matches.every((hit: any) => hit.session.id !== selected.id));
  assert.equal(refreshedPrefix.total, 3);
  assert.notEqual(refreshedPrefix.archiveSnapshot, completed.archiveSnapshot);
  assert.ok((await rpc('chat.search', {...query, archiveSnapshot: 'invalid'})).error);
  // A prefix deeper than the wire limit can be reconstructed in chunks only
  // while the same scoped archive order is authoritative.
  for (const sessionId of selectedIds)
    assert.equal((await rpc('chat.archive', {sessionId, archived: true})).error, undefined);
  const deepQuery = {...query, workspaceId, limit: 100};
  const firstChunk = (await rpc('chat.search', deepQuery)).result;
  assert.equal(firstChunk.matches.length, 100);
  const remaining = {...deepQuery, offset: 100, archiveSnapshot: firstChunk.archiveSnapshot};
  const lastChunk = (await rpc('chat.search', remaining)).result;
  const deepRows = [...firstChunk.matches, ...lastChunk.matches];
  assert.equal(firstChunk.total, selectedIds.size + 3);
  assert.equal(new Set(deepRows.map((hit: any) => hit.session.id)).size, firstChunk.total);
  assert.equal((await rpc('chat.archive', {sessionId: firstChunk.matches[26].session.id, archived: false,
    restoreWorkspaceId: workspaceId})).error, undefined);
  const movedBoundary = (await rpc('chat.search', remaining)).result;
  assert.equal(movedBoundary.archiveSnapshotChanged, true);
  assert.deepEqual(movedBoundary.matches, []);
  assert.ok(notifications.every((event) => event.type === 'session_navigation'));
  assert.equal((await rpc('shutdown')).result.ok, true);
  await exited;
  assert.equal(child.exitCode, 0);
  console.log(
    'SESSION_ARCHIVES_WIRE_OK: four-backend archive/restore, guarded undo with precise pinned order, exact references, scoped pagination, ordered metadata notifications and clean shutdown. No Agent/model initialization.',
  );
} finally {
  if (child.exitCode === null && child.signalCode === null) {
    child.kill('SIGKILL');
    await exited;
  }
  lines.close();
  await rm(directory, { recursive: true, force: true });
}

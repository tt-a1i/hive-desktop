/** Production desktop JSONL, real Hive and isolated Pi configuration; no vendor requests. */
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createInterface } from 'node:readline';

const directory = await mkdtemp(join(tmpdir(), 'hive-archives-wire-'));
const agentDir = join(directory, 'pi');
await mkdir(agentDir);
const settings = JSON.stringify({ defaultProvider: 'openai', defaultModel: 'gpt-6.1-terra' });
await writeFile(join(agentDir, 'settings.json'), settings);
const child = spawn(process.execPath, ['dist/main.js'], {
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
  assert.ok(notifications.every((event) => event.type === 'session_navigation'));
  assert.equal((await rpc('shutdown')).result.ok, true);
  await exited;
  assert.equal(child.exitCode, 0);
  console.log(
    'SESSION_ARCHIVES_WIRE_OK: production JSONL four-backend archive/restore, scoped cross-project pagination, persisted identity, ordered metadata notifications and clean shutdown. No Agent/model initialization.',
  );
} finally {
  if (child.exitCode === null && child.signalCode === null) {
    child.kill('SIGKILL');
    await exited;
  }
  lines.close();
  await rm(directory, { recursive: true, force: true });
}

/** Real runtime, HTTP/SQLite workspace and durable chat index. No Agent or model starts. */
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, readFile, rename, rm, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { createDesktopRuntime } from '../src/desktop-runtime.js';
import type { ChatBackend } from '../src/chat-index.js';

const directory = await mkdtemp(join(tmpdir(), 'hive-session-titles-'));
const state = join(directory, 'state');
let runtime: Awaited<ReturnType<typeof createDesktopRuntime>> | undefined;
try {
  runtime = await createDesktopRuntime(state, () => {});
  const workspace = await runtime.hive.request('POST', '/api/workspaces', {
    path: directory, name: 'Session names', autostart_orchestrator: false,
  }) as { id: string };
  for (const backend of ['pi', 'codex', 'pi-rpc', 'claude'] satisfies ChatBackend[]) {
    const original = await runtime.chats.createRecord(workspace.id, backend);
    const renamed = runtime.chats.rename(original.id, '  发布检查 🐝  ');
    assert.deepEqual(renamed, { ...original, title: '发布检查 🐝', customTitle: true });
    for (const invalid of [null, 2, undefined, '', ' \n ', 'a\nb', 'a\0b', 'a\u007fb', 'x'.repeat(121)]) {
      assert.throws(() => runtime!.chats.rename(original.id, invalid));
      assert.deepEqual(runtime.chats.list(workspace.id).find(chat => chat.id === original.id), renamed);
    }
    const unicode = '🐝'.repeat(120);
    assert.equal(runtime.chats.rename(original.id, unicode).title, unicode);
    runtime.chats.rename(original.id, backend + ' 会话');
  }
  assert.throws(() => runtime!.chats.rename('missing', 'title'));
  const records = runtime.chats.list(workspace.id);
  const file = join(state, 'chats/index.json');
  await rename(file, file + '.retained');
  await mkdir(file);
  try {
    assert.throws(() => runtime!.chats.rename(records[0].id, 'Must not be stored'));
    assert.deepEqual(runtime.chats.list(workspace.id), records, 'Failed disk commit must not change memory');
  } finally {
    await rm(file, { recursive: true });
    await rename(file + '.retained', file);
  }
  await runtime.close();
  runtime = undefined;
  // A pre-feature record without customTitle must remain readable.
  const stored = JSON.parse(await readFile(file, 'utf8'));
  const legacyId = stored[0].id;
  delete stored[0].customTitle;
  await writeFile(file, JSON.stringify(stored));
  runtime = await createDesktopRuntime(state, () => {});
  assert.deepEqual(runtime.chats.list(workspace.id), stored.sort((a: { updatedAt: number }, b: { updatedAt: number }) => b.updatedAt - a.updatedAt));
  for (const item of records.filter(item => item.id !== legacyId)) {
    assert.equal(runtime.chats.list(workspace.id).find(chat => chat.id === item.id)?.customTitle, true);
  }
  console.log('SESSION_TITLES_OK: four backends, validation, disk rollback and restart. No model request.');
} finally {
  await runtime?.close();
  await rm(directory, { recursive: true, force: true });
}

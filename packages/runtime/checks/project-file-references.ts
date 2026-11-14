import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdtemp, mkdir, readFile, rename, rm, symlink, unlink, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createDesktopRuntime } from '../src/desktop-runtime.js';

const dir = await mkdtemp(join(tmpdir(), 'hive-project-reference-'));
let runtime: Awaited<ReturnType<typeof createDesktopRuntime>> | undefined;
try {
  const project = join(dir, '项目 spaces'); const other = join(dir, 'other');
  await mkdir(project); await mkdir(other);
  const name = '需求 中文 🐝.md'; const source = join(project, name);
  const bytes = Buffer.from('原始内容\n'); await writeFile(source, bytes);
  const data = join(dir, 'desktop');
  runtime = await createDesktopRuntime(data, () => {});
  const ws = await runtime.hive.request('POST', '/api/workspaces', { path: project, name: 'Project', autostart_orchestrator: false }) as { id: string };
  const ws2 = await runtime.hive.request('POST', '/api/workspaces', { path: other, name: 'Other', autostart_orchestrator: false }) as { id: string };
  assert.deepEqual((await runtime.chats.projectFilesSearch(ws.id, '中文')).paths, [name]);
  const staged = await runtime.chats.projectFilesAttach(ws.id, name);
  const expected = { relativePath: name, sha256: createHash('sha256').update(bytes).digest('hex') };
  assert.deepEqual(staged.attachment.projectFile, expected);
  assert.deepEqual(await readFile(staged.filePath!), bytes);
  assert.deepEqual(await runtime.chats.projectFileStatus(ws.id, staged.attachment.id), { state: 'unchanged' });
  await assert.rejects(runtime.chats.projectFilesAttach(ws.id, '../other/file'));
  await assert.rejects(runtime.chats.attachments.get(ws2.id, [staged.attachment.id]));
  if (process.platform !== 'win32') {
    await writeFile(join(other, 'outside'), 'secret'); await symlink(join(other, 'outside'), join(project, 'escape'));
    await assert.rejects(runtime.chats.projectFilesAttach(ws.id, 'escape'));
  }
  const registry = join(data, 'chats', 'project-file-references');
  const backup = `${registry}.saved`;
  await rename(registry, backup);
  await writeFile(registry, 'block directory');
  try { await assert.rejects(runtime.chats.projectFilesAttach(ws.id, name)); }
  finally { await unlink(registry); await rename(backup, registry); }
  assert.deepEqual((await runtime.chats.attachments.get(ws.id, [staged.attachment.id]))[0]?.attachment.projectFile, expected);
  await writeFile(source, Buffer.from([0, 1]));
  assert.deepEqual(await runtime.chats.projectFileStatus(ws.id, staged.attachment.id), { state: 'changed' });
  await writeFile(source, 'changed');
  assert.deepEqual(await runtime.chats.projectFileStatus(ws.id, staged.attachment.id), { state: 'changed' });
  await runtime.close(); runtime = await createDesktopRuntime(data, () => {});
  const recovered = (await runtime.chats.attachments.get(ws.id, [staged.attachment.id]))[0]!;
  assert.deepEqual(recovered.attachment.projectFile, expected);
  assert.deepEqual(await readFile(recovered.filePath!), bytes);
  await unlink(source);
  assert.deepEqual(await runtime.chats.projectFileStatus(ws.id, staged.attachment.id), { state: 'missing' });
  assert.deepEqual(await readFile(recovered.filePath!), bytes);
  console.log('PROJECT_FILE_REFERENCES_OK');
} finally { await runtime?.close(); await rm(dir, { recursive: true, force: true }); }

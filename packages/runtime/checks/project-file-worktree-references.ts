import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdtemp, mkdir, readFile, realpath, rm, symlink, unlink, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createDesktopRuntime } from '../src/desktop-runtime.js';
const dir = await mkdtemp(join(tmpdir(), 'hive-ref-worktrees-'));
let runtime: Awaited<ReturnType<typeof createDesktopRuntime>> | undefined;
const git = (cwd: string, ...args: string[]) => execFileSync('git', ['-C', cwd, ...args], { stdio: 'pipe' });
try {
  const project = join(dir, 'project'); const other = join(dir, 'other'); const data = join(dir, 'desktop');
  await mkdir(project); await mkdir(other);
  git(project, 'init', '-q'); git(project, 'config', 'user.name', 'Fixture'); git(project, 'config', 'user.email', 'fixture@example.invalid');
  const name = '同名 file.txt'; await writeFile(join(project, name), 'main'); git(project, 'add', '.'); git(project, 'commit', '-qm', 'base');
  runtime = await createDesktopRuntime(data, () => {});
  const ws = await runtime.hive.request('POST', '/api/workspaces', { path: project, name: 'Project', autostart_orchestrator: false }) as { id: string };
  const otherWs = await runtime.hive.request('POST', '/api/workspaces', { path: other, name: 'Other', autostart_orchestrator: false }) as { id: string };
  const a = await runtime.chats.createRecord(ws.id, 'pi', {isolation: 'worktree'});
  const b = await runtime.chats.createRecord(ws.id, 'codex', {isolation: 'worktree'});
  const aDir = (await runtime.chats.sessionExecution(a.id)).path; const bDir = (await runtime.chats.sessionExecution(b.id)).path;
  const unicodePath = 'Re\u0301sume\u0301 工作树.txt';
  await writeFile(join(aDir, unicodePath), 'Unicode path in the first worktree');
  const [visiblePath] = (await runtime.chats.projectFilesSearch(ws.id, '工作树.txt', 0, a.id)).paths;
  assert(visiblePath);
  assert.equal(visiblePath.normalize('NFC'), unicodePath.normalize('NFC'));
  for (const query of ['RÉSUMÉ', 'RE\u0301SUME\u0301']) {
    assert.deepEqual((await runtime.chats.projectFilesSearch(ws.id, query, 0, a.id)).paths, [visiblePath]);
    assert.deepEqual((await runtime.chats.projectFilesSearch(ws.id, query, 0, b.id)).paths, []);
  }
  assert.deepEqual((await runtime.chats.projectFilesSearch(ws.id, 'Résumé', 0, a.id, undefined, true)).paths, [visiblePath]);
  assert.deepEqual((await runtime.chats.projectFilesSearch(ws.id, 'RÉSUMÉ', 0, a.id, undefined, true)).paths, []);
  await assert.rejects(runtime.chats.projectFilesSearch(ws.id, 'Résumé', 0, a.id, undefined, 'true'), /Invalid project file search/);
  const unicodeRef = await runtime.chats.projectFilesAttach(ws.id, visiblePath, a.id);
  assert.equal(unicodeRef.attachment.projectFile?.relativePath, visiblePath);
  assert.equal(unicodeRef.attachment.projectFile?.sourceSessionId, a.id);
  assert.equal((await readFile(unicodeRef.filePath!)).toString(), 'Unicode path in the first worktree');
  await writeFile(join(aDir, name), 'one'); await writeFile(join(bDir, name), 'two');
  await writeFile(join(aDir, 'only-one.txt'), 'x'); await writeFile(join(bDir, 'only-two.txt'), 'x');
  const mainPreview = await runtime.chats.projectFilesPreview(ws.id, name);
  const aPreview = await runtime.chats.projectFilesPreview(ws.id, name, a.id);
  const bPreview = await runtime.chats.projectFilesPreview(ws.id, name, b.id);
  assert.deepEqual([mainPreview.text, aPreview.text, bPreview.text], ['main', 'one', 'two']);
  assert.equal(new Set([mainPreview.sha256, aPreview.sha256, bPreview.sha256]).size, 3);
  for (const [sessionId, directory, expected] of [[undefined, project, 'main'], [a.id, aDir, 'one'], [b.id, bDir, 'two']] as const) {
    const file = await runtime.chats.projectFilesResolveOpen(ws.id, name, sessionId);
    assert.deepEqual(file, {root: await realpath(directory), path: await realpath(join(directory, name))});
    assert.equal(await readFile(file.path, 'utf8'), expected);
  }
  const punctuation = '文件 with spaces & ; %.txt';
  await writeFile(join(aDir, punctuation), 'literal editor path');
  assert.equal((await runtime.chats.projectFilesResolveOpen(ws.id, punctuation, a.id)).path, await realpath(join(aDir, punctuation)));
  await assert.rejects(runtime.chats.projectFilesResolveOpen(otherWs.id, name, a.id));
  for (const path of ['../' + name, 'missing.txt', 'other\\file', '.git']) {
    await assert.rejects(runtime.chats.projectFilesResolveOpen(ws.id, path, a.id));
  }
  if (process.platform !== 'win32') {
    await symlink(join(aDir, name), join(aDir, 'same-root-link.txt'));
    await symlink(join(bDir, name), join(aDir, 'other-root-link.txt'));
    await assert.rejects(runtime.chats.projectFilesResolveOpen(ws.id, 'same-root-link.txt', a.id));
    await assert.rejects(runtime.chats.projectFilesResolveOpen(ws.id, 'other-root-link.txt', a.id));
  }
  assert((await runtime.chats.projectFilesList(ws.id, '', 0, a.id)).entries.some(entry => entry.path === 'only-one.txt'));
  assert(!(await runtime.chats.projectFilesList(ws.id, '', 0, b.id)).entries.some(entry => entry.path === 'only-one.txt'));
  assert.deepEqual((await runtime.chats.projectFilesSearchContent(ws.id, 'one', 0, a.id)).matches.map(({path, line}) => ({path, line})), [{path: name, line: 1}]);
  assert.deepEqual((await runtime.chats.projectFilesSearchContent(ws.id, 'one', 0, b.id)).matches, []);
  const contentName = '同名 search.md';
  const token = 'MULTI_SCOPE_HIT';
  await writeFile(join(project, contentName), `${token} main\n`);
  await writeFile(join(aDir, contentName), `\n${token} alpha 2\n\n${token.toLowerCase()} alpha 4`);
  await writeFile(join(bDir, contentName), `${token} beta 1\n\n${token} beta 3\n`);
  for (const [sessionId, lines] of [[undefined, [1]], [a.id, [2, 4]], [b.id, [1, 3]]] as const) {
    const result = await runtime.chats.projectFilesSearchContent(ws.id, token, 0, sessionId);
    assert.deepEqual(result.matches.map(({path, line}) => ({path, line})), lines.map(line => ({path: contentName, line})));
    assert.equal(result.total, lines.length);
    assert(!result.limited);
    const preview = await runtime.chats.projectFilesPreview(ws.id, contentName, sessionId);
    for (const hit of result.matches) {
      const original = preview.text.split('\n')[hit.line - 1];
      assert(original?.toUpperCase().startsWith(token));
      assert.equal(hit.snippet?.text, original);
      assert.deepEqual(hit.snippet?.ranges, [[0, token.length]]);
    }
  }
  assert.deepEqual((await runtime.chats.projectFilesSearchContent(ws.id, token, 0, a.id, undefined, true)).matches.map(({path, line}) => ({path, line})), [{path: contentName, line: 2}]);
  assert.deepEqual((await runtime.chats.projectFilesSearchContent(ws.id, token, 0, b.id, undefined, true)).matches.map(({path, line}) => ({path, line})), [{path: contentName, line: 1}, {path: contentName, line: 3}]);
  await writeFile(join(aDir, contentName), `${token}_suffix\n${token} alpha\n`);
  assert.deepEqual((await runtime.chats.projectFilesSearchContent(ws.id, token, 0, a.id, undefined, true, true)).matches.map(({line}) => line), [2]);
  assert.deepEqual((await runtime.chats.projectFilesSearchContent(ws.id, token, 0, b.id, undefined, true, true)).matches.map(({line}) => line), [1, 3]);
  await assert.rejects(runtime.chats.projectFilesSearchContent(ws.id, token, 0, a.id, undefined, false, 'true'), /Invalid project file search/);
  await assert.rejects(runtime.chats.projectFilesSearchContent(ws.id, token, 0, a.id, undefined, 'true'), /Invalid project file search/);
  const expression = `^${token} (?:alpha|beta [0-9])$`;
  for (const [sessionId, lines] of [[undefined, []], [a.id, [2]], [b.id, [1, 3]]] as const) {
    const result = await runtime.chats.projectFilesSearchContent(ws.id, expression, 0, sessionId, undefined, false, false, true);
    assert.deepEqual(result.matches.map(({path, line}) => ({path, line})), lines.map(line => ({path: contentName, line})));
  }
  await assert.rejects(runtime.chats.projectFilesSearchContent(ws.id, token, 0, a.id, undefined, false, false, 'true'), /Invalid project file search/);
  await assert.rejects(runtime.chats.projectFilesSearchContent(otherWs.id, expression, 0, a.id, undefined, false, false, true));
  await assert.rejects(runtime.chats.projectFilesSearchContent(otherWs.id, token, 0, a.id));
  await assert.rejects(runtime.chats.projectFilesPreview(otherWs.id, name, a.id));
  assert.deepEqual((await runtime.chats.projectFilesSearch(ws.id, 'only-', 0, a.id)).paths, ['only-one.txt']);
  assert.deepEqual((await runtime.chats.projectFilesSearch(ws.id, 'only-', 0, b.id)).paths, ['only-two.txt']);
  assert.deepEqual((await runtime.chats.projectFilesSearch(ws.id, 'only-')).paths, []);
  const main = await runtime.chats.projectFilesAttach(ws.id, name);
  await assert.rejects(runtime.chats.projectFilesAttach(ws.id, name, a.id, bPreview.sha256), /文件在预览后已更改/);
  const aRef = await runtime.chats.projectFilesAttach(ws.id, name, a.id, aPreview.sha256);
  const bRef = await runtime.chats.projectFilesAttach(ws.id, name, b.id);
  assert.equal(main.attachment.projectFile?.sourceSessionId, undefined);
  assert.equal(aRef.attachment.projectFile?.sourceSessionId, a.id);
  assert.equal(aRef.attachment.projectFile?.sha256, aPreview.sha256);
  assert.equal(bRef.attachment.projectFile?.sourceSessionId, b.id);
  assert.equal((await readFile(main.filePath!)).toString(), 'main');
  assert.equal((await readFile(aRef.filePath!)).toString(), 'one');
  assert.equal((await readFile(bRef.filePath!)).toString(), 'two');
  await assert.rejects(runtime.chats.projectFilesAttach(otherWs.id, name, a.id));
  await assert.rejects(runtime.chats.attachments.get(otherWs.id, [aRef.attachment.id]));
  await writeFile(join(aDir, name), 'changed'); await unlink(join(bDir, name));
  await assert.rejects(runtime.chats.projectFilesAttach(ws.id, name, a.id, aPreview.sha256), /文件在预览后已更改/);
  const refreshed = await runtime.chats.projectFilesPreview(ws.id, name, a.id);
  const changedRef = await runtime.chats.projectFilesAttach(ws.id, name, a.id, refreshed.sha256);
  assert.equal(await readFile(changedRef.filePath!, 'utf8'), 'changed');
  await assert.rejects(runtime.chats.projectFilesResolveOpen(ws.id, name, b.id));
  assert.deepEqual(await runtime.chats.projectFileStatus(ws.id, aRef.attachment.id), {state: 'changed'});
  assert.deepEqual(await runtime.chats.projectFileStatus(ws.id, bRef.attachment.id), {state: 'missing'});
  assert.deepEqual(await runtime.chats.projectFileStatus(ws.id, main.attachment.id), {state: 'unchanged'});
  await runtime.close(); runtime = await createDesktopRuntime(data, () => {});
  const recovered = await runtime.chats.attachments.get(ws.id, [aRef.attachment.id, bRef.attachment.id]);
  assert.equal(recovered[0]?.attachment.projectFile?.sourceSessionId, a.id);
  assert.equal(recovered[1]?.attachment.projectFile?.sourceSessionId, b.id);
  assert.equal((await readFile(recovered[0]!.filePath!)).toString(), 'one');
  assert.equal((await readFile(recovered[1]!.filePath!)).toString(), 'two');
  assert.deepEqual(await runtime.chats.projectFileStatus(ws.id, aRef.attachment.id), {state: 'changed'});
  assert.deepEqual(await runtime.chats.projectFileStatus(ws.id, bRef.attachment.id), {state: 'missing'});
  await rm(aDir, { recursive: true, force: true });
  await assert.rejects(runtime.chats.projectFileStatus(ws.id, aRef.attachment.id));
  assert.equal((await readFile((await runtime.chats.attachments.get(ws.id, [aRef.attachment.id]))[0]!.filePath!)).toString(), 'one');
  console.log('PROJECT_FILE_WORKTREE_REFERENCES_OK');
} finally { await runtime?.close(); await rm(dir, { recursive: true, force: true }); }

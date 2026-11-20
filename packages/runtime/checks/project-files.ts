import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdtemp, mkdir, realpath, rm, symlink, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { listProjectDirectory, previewProjectFile, searchProjectContents, searchProjectFiles, resolveProjectFile, readProjectFile } from '../src/project-files.js';

const dir = await mkdtemp(join(tmpdir(), 'hive project files '));
try {
  const repo = join(dir, 'repository'); await mkdir(repo);
  execFileSync('git', ['init', '-q', repo]);
  await writeFile(join(repo, '.gitignore'), 'ignored*\n');
  await writeFile(join(repo, 'ignored.txt'), 'private unlisted phrase');
  await writeFile(join(repo, 'tracked ignored.txt'), 'tracked');
  execFileSync('git', ['-C', repo, 'add', 'tracked ignored.txt']);
  const nested = join(repo, 'nested'); await mkdir(nested);
  await writeFile(join(nested, '中文 space.txt'), 'hello');
  for (let i = 0; i < 105; i++) await writeFile(join(nested, `file-${String(i).padStart(3, '0')}.txt`), 'x');
  const replacedDirectory = join(repo, 'directory-replacement');
  await mkdir(replacedDirectory);
  await writeFile(join(replacedDirectory, 'tracked.txt'), 'old');
  execFileSync('git', ['-C', repo, 'add', 'directory-replacement/tracked.txt']);
  await rm(replacedDirectory, { recursive: true });
  await writeFile(replacedDirectory, 'new');
  const all = await searchProjectFiles(repo, '');
  assert(!all.paths.includes('ignored.txt'));
  assert(all.paths.includes('directory-replacement'));
  assert(all.paths.includes('nested/file-000.txt'));
  assert.deepEqual((await searchProjectFiles(repo, 'directory-replacement')).paths, ['directory-replacement']);
  assert((await searchProjectFiles(repo, 'tracked ignored')).paths.includes('tracked ignored.txt'));
  await mkdir(join(repo, 'empty'));
  const rootEntries = (await listProjectDirectory(repo, '')).entries;
  assert.deepEqual(rootEntries.slice(0, 2), [{path: 'empty', directory: true}, {path: 'nested', directory: true}]);
  assert(!rootEntries.some(entry => entry.path === '.git'));
  assert.deepEqual(await listProjectDirectory(repo, 'empty'), {entries: [], total: 0});
  assert.equal((await listProjectDirectory(repo, 'nested', 100)).entries.length, 6);
  const page = await searchProjectFiles(nested, 'file-');
  assert.equal(page.total, 105); assert.equal(page.paths.length, 100);
  assert.deepEqual((await searchProjectFiles(nested, 'file-', 100)).paths, Array.from({length: 5}, (_, i) => `file-${i + 100}.txt`));
  assert.deepEqual((await searchProjectFiles(nested, '中文')).paths, ['中文 space.txt']);
  assert.equal(await resolveProjectFile(nested, '中文 space.txt'), await realpath(join(nested, '中文 space.txt')));
  assert.equal((await readProjectFile(nested, '中文 space.txt')).bytes.toString(), 'hello');
  await writeFile(join(nested, 'binary.bin'), Buffer.from([0, 1]));
  await assert.rejects(readProjectFile(nested, 'binary.bin'));
  await assert.rejects(previewProjectFile(nested, 'binary.bin'), /not text/);
  await writeFile(join(nested, 'unicode.txt'), 'first line\nCafe\u0301 MATCH\n');
  await writeFile(join(nested, 'large.txt'), Buffer.alloc(1024 * 1024 + 1, 'a'));
  await assert.rejects(previewProjectFile(nested, 'large.txt'), /preview limit/);
  assert.deepEqual(await searchProjectContents(repo, 'CAFÉ match'), {
    matches: [{path: 'nested/unicode.txt', line: 2}], total: 1, limited: true,
  });
  const nextContentPage = await searchProjectContents(repo, 'x', 100);
  assert.equal(nextContentPage.total, 105);
  assert.deepEqual(nextContentPage.matches, Array.from({length: 5}, (_, i) => ({
    path: `nested/file-${i + 100}.txt`, line: 1,
  })));
  assert.deepEqual(await previewProjectFile(repo, 'nested/unicode.txt'), {text: 'first line\nCafe\u0301 MATCH\n'});
  assert.equal((await searchProjectContents(repo, 'private unlisted phrase')).total, 0);
  await assert.rejects(searchProjectContents(repo, 'match', 0, AbortSignal.abort()), {name: 'AbortError'});
  await assert.rejects(listProjectDirectory(repo, '../'));
  for (const path of ['../ignored.txt', '/etc/passwd', '.git/config', 'x\0y']) await assert.rejects(resolveProjectFile(repo, path));
  if (process.platform !== 'win32') {
    await symlink(dir, join(nested, 'escape'));
    await assert.rejects(resolveProjectFile(nested, 'escape/repository/.gitignore'));
    await assert.rejects(listProjectDirectory(nested, 'escape'));
    await assert.rejects(previewProjectFile(nested, 'escape/repository/.gitignore'));
    assert(!(await searchProjectFiles(nested, 'escape')).paths.length);
  }
  const ignoredRoot = join(repo, 'ignored-project'); await mkdir(ignoredRoot);
  await writeFile(join(ignoredRoot, 'keyboard-alpha.txt'), 'searchable content');
  assert.deepEqual((await searchProjectFiles(ignoredRoot, 'keyboard')).paths, ['keyboard-alpha.txt']);
  assert.deepEqual((await searchProjectContents(ignoredRoot, 'searchable')).matches, [{path: 'keyboard-alpha.txt', line: 1}]);
  assert(!(await searchProjectFiles(repo, 'keyboard')).paths.length);
  const plain = join(dir, 'plain'); await mkdir(plain); await mkdir(join(plain, 'node_modules'));
  await writeFile(join(plain, 'yes.txt'), 'x'); await writeFile(join(plain, 'node_modules', 'no.txt'), 'x');
  assert.deepEqual((await searchProjectFiles(plain, '')).paths, ['yes.txt']);
  await assert.rejects(searchProjectFiles(join(dir, 'missing'), ''));
  await assert.rejects(searchProjectFiles(repo, '\0'));
  await assert.rejects(searchProjectFiles(repo, '', -1));
  console.log('PROJECT_FILES_OK');
} finally { await rm(dir, { recursive: true, force: true }); }

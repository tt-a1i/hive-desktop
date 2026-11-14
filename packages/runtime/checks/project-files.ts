import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdtemp, mkdir, realpath, rm, symlink, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { searchProjectFiles, resolveProjectFile, readProjectFile } from '../src/project-files.js';

const dir = await mkdtemp(join(tmpdir(), 'hive project files '));
try {
  const repo = join(dir, 'repository'); await mkdir(repo);
  execFileSync('git', ['init', '-q', repo]);
  await writeFile(join(repo, '.gitignore'), 'ignored*\n');
  await writeFile(join(repo, 'ignored.txt'), 'ignored');
  await writeFile(join(repo, 'tracked ignored.txt'), 'tracked');
  execFileSync('git', ['-C', repo, 'add', 'tracked ignored.txt']);
  const nested = join(repo, 'nested'); await mkdir(nested);
  await writeFile(join(nested, '中文 space.txt'), 'hello');
  for (let i = 0; i < 105; i++) await writeFile(join(nested, `file-${String(i).padStart(3, '0')}.txt`), 'x');
  const all = await searchProjectFiles(repo, '');
  assert(!all.paths.includes('ignored.txt'));
  assert((await searchProjectFiles(repo, 'tracked ignored')).paths.includes('tracked ignored.txt'));
  const page = await searchProjectFiles(nested, 'file-');
  assert.equal(page.total, 105); assert.equal(page.paths.length, 100);
  assert.deepEqual((await searchProjectFiles(nested, 'file-', 100)).paths, Array.from({length: 5}, (_, i) => `file-${i + 100}.txt`));
  assert.deepEqual((await searchProjectFiles(nested, '中文')).paths, ['中文 space.txt']);
  assert.equal(await resolveProjectFile(nested, '中文 space.txt'), await realpath(join(nested, '中文 space.txt')));
  assert.equal((await readProjectFile(nested, '中文 space.txt')).bytes.toString(), 'hello');
  await writeFile(join(nested, 'binary.bin'), Buffer.from([0, 1]));
  await assert.rejects(readProjectFile(nested, 'binary.bin')); 
  for (const path of ['../ignored.txt', '/etc/passwd', '.git/config', 'x\0y']) await assert.rejects(resolveProjectFile(repo, path));
  if (process.platform !== 'win32') {
    await symlink(dir, join(nested, 'escape'));
    await assert.rejects(resolveProjectFile(nested, 'escape/repository/.gitignore'));
    assert(!(await searchProjectFiles(nested, 'escape')).paths.length);
  }
  const plain = join(dir, 'plain'); await mkdir(plain); await mkdir(join(plain, 'node_modules'));
  await writeFile(join(plain, 'yes.txt'), 'x'); await writeFile(join(plain, 'node_modules', 'no.txt'), 'x');
  assert.deepEqual((await searchProjectFiles(plain, '')).paths, ['yes.txt']);
  await assert.rejects(searchProjectFiles(join(dir, 'missing'), ''));
  await assert.rejects(searchProjectFiles(repo, '\0'));
  await assert.rejects(searchProjectFiles(repo, '', -1));
  console.log('PROJECT_FILES_OK');
} finally { await rm(dir, { recursive: true, force: true }); }

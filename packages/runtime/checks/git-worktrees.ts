import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { chmod, mkdtemp, mkdir, readFile, rename, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createManagedWorktree, readWorktreeIntent, recoverManagedWorktree, removeManagedWorktree, validateManagedWorktree } from '../src/git-worktrees.js';
const root = await mkdtemp(join(tmpdir(), 'hive worktree '));
function git(cwd: string, ...args: string[]) { return execFileSync('git', ['-C', cwd, ...args], { encoding: 'utf8' }).trim(); }
try {
  const project = join(root, 'project \n'); const data = join(root, 'data'); await mkdir(project);
  git(project, 'init', '-q'); git(project, 'config', 'user.name', 'Fixture'); git(project, 'config', 'user.email', 'fixture@example.invalid');
  await mkdir(join(project, 'sub folder')); await writeFile(join(project, 'sub folder', 'same.txt'), 'main');
  git(project, 'add', '.'); git(project, 'commit', '-qm', 'base');
  const savedEnv = { GIT_DIR: process.env.GIT_DIR, GIT_WORK_TREE: process.env.GIT_WORK_TREE, GIT_INDEX_FILE: process.env.GIT_INDEX_FILE, GIT_CONFIG_COUNT: process.env.GIT_CONFIG_COUNT, GIT_CONFIG_KEY_0: process.env.GIT_CONFIG_KEY_0, GIT_CONFIG_VALUE_0: process.env.GIT_CONFIG_VALUE_0 };
  Object.assign(process.env, { GIT_DIR: join(root, 'wrong'), GIT_WORK_TREE: root, GIT_INDEX_FILE: join(root, 'wrong-index'), GIT_CONFIG_COUNT: '1', GIT_CONFIG_KEY_0: 'core.bare', GIT_CONFIG_VALUE_0: 'true' });
  let first;
  try { first = await createManagedWorktree({ projectRoot: join(project, 'sub folder'), dataRoot: data, chatId: 'first' }); }
  finally { for (const [key, value] of Object.entries(savedEnv)) { if (value === undefined) delete process.env[key]; else process.env[key] = value; } }
  const second = await createManagedWorktree({ projectRoot: join(project, 'sub folder'), dataRoot: data, chatId: 'second' });
  assert.equal(await validateManagedWorktree(data, first), first.executionPath);
  assert(first.executionPath.endsWith('sub folder'));
  const intentPath = join(data, 'worktrees', 'first', 'intent.json');
  const intent = await readWorktreeIntent(data, 'first');
  await writeFile(intentPath, JSON.stringify({ ...intent, state: 'creating' }));
  await assert.rejects(validateManagedWorktree(data, first), /interrupted/);
  assert.deepEqual(await recoverManagedWorktree(data, first), first);
  await assert.rejects(recoverManagedWorktree(data, { ...first, ownerToken: 'wrong' }), /ownership/);
  git(first.managedWorktreeRoot, 'worktree', 'lock', first.managedWorktreeRoot);
  await assert.rejects(removeManagedWorktree(data, first, { active: false }), /locked/);
  git(first.managedWorktreeRoot, 'worktree', 'unlock', first.managedWorktreeRoot);
  await writeFile(join(first.executionPath, 'new.txt'), 'untracked');
  await assert.rejects(removeManagedWorktree(data, first, { active: false }), /uncommitted/);
  await rm(join(first.executionPath, 'new.txt'));
  await writeFile(join(first.executionPath, 'same.txt'), 'first'); await writeFile(join(second.executionPath, 'same.txt'), 'second');
  assert.equal(await readFile(join(project, 'sub folder', 'same.txt'), 'utf8'), 'main');
  assert.equal(await readFile(join(first.executionPath, 'same.txt'), 'utf8'), 'first');
  assert.equal(await readFile(join(second.executionPath, 'same.txt'), 'utf8'), 'second');
  await assert.rejects(removeManagedWorktree(data, first, { active: false }), /uncommitted/);
  await assert.rejects(removeManagedWorktree(data, second, { active: true }), /in use/);
  await assert.rejects(createManagedWorktree({ projectRoot: project, dataRoot: data, chatId: 'first' }));
  await assert.rejects(createManagedWorktree({ projectRoot: project, dataRoot: data, chatId: 'conflict', branch: first.branch }), (e: any) => e.code === 'BRANCH_OCCUPIED');
  assert.equal((await readWorktreeIntent(data, 'conflict')).state, 'creating');
  const empty = join(root, 'empty'); await mkdir(empty); git(empty, 'init', '-q');
  await assert.rejects(createManagedWorktree({ projectRoot: empty, dataRoot: data, chatId: 'unborn' }), (e: any) => e.code === 'UNBORN_OR_MISSING_HEAD');
  const plain = join(root, 'plain'); await mkdir(plain);
  await assert.rejects(createManagedWorktree({ projectRoot: plain, dataRoot: data, chatId: 'nongit' }), (e: any) => e.code === 'NOT_REPOSITORY');
  const missingPrefix = join(project, 'untracked project'); await mkdir(missingPrefix);
  await assert.rejects(createManagedWorktree({ projectRoot: missingPrefix, dataRoot: data, chatId: 'missing-prefix' }), (e: any) => e.code === 'PROJECT_NOT_IN_BASE');
  assert.equal(git(project, 'branch', '--list', 'hive/missing-prefix'), '');
  await assert.rejects(readWorktreeIntent(data, 'missing-prefix'));
  const markerless = await createManagedWorktree({ projectRoot: project, dataRoot: data, chatId: 'markerless' });
  const markerlessIntent = join(data, 'worktrees', 'markerless', 'intent.json');
  await writeFile(markerlessIntent, JSON.stringify({ state: 'creating', record: { ...markerless, gitDir: undefined } }));
  await rm(join(markerless.gitDir!, 'hive-desktop-owner.json'));
  const unfinished = await readWorktreeIntent(data, 'markerless');
  await assert.rejects(recoverManagedWorktree(data, unfinished.record), (e: any) => e.code === 'IDENTITY_MISMATCH');
  assert.equal(await readFile(join(markerless.executionPath, 'sub folder', 'same.txt'), 'utf8'), 'main');
  git(project, 'worktree', 'remove', markerless.managedWorktreeRoot);
  const replacement = await createManagedWorktree({ projectRoot: project, dataRoot: data, chatId: 'replacement' });
  git(project, 'worktree', 'remove', replacement.managedWorktreeRoot);
  git(project, 'worktree', 'add', replacement.managedWorktreeRoot, replacement.branch);
  await assert.rejects(validateManagedWorktree(data, replacement), (e: any) => e.code === 'IDENTITY_MISMATCH');
  await assert.rejects(removeManagedWorktree(data, replacement, { active: false }), (e: any) => e.code === 'IDENTITY_MISMATCH');
  assert.equal(await readFile(join(replacement.executionPath, 'sub folder', 'same.txt'), 'utf8'), 'main');
  git(project, 'worktree', 'remove', replacement.managedWorktreeRoot);
  const savedPath = process.env.PATH;
  process.env.PATH = join(root, 'no-git');
  try { await assert.rejects(createManagedWorktree({ projectRoot: project, dataRoot: data, chatId: 'missing-git' }), (e: any) => e.code === 'GIT_NOT_FOUND'); }
  finally { if (savedPath === undefined) delete process.env.PATH; else process.env.PATH = savedPath; }
  const brokenData = join(root, 'broken-data'); await mkdir(brokenData); await writeFile(join(brokenData, 'worktrees'), 'block');
  await assert.rejects(createManagedWorktree({ projectRoot: project, dataRoot: brokenData, chatId: 'blocked' }));
  if (process.platform !== 'win32') {
    const intentPath = join(data, 'worktrees', 'hook-failure', 'intent.json');
    const hook = join(project, '.git', 'hooks', 'post-checkout');
    await writeFile(hook, '#!/bin/sh\nmv "$HIVE_TEST_INTENT" "$HIVE_TEST_INTENT.saved"\nmkdir "$HIVE_TEST_INTENT"\n');
    await chmod(hook, 0o755);
    process.env.HIVE_TEST_INTENT = intentPath;
    try {
      await assert.rejects(createManagedWorktree({ projectRoot: project, dataRoot: data, chatId: 'hook-failure' }), (error: any) => error.code === 'METADATA_WRITE_FAILED');
    } finally { delete process.env.HIVE_TEST_INTENT; await rm(hook, { force: true }); }
    await rm(intentPath, { recursive: true }); await rename(`${intentPath}.saved`, intentPath);
    const interrupted = await readWorktreeIntent(data, 'hook-failure');
    assert.equal(interrupted.state, 'creating');
    assert.equal(await readFile(join(interrupted.record.executionPath, 'sub folder', 'same.txt'), 'utf8'), 'main');
    const recovered = await recoverManagedWorktree(data, interrupted.record);
    await removeManagedWorktree(data, recovered, { active: false });
  }
  git(first.managedWorktreeRoot, 'restore', '.'); git(second.managedWorktreeRoot, 'restore', '.');
  const fromLinked = await createManagedWorktree({ projectRoot: first.executionPath, dataRoot: data, chatId: 'linked' });
  assert.equal(fromLinked.repoCommonDir, first.repoCommonDir);
  await writeFile(join(fromLinked.executionPath, 'commit.txt'), 'unpushed');
  git(fromLinked.managedWorktreeRoot, 'add', '.'); git(fromLinked.managedWorktreeRoot, 'commit', '-qm', 'local');
  await assert.rejects(removeManagedWorktree(data, fromLinked, { active: false }), /without an upstream/);
  git(fromLinked.managedWorktreeRoot, 'reset', '--hard', fromLinked.baseOid);
  await removeManagedWorktree(data, fromLinked, { active: false });
  await removeManagedWorktree(data, first, { active: false });
  assert.equal(git(project, 'branch', '--list', 'hive/first'), 'hive/first');
  await removeManagedWorktree(data, second, { active: false });
  await assert.rejects(validateManagedWorktree(data, first));
  console.log('GIT_WORKTREES_OK');
} finally { await rm(root, { recursive: true, force: true }); }

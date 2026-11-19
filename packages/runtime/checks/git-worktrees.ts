import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import { chmod, cp, mkdtemp, mkdir, readFile, rename, rm, writeFile } from 'node:fs/promises';
import { syncBuiltinESMExports } from 'node:module';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { createManagedWorktree, previewManagedWorktreeRemoval, readWorktreeIntent, recoverManagedWorktree, removeManagedWorktree, validateManagedWorktree } from '../src/git-worktrees.js';
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
  await writeFile(join(project, '.git/info/exclude'), 'local-settings.json\n');
  const ignored = join(first.executionPath, 'local-settings.json');
  await writeFile(ignored, 'local only');
  await assert.rejects(removeManagedWorktree(data, first, {active: false}), (error: any) => error.code === 'WORKTREE_DIRTY');
  assert.equal(await readFile(ignored, 'utf8'), 'local only');
  await rm(ignored);
  git(first.executionPath, 'update-index', '--assume-unchanged', 'same.txt');
  await assert.rejects(previewManagedWorktreeRemoval(data, first), (error: any) => error.code === 'WORKTREE_HIDDEN_CHANGES');
  git(first.executionPath, 'update-index', '--no-assume-unchanged', 'same.txt');
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
  const bare = join(root, 'remote.git');
  git(root, 'init', '--bare', '--template=', bare);
  git(project, 'remote', 'add', 'fixture', bare);
  git(fromLinked.managedWorktreeRoot, 'push', 'fixture', `HEAD:refs/heads/${fromLinked.branch}`);
  const pushed = await previewManagedWorktreeRemoval(data, fromLinked);
  assert.deepEqual(pushed.verifiedRemote, {remote:'fixture', branch:fromLinked.branch});
  git(bare, 'update-ref', '-d', `refs/heads/${fromLinked.branch}`);
  await assert.rejects(removeManagedWorktree(data, fromLinked, {active:false, expectedVersion:pushed.version}), (error:any) => error.code === 'UNPUSHED_COMMITS');
  assert.equal(await readFile(join(fromLinked.executionPath, 'commit.txt'), 'utf8'), 'unpushed');
  git(fromLinked.managedWorktreeRoot, 'push', 'fixture', `HEAD:refs/heads/${fromLinked.branch}`);
  await assert.rejects(removeManagedWorktree(data, fromLinked, {active:false, expectedVersion:pushed.version,
    beforeRemove: async () => { git(bare, 'update-ref', '-d', `refs/heads/${fromLinked.branch}`); },
  }), (error:any) => error.code === 'UNPUSHED_COMMITS');
  assert.equal(await readFile(join(fromLinked.executionPath, 'commit.txt'), 'utf8'), 'unpushed');
  git(fromLinked.managedWorktreeRoot, 'push', 'fixture', `HEAD:refs/heads/${fromLinked.branch}`);

  const removalIntent = join(dirname(fromLinked.managedWorktreeRoot), 'intent.json');
  const originalRename = fs.promises.rename;
  let rejectPhase: 'intent' | 'receipt' = 'intent';
  fs.promises.rename = async (source, destination) => {
    if (destination === removalIntent) {
      const document = JSON.parse(await readFile(source, 'utf8'));
      if ((rejectPhase === 'intent' && document.removal) || (rejectPhase === 'receipt' && document.removed))
        throw Object.assign(new Error('Injected worktree removal disk error'), {code:'EIO'});
    }
    return originalRename(source, destination);
  };
  syncBuiltinESMExports();
  try {
    await assert.rejects(removeManagedWorktree(data, fromLinked, {active:false, expectedVersion:pushed.version}),
      (error:any) => error.code === 'EIO');
    assert.equal(await readFile(join(fromLinked.executionPath, 'commit.txt'), 'utf8'), 'unpushed');
    rejectPhase = 'receipt';
    await assert.rejects(removeManagedWorktree(data, fromLinked, {active:false, expectedVersion:pushed.version}),
      (error:any) => error.code === 'WORKTREE_REMOVAL_UNCERTAIN');
  } finally {
    fs.promises.rename = originalRename;
    syncBuiltinESMExports();
  }
  const pending = await readFile(removalIntent, 'utf8');
  assert.equal(JSON.parse(pending).removal.headOid, pushed.headOid);
  assert.equal(JSON.parse(pending).removed, undefined);
  assert.ok(!git(project, 'worktree', 'list', '--porcelain').includes(fromLinked.managedWorktreeRoot));
  await mkdir(fromLinked.managedWorktreeRoot);
  assert.equal((await readWorktreeIntent(data, fromLinked.chatId)).removed, undefined, 'an unrelated replacement blocks reconciliation');
  await rm(fromLinked.managedWorktreeRoot, {recursive:true});
  git(project, 'update-ref', `refs/heads/${fromLinked.branch}`, fromLinked.baseOid);
  await assert.rejects(validateManagedWorktree(data, fromLinked), (error:any) => error.code === 'WORKTREE_REMOVAL_UNCERTAIN');
  assert.equal(await readFile(removalIntent, 'utf8'), pending);
  git(project, 'update-ref', `refs/heads/${fromLinked.branch}`, pushed.headOid);
  assert.equal(git(project, 'rev-parse', fromLinked.branch), pushed.headOid);
  const common = fromLinked.repoCommonDir;
  await rename(common, common + '.held');
  try {
    await cp(common + '.held', common, {recursive:true});
    assert.equal(git(project, 'rev-parse', fromLinked.branch), pushed.headOid);
    assert.equal((await readWorktreeIntent(data, fromLinked.chatId)).removed, undefined, 'replacement Git metadata is not the original repository');
    assert.equal(await readFile(removalIntent, 'utf8'), pending);
  } finally {
    await rm(common, {recursive:true,force:true});
    await rename(common + '.held', common);
  }
  assert.equal((await readWorktreeIntent(data, fromLinked.chatId)).removed, true);
  assert.equal(JSON.parse(await readFile(removalIntent, 'utf8')).removal, undefined);
  const external = await createManagedWorktree({projectRoot:project,dataRoot:data,chatId:'externally-removed'});
  git(project,'worktree','remove','--',external.managedWorktreeRoot);
  assert.equal((await readWorktreeIntent(data, external.chatId)).removed, undefined);
  await assert.rejects(validateManagedWorktree(data, external), (error:any) => error.code === 'IDENTITY_MISMATCH');
  const contested = await createManagedWorktree({projectRoot:project,dataRoot:data,chatId:'replaced-during-removal'});
  const contestedIntent = join(dirname(contested.managedWorktreeRoot), 'intent.json');
  let replacementInstalled = false;
  fs.promises.rename = async (source, destination) => {
    await originalRename(source, destination);
    if (destination !== contestedIntent || !JSON.parse(await readFile(destination, 'utf8')).removal || replacementInstalled) return;
    git(project,'worktree','remove','--',contested.managedWorktreeRoot);
    git(project,'worktree','add','-b','external-replacement',contested.managedWorktreeRoot);
    replacementInstalled = true;
  };
  syncBuiltinESMExports();
  try {
    await assert.rejects(removeManagedWorktree(data, contested, {active:false}), (error:any) => error.code === 'IDENTITY_MISMATCH');
  } finally {
    fs.promises.rename = originalRename;
    syncBuiltinESMExports();
  }
  assert.equal(replacementInstalled,true);
  assert.equal(git(contested.managedWorktreeRoot,'branch','--show-current'),'external-replacement');
  assert.equal((await readWorktreeIntent(data, contested.chatId)).removed,undefined);
  await removeManagedWorktree(data, first, { active: false });
  assert.equal(git(project, 'branch', '--list', 'hive/first'), 'hive/first');
  await removeManagedWorktree(data, second, { active: false });
  await assert.rejects(validateManagedWorktree(data, first), (error: any) => error.code === 'WORKTREE_REMOVED');
  console.log('GIT_WORKTREES_OK');
} finally { await rm(root, { recursive: true, force: true }); }

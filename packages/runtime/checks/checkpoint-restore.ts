import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { link, mkdir, mkdtemp, readFile, realpath, rm, rmdir, symlink, unlink, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { CodeCheckpoints } from '../src/code-checkpoints.js';
import { restoreCheckpoint } from '../src/checkpoint-restore.js';
import { createManagedWorktree } from '../src/git-worktrees.js';

async function run(group: 'guards' | 'recovery') {
  const root = await realpath(await mkdtemp(join(tmpdir(), 'hive-coded-rewind-')));
  const project = join(root, 'project');
  const dataRoot = join(root, 'state');
  const workspaceId = 'rewind-workspace', sessionId = randomUUID(), operationId = randomUUID();
  const git = (cwd: string, ...args: string[]) => execFileSync('git', [
    '-c', 'user.name=Restore Fixture', '-c', 'user.email=fixture@example.invalid',
    '-c', 'commit.gpgSign=false', '-c', `core.hooksPath=${process.platform === 'win32' ? 'NUL' : '/dev/null'}`, ...args,
  ], {cwd, encoding: 'utf8', env: {...process.env, GIT_OPTIONAL_LOCKS: '0'}}).trim();

  const started = Date.now();
  const mark = (stage: string) => console.log(JSON.stringify({check: 'checkpoint-restore', group, stage, elapsedMs: Date.now() - started}));
  try {
    mark('setup');
    await mkdir(project);
    git(project, 'init', '--template=');
    await mkdir(join(project, 'nested'));
    for (const [path, bytes] of [
      ['.gitignore', 'ignored.txt\n'], ['shape', 'target-file'], ['nested/base.txt', 'target-child'],
      ['tracked.txt', 'target-head'], ['target-only.txt', 'keep in target'],
    ]) await writeFile(join(project, path), bytes);
    git(project, 'add', '.');
    git(project, 'commit', '-m', 'Target HEAD');
    const targetHead = git(project, 'rev-parse', 'HEAD');
    const owned = await createManagedWorktree({projectRoot: project, dataRoot, chatId: sessionId, branch: 'codex/restore-fixture'});
    const tree = owned.executionPath;
    assert.ok(owned.gitDir);
    const cp = new CodeCheckpoints({dataDir: dataRoot, workspaceId, sessionId, executionPath: tree});
    const capture = async () => {
      const preview = await cp.preview();
      assert.equal(preview.canCapture, true, JSON.stringify(preview.unsupported));
      const saved = await cp.capture({expectedVersion: preview.version, requestId: randomUUID()});
      return cp.reader(saved.checkpointId);
    };
    await writeFile(join(tree, 'tracked.txt'), 'target-index');
    git(tree, 'add', 'tracked.txt');
    await writeFile(join(tree, 'tracked.txt'), 'target-disk');
    await writeFile(join(tree, 'ignored.txt'), 'ignored before checkpoint');
    if (process.platform !== 'win32') await symlink('tracked.txt', join(tree, 'target-link'));
    mark('target.capture');
    const target = await capture();

    await unlink(join(tree, 'shape'));
    await mkdir(join(tree, 'shape'));
    await writeFile(join(tree, 'shape', 'new.txt'), 'future-child');
    await unlink(join(tree, 'nested', 'base.txt'));
    await rmdir(join(tree, 'nested'));
    await writeFile(join(tree, 'nested'), 'future-file');
    await unlink(join(tree, 'target-only.txt'));
    await writeFile(join(tree, 'future-only.txt'), 'keep in the backup');
    if (process.platform !== 'win32') await unlink(join(tree, 'target-link'));
    git(tree, 'add', '-A');
    git(tree, 'commit', '-m', 'Future HEAD');
    const futureHead = git(tree, 'rev-parse', 'HEAD');
    await writeFile(join(tree, 'tracked.txt'), 'future-index');
    git(tree, 'add', 'tracked.txt');
    await writeFile(join(tree, 'tracked.txt'), 'future-disk');
    await writeFile(join(tree, 'ignored.txt'), 'ignored changes must stay');
    mark('future.capture');
    const future = await capture();
    const request = {dataRoot, workspaceId, sessionId, operationId, checkpoints: cp, future, target};
    assert.notEqual(targetHead, futureHead);
    const indexLock = join(owned.gitDir, 'index.lock');
    if (group === 'guards') {
      mark('guards');
      await assert.rejects(restoreCheckpoint({...request, sessionId: randomUUID()}));
      await assert.rejects(restoreCheckpoint({...request, workspaceId: 'different-project'}), /exact owned worktree/);

      await writeFile(indexLock, 'someone else owns this lock', {flag: 'wx'});
      await assert.rejects(restoreCheckpoint(request), /locked by another operation/);
      assert.equal(await readFile(indexLock, 'utf8'), 'someone else owns this lock');
      assert.equal(await readFile(join(tree, 'tracked.txt'), 'utf8'), 'future-disk');
      await unlink(indexLock);
    }

    if (group === 'recovery') {
      mark('partial-recovery');
      const indexStage = join(owned.gitDir, `hive-restore-index-${operationId}`);
      assert.ok(target.manifest.rawIndex);
      await writeFile(indexStage, await target.read(target.manifest.rawIndex), {flag: 'wx'});
      await link(indexStage, indexLock);

      // A concurrent ref writer fails the final CAS after the tree and index have
      // changed. The same operation can verify that partial state and continue.
      const refLock = join(owned.repoCommonDir, 'refs/heads/codex/restore-fixture.lock');
      await writeFile(refLock, 'foreign reference lock', {flag: 'wx'});
      await assert.rejects(restoreCheckpoint(request), /RESTORE_GIT_FAILED/);
      assert.equal(git(tree, 'rev-parse', 'HEAD'), futureHead);
      assert.equal(git(tree, 'show', ':tracked.txt'), 'target-index');
      assert.equal(await readFile(join(tree, 'tracked.txt'), 'utf8'), 'target-disk');
      assert.equal(await readFile(refLock, 'utf8'), 'foreign reference lock');
      await unlink(refLock);

      mark('external-writer-conflict');
      await writeFile(join(tree, 'tracked.txt'), 'external writer');
      await assert.rejects(restoreCheckpoint(request), /File changed outside the restore/);
      assert.equal(await readFile(join(tree, 'tracked.txt'), 'utf8'), 'external writer');
      await writeFile(join(tree, 'tracked.txt'), 'target-disk');
    }
    mark('restore-and-retry');
    await restoreCheckpoint(request);
    await restoreCheckpoint(request);
    mark('verify');
    assert.equal(git(tree, 'rev-parse', 'HEAD'), targetHead);
    assert.equal(git(tree, 'show', 'HEAD:tracked.txt'), 'target-head');
    assert.equal(git(tree, 'show', ':tracked.txt'), 'target-index');
    assert.equal(await readFile(join(tree, 'shape'), 'utf8'), 'target-file');
    assert.equal(await readFile(join(tree, 'nested', 'base.txt'), 'utf8'), 'target-child');
    assert.equal(await readFile(join(tree, 'ignored.txt'), 'utf8'), 'ignored changes must stay');
    assert.equal(await cp.matchesLayout(target.manifest), true);
    assert.equal((await future.read(future.manifest.disk.find(entry => entry.path === 'future-only.txt')!)).toString(), 'keep in the backup');
    assert.equal(await readFile(join(project, 'tracked.txt'), 'utf8'), 'target-head');
    console.log(`CHECKPOINT_RESTORE_OK ${group}: real owned Git worktree, HEAD/index/disk, ignored bytes, foreign locks and explicit recovery`);
  } finally { await rm(root, {recursive: true, force: true, maxRetries: 5}); }
}

const at = process.argv.indexOf('--group');
const selected = at < 0 ? ['guards', 'recovery'] as const : [process.argv[at + 1]];
for (const group of selected) {
  assert.ok(group === 'guards' || group === 'recovery', 'Pass --group guards|recovery, or omit it to run both');
  await run(group);
}

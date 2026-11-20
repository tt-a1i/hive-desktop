import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import {
  checkoutGitBranch,
  dropGitStash,
  fetchGitRemote,
  listGitBranches,
  listGitStashes,
  popGitStash,
  pullGitBranch,
  pushGitStash,
} from '../src/git-branches-stash.js';

const root = mkdtempSync(join(tmpdir(), 'hive-git-branch-stash-'));
const originDir = join(root, 'origin.git');
const repoDir = join(root, 'repo');
const peerDir = join(root, 'peer');

function git(cwd: string, args: string[]): string {
  return execFileSync('git', args, {
    cwd,
    encoding: 'utf8',
    env: {
      ...process.env,
      GIT_AUTHOR_NAME: 'Hive Test',
      GIT_AUTHOR_EMAIL: 'hive@example.com',
      GIT_COMMITTER_NAME: 'Hive Test',
      GIT_COMMITTER_EMAIL: 'hive@example.com',
    },
  }).trim();
}

try {
  // Set up bare origin + initial repo + peer clone
  git(root, ['init', '--bare', originDir]);
  git(root, ['clone', originDir, repoDir]);
  git(repoDir, ['checkout', '-b', 'main']);
  writeFileSync(join(repoDir, 'README.md'), 'hello\n', 'utf8');
  git(repoDir, ['add', 'README.md']);
  git(repoDir, ['commit', '-m', 'init']);
  git(repoDir, ['push', '-u', 'origin', 'main']);

  git(root, ['clone', originDir, peerDir]);
  git(peerDir, ['checkout', 'main']);

  // 1. List branches
  const initialBranches = await listGitBranches(repoDir);
  assert.equal(initialBranches.current, 'main');
  assert.equal(initialBranches.detached, false);
  assert.ok(initialBranches.local.some(b => b.name === 'main' && b.isCurrent));

  // 2. Create & switch branch
  const created = await checkoutGitBranch(repoDir, { branch: 'feat/cockpit', create: true });
  assert.equal(created.ok, true);
  assert.equal(created.branch, 'feat/cockpit');
  assert.equal(created.created, true);

  const afterCreate = await listGitBranches(repoDir);
  assert.equal(afterCreate.current, 'feat/cockpit');
  assert.ok(afterCreate.local.some(b => b.name === 'main'));

  // 3. Switch back to main
  const switched = await checkoutGitBranch(repoDir, { branch: 'main' });
  assert.equal(switched.branch, 'main');
  assert.equal(switched.created, false);

  // 4. Stash push (including untracked), list, pop
  writeFileSync(join(repoDir, 'README.md'), 'hello dirty\n', 'utf8');
  writeFileSync(join(repoDir, 'scratch.txt'), 'untracked work\n', 'utf8');
  const pushed = await pushGitStash(repoDir, { message: 'wip-changes', includeUntracked: true });
  assert.equal(pushed.ok, true);
  assert.equal(existsSync(join(repoDir, 'scratch.txt')), false);
  assert.equal(readFileSync(join(repoDir, 'README.md'), 'utf8'), 'hello\n');

  const stashes = await listGitStashes(repoDir);
  assert.equal(stashes.stashes.length, 1);
  assert.equal(stashes.stashes[0]!.ref, 'stash@{0}');
  assert.match(stashes.stashes[0]!.message, /wip-changes/);

  const popped = await popGitStash(repoDir, { ref: 'stash@{0}' });
  assert.equal(popped.ok, true);
  assert.equal(readFileSync(join(repoDir, 'README.md'), 'utf8'), 'hello dirty\n');
  assert.equal(readFileSync(join(repoDir, 'scratch.txt'), 'utf8'), 'untracked work\n');

  // 5. Stash push + drop
  await pushGitStash(repoDir, { message: 'drop-me', includeUntracked: true });
  assert.equal((await listGitStashes(repoDir)).stashes.length, 1);
  await dropGitStash(repoDir, { ref: 'stash@{0}' });
  assert.equal((await listGitStashes(repoDir)).stashes.length, 0);

  // 6. Remote commit from peer -> fetch & pull --ff-only
  writeFileSync(join(peerDir, 'README.md'), 'hello from peer\n', 'utf8');
  git(peerDir, ['add', 'README.md']);
  git(peerDir, ['commit', '-m', 'peer update']);
  git(peerDir, ['push', 'origin', 'main']);

  const fetched = await fetchGitRemote(repoDir, { prune: true });
  assert.equal(fetched.ok, true);
  const beforePullBranches = await listGitBranches(repoDir);
  const mainBranch = beforePullBranches.local.find(b => b.name === 'main');
  assert.equal(mainBranch?.behind, 1);

  const pulled = await pullGitBranch(repoDir, { rebase: false });
  assert.equal(pulled.ok, true);
  assert.equal(readFileSync(join(repoDir, 'README.md'), 'utf8'), 'hello from peer\n');

  console.log('GIT_BRANCHES_STASH_OK: branch list/create/switch, stash push/list/pop/drop, and fetch/pull --ff-only verified.');
} finally {
  rmSync(root, { recursive: true, force: true });
}

/** Real Hive workspace binding + real Git; no Agent processes or model requests. */
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdtemp, mkdir, writeFile, readFile, realpath, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createDesktopRuntime } from '../src/desktop-runtime.js';

const directory = await mkdtemp(join(tmpdir(), 'hive-workspace-git-'));
const repo = join(directory, 'repo 空格');
const plain = join(directory, 'not a repo');
const hooks = join(directory, 'empty-hooks');
await Promise.all([repo, plain, hooks].map(path => mkdir(path)));
const config = join(directory, 'gitconfig'); await writeFile(config, '');
const previous = {global: process.env.GIT_CONFIG_GLOBAL, system: process.env.GIT_CONFIG_NOSYSTEM};
process.env.GIT_CONFIG_GLOBAL = config; process.env.GIT_CONFIG_NOSYSTEM = '1';
const git = (...args: string[]) => execFileSync('git', ['-C', repo,
  '-c', `core.hooksPath=${hooks}`, '-c', 'core.fsmonitor=false', '-c', 'core.autocrlf=false',
  '-c', 'user.name=Hive fixture', '-c', 'user.email=fixture@example.test', '-c', 'commit.gpgSign=false', ...args], {encoding: 'utf8'});
let runtime: Awaited<ReturnType<typeof createDesktopRuntime>> | undefined;
let events = 0;
try {
  git('init', '--quiet', '--initial-branch=main');
  const path = '说明 file.txt';
  await writeFile(join(repo, path), 'before\n');
  await writeFile(join(repo, '.gitignore'), '.hive/\n');
  git('add', '--', path, '.gitignore'); git('commit', '--quiet', '-m', 'fixture baseline');
  await writeFile(join(repo, path), 'staged\n'); git('add', '--', path);
  await writeFile(join(repo, path), 'working\n');
  const indexBefore = await readFile(join(repo, '.git/index'));
  runtime = await createDesktopRuntime(join(directory, 'state'), () => events++);
  const workspace = await runtime.hive.request('POST', '/api/workspaces',
    {path: repo, name: 'Git workspace', autostart_orchestrator: false}) as {id: string};
  const other = await runtime.hive.request('POST', '/api/workspaces',
    {path: plain, name: 'Plain workspace', autostart_orchestrator: false}) as {id: string};
  const status = await runtime.chats.gitStatus(workspace.id);
  assert.ok(status.ok);
  assert.equal(status.status.projectRoot, await realpath(repo));
  assert.equal(status.status.branch, 'main');
  assert.equal(status.status.entries.length, 1);
  assert.equal(status.status.repositoryStagedTotal, 1);
  assert.equal(status.status.entries[0]?.path, path);
  assert.equal(status.status.entries[0]?.indexStatus, 'M');
  assert.equal(status.status.entries[0]?.worktreeStatus, 'M');
  for (const [comparison, before, after] of [
    ['staged', 'before\n', 'staged\n'], ['unstaged', 'staged\n', 'working\n'], ['head-worktree', 'before\n', 'working\n'],
  ] as const) {
    const result = await runtime.chats.gitDiff(workspace.id, path, comparison);
    assert.ok(result.ok);
    assert.equal(result.diff.path, path);
    assert.equal(result.diff.before.text, before);
    assert.equal(result.diff.after.text, after);
    assert.ok(result.diff.patch?.includes(`+${after.trim()}`));
  }
  const outside = await runtime.chats.gitDiff(workspace.id, '../outside', 'unstaged');
  assert.ok(!outside.ok); assert.equal(outside.error.code, 'INVALID_PATH');
  const invalid = await runtime.chats.gitDiff(workspace.id, path, null);
  assert.ok(!invalid.ok); assert.equal(invalid.error.code, 'INVALID_ARGUMENT');
  const noRepo = await runtime.chats.gitStatus(other.id);
  assert.ok(!noRepo.ok); assert.equal(noRepo.error.code, 'NOT_REPOSITORY');
  const noRepoDiff = await runtime.chats.gitDiff(other.id, path);
  assert.ok(!noRepoDiff.ok); assert.equal(noRepoDiff.error.code, 'NOT_REPOSITORY');
  await assert.rejects(runtime.chats.gitStatus('missing-workspace'));
  assert.deepEqual(await readFile(join(repo, '.git/index')), indexBefore, 'Read-only requests must preserve the actual index bytes');
  assert.equal(await readFile(join(repo, path), 'utf8'), 'working\n');
  assert.equal(events, 0);
  console.log('WORKSPACE_GIT_OK: real Hive binding, staged/working/HEAD diffs, typed failures, path isolation and unchanged index; no Agents/models.');
} finally {
  await runtime?.close();
  if (previous.global === undefined) delete process.env.GIT_CONFIG_GLOBAL; else process.env.GIT_CONFIG_GLOBAL = previous.global;
  if (previous.system === undefined) delete process.env.GIT_CONFIG_NOSYSTEM; else process.env.GIT_CONFIG_NOSYSTEM = previous.system;
  await rm(directory, {recursive: true, force: true});
}

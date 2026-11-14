/** Verify desktop close waits for an actual Git commit hook, without any Agent. */
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { access, chmod, mkdir, mkdtemp, readFile, realpath, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createDesktopRuntime } from '../src/desktop-runtime.js';

if (process.platform === 'win32') {
 console.log('GIT_WRITE_LIFECYCLE_SKIP: POSIX executable shell hook fixture; Windows lifecycle remains unverified.');
} else {
 const dir = await realpath(await mkdtemp(join(tmpdir(), 'hive-git-close-')));
 const repo = join(dir, 'repo'), entered = join(dir, 'entered'), release = join(dir, 'release'), finished = join(dir, 'finished');
 const old = { GIT_CONFIG_GLOBAL: process.env.GIT_CONFIG_GLOBAL, GIT_CONFIG_NOSYSTEM: process.env.GIT_CONFIG_NOSYSTEM,
  HIVE_TEST_ENTERED: process.env.HIVE_TEST_ENTERED, HIVE_TEST_RELEASE: process.env.HIVE_TEST_RELEASE, HIVE_TEST_FINISHED: process.env.HIVE_TEST_FINISHED };
 let runtime: Awaited<ReturnType<typeof createDesktopRuntime>> | undefined;
 let commitPromise: Promise<unknown> | undefined;
 try {
  await mkdir(repo); await writeFile(join(dir, 'gitconfig'), '');
  process.env.GIT_CONFIG_GLOBAL = join(dir, 'gitconfig'); process.env.GIT_CONFIG_NOSYSTEM = '1';
  process.env.HIVE_TEST_ENTERED = entered; process.env.HIVE_TEST_RELEASE = release; process.env.HIVE_TEST_FINISHED = finished;
  const git = (...args: string[]) => execFileSync('git', ['-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.test', '-c', 'commit.gpgSign=false', ...args], {cwd: repo, encoding: 'utf8', stdio: 'pipe'}).trim();
  git('init', '--template='); git('config', 'user.name', 'Fixture'); git('config', 'user.email', 'fixture@example.test'); git('config', 'commit.gpgSign', 'false');
  await writeFile(join(repo, 'file.txt'), 'base\n'); git('add', 'file.txt'); git('commit', '-m', 'base');
  const oldHead = git('rev-parse', 'HEAD');
  await writeFile(join(repo, 'file.txt'), 'staged contents\n'); git('add', 'file.txt');
  const hook = join(repo, '.git', 'hooks', 'pre-commit');
  await mkdir(join(repo, '.git', 'hooks'), {recursive: true});
  await writeFile(hook, '#!/bin/sh\nprintf entered > "$HIVE_TEST_ENTERED"\nwhile [ ! -e "$HIVE_TEST_RELEASE" ]; do sleep 0.05; done\nprintf finished > "$HIVE_TEST_FINISHED"\n');
  await chmod(hook, 0o700);
  runtime = await createDesktopRuntime(join(dir, 'state'), () => {});
  const workspace = await runtime.hive.request('POST', '/api/workspaces', {path: repo, name: 'fixture', autostart_orchestrator: false}) as {id: string};
  const preview = await runtime.chats.gitCommitPreview(workspace.id); assert.equal(preview.ok, true);
  if (!preview.ok) throw Error('commit preview failed');
  const commit = runtime.chats.gitCommitApply(workspace.id, preview.preview.version, 'lifecycle commit', false);
  commitPromise = commit;
  const deadline = Date.now() + 5000;
  while (!(await access(entered).then(() => true, () => false))) {
   if (Date.now() > deadline) throw Error('pre-commit hook did not start');
   await new Promise(resolve => setTimeout(resolve, 25));
  }
  assert.equal(await readFile(entered, 'utf8'), 'entered');
  let closed = false;
  const closing = runtime.close().then(() => { closed = true; });
  await new Promise(resolve => setTimeout(resolve, 150));
  assert.equal(closed, false, 'desktop close must wait for the in-flight commit');
  await assert.rejects(runtime.chats.gitIndexApply(workspace.id, 'stage', ['file.txt'], 'invalid'), /closing/);
  await assert.rejects(runtime.chats.gitCommitApply(workspace.id, preview.preview.version, 'second', false), /closing/);
  await writeFile(release, 'release');
  const result = await commit; assert.equal(result.ok, true); assert.equal(result.committed, true); assert.equal(result.indexUpdated, true);
  await closing;
  assert.equal(await readFile(finished, 'utf8'), 'finished');
  assert.notEqual(git('rev-parse', 'HEAD'), oldHead); assert.equal(git('rev-parse', 'HEAD'), result.commitOid);
  assert.equal(git('show', 'HEAD:file.txt'), 'staged contents'); assert.equal(git('diff', '--cached', '--name-only'), '');
  await assert.rejects(runtime.chats.gitIndexApply(workspace.id, 'stage', ['file.txt'], 'invalid'), /closing/);
  await assert.rejects(runtime.chats.gitCommitApply(workspace.id, preview.preview.version, 'third', false), /closing/);
  await new Promise(resolve => setTimeout(resolve, 300));
  assert.equal(await readFile(finished, 'utf8'), 'finished'); assert.equal(git('rev-parse', 'HEAD'), result.commitOid);
  runtime = undefined;
  console.log('GIT_WRITE_LIFECYCLE_OK');
 } finally {
  await writeFile(release, 'release').catch(() => {});
  await commitPromise?.catch(() => {});
  if (runtime) await runtime.close();
  for (const [key, value] of Object.entries(old)) { if (value === undefined) delete process.env[key]; else process.env[key] = value; }
  await rm(dir, {recursive: true, force: true, maxRetries: 5});
 }
}

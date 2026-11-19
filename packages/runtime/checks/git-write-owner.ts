/** macOS private local repositories and process ownership; never contacts a remote service. */
import assert from 'node:assert/strict';
import { execFileSync, spawn } from 'node:child_process';
import { access, mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { fileURLToPath } from 'node:url';
import { commitGitIndex, previewGitCommit } from '../src/git-mutations.js';
import { remoteCommand } from '../src/git-remotes.js';

assert.equal(process.platform, 'darwin');
assert(process.env.HIVE_DESKTOP_SHELL_OWNER, 'Supply the native owner executable');
if (process.argv[2] === '--run') {
  const [, , , mode, repo, bare] = process.argv;
  if (mode === 'commit') {
    const preview = await previewGitCommit(repo!, {timeoutMs: 10000});
    await commitGitIndex(repo!, {expectedVersion: preview.version, message: 'fixture'}, {timeoutMs: 10000});
  } else await remoteCommand('/usr/bin/git', ['push', bare!, 'HEAD:refs/heads/fixture'], repo!, 10000);
  process.exit(0);
}
const root = await mkdtemp(join(tmpdir(), 'hive-git-write-owner-'));
const env = {...process.env, GIT_CONFIG_GLOBAL: '/dev/null', GIT_CONFIG_SYSTEM: '/dev/null', GIT_CONFIG_NOSYSTEM: '1'};
const alive = (pid: number) => { try { process.kill(pid, 0); return true; } catch { return false; } };
const exists = async (path: string) => { try { await access(path); return true; } catch { return false; } };
const active = new Set<number>();
const unrelated = spawn(process.execPath, ['-e', 'setInterval(()=>{},1000)'], {detached: true, stdio: 'ignore'});
async function until(test: () => Promise<boolean> | boolean, label: string) {
  for (let i = 0; i < 500; i++) { if (await test()) return; await delay(10); }
  throw new Error(`Timed out: ${label}`);
}
try {
  for (const mode of ['commit', 'push']) for (const killGroup of [false, true]) {
    const dir = join(root, `${mode}-${killGroup}`), repo = join(dir, 'repo'), bare = join(dir, 'bare');
    await mkdir(repo, {recursive: true});
    const git = (...args: string[]) => execFileSync('/usr/bin/git', ['-C', repo, ...args], {env, encoding: 'utf8'}).trim();
    git('init', '--template=', '-q'); git('config', 'user.name', 'Fixture'); git('config', 'user.email', 'fixture@example.invalid'); git('config', 'commit.gpgSign', 'false');
    await writeFile(join(repo, 'file'), 'base'); git('add', 'file'); git('commit', '-qm', 'base');
    const head = git('rev-parse', 'HEAD');
    await writeFile(join(repo, 'file'), 'changed'); git('add', 'file');
    const index = await readFile(join(repo, '.git/index'));
    execFileSync('/usr/bin/git', ['init', '--bare', '--template=', '-q', bare], {env});
    const record = join(dir, 'pids'), marker = join(dir, 'late-write');
    const hook = mode === 'commit' ? 'pre-commit' : 'pre-push';
    await mkdir(join(repo, '.git/hooks'), {recursive: true});
    const hookScript = join(dir, 'hook.cjs');
    await writeFile(join(repo, '.git/hooks', hook), '#!/bin/sh\nexec "$HIVE_TEST_NODE" "$HIVE_TEST_GIT_HOOK" "$@"\n', {mode: 0o700});
    await writeFile(hookScript, `const fs=require('fs'),{spawn}=require('child_process');\nconst helper=spawn(process.execPath,['-e',${JSON.stringify(`setTimeout(()=>require('fs').writeFileSync(${JSON.stringify(marker)},'continued'),1200);setInterval(()=>{},1000)`)}],{stdio:['ignore','inherit','inherit']});\nfs.writeFileSync(${JSON.stringify(record)},JSON.stringify({git:process.ppid,hook:process.pid,helper:helper.pid}));\nsetInterval(()=>{},1000);\n`, {mode: 0o700});
    const node = spawn(process.execPath, [...process.execArgv, fileURLToPath(import.meta.url), '--run', mode, repo, bare], {env: {...env, HIVE_TEST_NODE: process.execPath, HIVE_TEST_GIT_HOOK: hookScript}, detached: true, stdio: ['ignore', 'ignore', 'inherit']});
    active.add(node.pid!);
    const closed = new Promise<void>(resolve => node.once('close', () => resolve()));
    await until(() => exists(record), 'hook started');
    const pids = JSON.parse(await readFile(record, 'utf8')) as {git:number;hook:number;helper:number};
    Object.values(pids).forEach(pid => active.add(pid));
    process.kill(killGroup ? -node.pid! : node.pid!, 'SIGKILL'); await closed; active.delete(node.pid!);
    await until(() => Object.values(pids).every(pid => !alive(pid)), 'Git, hook and helper exit');
    Object.values(pids).forEach(pid => active.delete(pid));
    await delay(1400);
    assert.equal(await exists(marker), false, 'helper did not continue writing after owner death');
    assert(alive(unrelated.pid!), 'unrelated process survives');
    assert.equal(git('rev-parse', 'HEAD'), head);
    assert((await readFile(join(repo, '.git/index'))).equals(index));
    const lock = await exists(join(repo, '.git/index.lock'));
    assert.equal(lock, mode === 'commit', 'interrupted application-owned index lock remains for inspection');
    console.log(JSON.stringify({mode, kill: killGroup ? 'node-group' : 'node-only', ownedGone: true, unrelatedAlive: true, lateWrite: false, headAndIndexUnchanged: true, indexLockRemains: lock}));
  }
} finally {
  for (const pid of [...active, unrelated.pid!]) { try { process.kill(pid, 'SIGKILL'); } catch {} }
  await rm(root, {recursive: true, force: true});
}

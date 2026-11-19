/** Offline fake gh, real isolated Git and process ownership on macOS. */
import assert from 'node:assert/strict';
import { execFileSync, spawn } from 'node:child_process';
import { access, mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { fileURLToPath } from 'node:url';
import { remoteCommand } from '../src/git-remotes.js';

assert.equal(process.platform, 'darwin');
assert(process.env.HIVE_DESKTOP_SHELL_OWNER, 'Supply the native owner executable');
if (process.argv[2] === '--run') {
  const [, , , gh, repo, record, marker] = process.argv;
  await remoteCommand(gh!, ['pr', 'create', '--repo', 'github.com/fixture/repo', record!, marker!], repo!, 10000, true);
  process.exit(0);
}
const root = await mkdtemp(join(tmpdir(), 'hive-gh-owner-'));
const gitEnv = { ...process.env, GIT_CONFIG_GLOBAL: '/dev/null', GIT_CONFIG_SYSTEM: '/dev/null', GIT_CONFIG_NOSYSTEM: '1' };
for (const key of ['GH_TOKEN', 'GITHUB_TOKEN', 'GH_ENTERPRISE_TOKEN', 'GITHUB_ENTERPRISE_TOKEN']) {
  delete process.env[key];
  delete gitEnv[key];
}
process.env.HOME = root;
process.env.GH_CONFIG_DIR = join(root, 'gh-config');
process.env.GH_REPO = 'wrong/repository';
gitEnv.HOME = root;
gitEnv.GH_CONFIG_DIR = join(root, 'gh-config');
const active = new Set<number>();
const alive = (pid: number) => { try { process.kill(pid, 0); return true; } catch { return false; } };
const exists = async (path: string) => { try { await access(path); return true; } catch { return false; } };
async function until(test: () => Promise<boolean> | boolean, label: string) {
  for (let i = 0; i < 500; i++) { if (await test()) return; await delay(10); }
  throw Error(`Timed out: ${label}`);
}
const unrelated = spawn(process.execPath, ['-e', 'setInterval(()=>{},1000)'], { detached: true, stdio: 'ignore' });
try {
  const repo = join(root, 'repo');
  await mkdir(repo);
  const git = (...args: string[]) => execFileSync('/usr/bin/git', ['-C', repo, ...args], { env: gitEnv, encoding: 'utf8' }).trim();
  git('init', '--template=', '-q');
  git('config', 'user.name', 'Fixture'); git('config', 'user.email', 'fixture@example.invalid');
  await writeFile(join(repo, 'file'), 'base'); git('add', 'file'); git('commit', '-qm', 'base');
  const head = git('rev-parse', 'HEAD');
  const index = await readFile(join(repo, '.git/index'));
  const gh = join(root, 'gh');
  const peer = fileURLToPath(new URL('./fixtures/git-gh-owner-peer.cjs', import.meta.url));
  const quote = (value: string) => "'" + value.replaceAll("'", "'\\''") + "'";
  const wrapper = `#!/bin/sh\nexec ${quote(process.execPath)} ${quote(peer)} "$@"\n`;
  await writeFile(gh, wrapper, { mode: 0o700 });
  const normal = await remoteCommand(gh, ['normal', 'value with spaces'], repo, 10000, true);
  assert.deepEqual(JSON.parse(normal.stdout), {args: ['normal', 'value with spaces'], ghRepo: null});
  assert.match(normal.stderr, /fixture diagnostic/);
  assert.equal(normal.code, 0);
  for (const killGroup of [false, true]) {
    const record = join(root, `pids-${killGroup}`), marker = join(root, `fake-pr-${killGroup}`);
    const node = spawn(process.execPath, [...process.execArgv, fileURLToPath(import.meta.url), '--run', gh, repo, record, marker], { env: gitEnv, detached: true, stdio: 'ignore' });
    active.add(node.pid!);
    const closed = new Promise<void>(resolve => node.once('close', () => resolve()));
    await until(() => exists(record), 'fake gh started');
    const pids = JSON.parse(await readFile(record, 'utf8')) as {gh: number; helper: number};
    Object.values(pids).forEach(pid => active.add(pid));
    process.kill(killGroup ? -node.pid! : node.pid!, 'SIGKILL');
    await closed; active.delete(node.pid!);
    await delay(1600);
    const result = { kill: killGroup ? 'node-group' : 'node-only', ghAlive: alive(pids.gh), helperAlive: alive(pids.helper), lateWrite: await exists(marker), unrelatedAlive: alive(unrelated.pid!) };
    console.log(JSON.stringify(result));
    assert.equal(result.ghAlive, false);
    assert.equal(result.helperAlive, false);
    assert.equal(result.lateWrite, false);
    assert.equal(result.unrelatedAlive, true);
    assert.equal(git('rev-parse', 'HEAD'), head);
    assert((await readFile(join(repo, '.git/index'))).equals(index));
    Object.values(pids).forEach(pid => active.delete(pid));
  }
  const record = join(root, 'pids-timeout'), marker = join(root, 'fake-pr-timeout');
  await assert.rejects(remoteCommand(gh, ['pr', 'create', record, marker], repo, 500, true), (error: any) => error.code === 'REMOTE_TIMEOUT');
  const timedOut = JSON.parse(await readFile(record, 'utf8')) as {gh: number; helper: number};
  Object.values(timedOut).forEach(pid => active.add(pid));
  await until(() => Object.values(timedOut).every(pid => !alive(pid)), 'timed out gh and helper exit');
  await delay(1400);
  assert.equal(await exists(marker), false);
  assert(alive(unrelated.pid!));
  Object.values(timedOut).forEach(pid => active.delete(pid));
  console.log(JSON.stringify({timeout: true, ownedGone: true, lateWrite: false, unrelatedAlive: true, stdoutStderrAndStdin: true}));
} finally {
  for (const pid of [...active, unrelated.pid!]) { try { process.kill(pid, 'SIGKILL'); } catch {} }
  await rm(root, { recursive: true, force: true });
}

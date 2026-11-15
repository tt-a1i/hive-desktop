/** Deterministic metadata regression for Hive's actual PTY adapter; no agent/model. */
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { dirname, join } from 'node:path';
import { pathToFileURL } from 'node:url';

const require = createRequire(import.meta.url);
const hiveRoot = dirname(require.resolve('@tt-a1i/hive/package.json'));
const { attachAgentPty, toAgentRunSnapshot, taskkillProcessTree } = await import(
 pathToFileURL(join(hiveRoot, 'dist/src/server/agent-manager-support.js')).href
);
let onData: ((chunk: string) => void) | undefined;
const pty = {
 pid: 0,
 onExit() {}, onData(callback: (chunk: string) => void) { onData = callback; }, on() {},
 pause() {}, resume() {}, resize() {}, write() {}, kill() {},
};
const run: any = {runId: 'fixture-run', agentId: 'fixture-agent', status: 'starting', output: '', exitCode: null};
attachAgentPty(run, pty, {publish() {}}, 'win32');
assert.equal(toAgentRunSnapshot(run).pid, 0, 'Uninitialized PID must not be fabricated');
pty.pid = 9860;
assert.equal(toAgentRunSnapshot(run).pid, 9860, 'Snapshot must observe the native PTY PID when published');
onData!('ready');
assert.equal(toAgentRunSnapshot(run).status, 'running');
assert.equal(toAgentRunSnapshot(run).pid, 9860);
let ran = false;
assert.equal(taskkillProcessTree(0, 'win32', () => { ran = true; }), false);
assert.equal(ran, false, 'PID zero must not be sent to taskkill');
const nonPty = {runId: 'child', agentId: 'child-agent', status: 'running', output: '', exitCode: null,
 process: {pid: 77, isStopped: () => false}};
assert.equal(toAgentRunSnapshot(nonPty).pid, 77, 'Ordinary child snapshots retain their own PID');
console.log('PTY_LIVE_PID_OK: delayed PID publication and zero guard; controlled adapter only, no native Windows claim.');

if (process.platform !== 'win32') {
 const hiveRequire = createRequire(require.resolve('@tt-a1i/hive/package.json'));
 const native = hiveRequire('@lydell/node-pty');
 const shell = native.spawn('/bin/sh', ['-c', 'printf "PTY_PID:%s\\n" "$$"; exec sleep 30'],
  {cols: 80, rows: 24, cwd: process.cwd(), env: process.env});
 const real: any = {runId: 'real-pty', agentId: 'real-agent', status: 'starting', output: '', exitCode: null};
 let exited = false;
 real.onNativeExit = () => { exited = true; };
 attachAgentPty(real, shell, {publish() {}, clear() {}});
 try {
  const deadline = Date.now() + 10000;
  while (!/PTY_PID:(\d+)/.test(real.output)) {
   if (Date.now() > deadline) throw Error('Real PTY did not publish its PID');
   await new Promise(resolve => setTimeout(resolve, 20));
  }
  const observed = Number(real.output.match(/PTY_PID:(\d+)/)![1]);
  assert.ok(observed > 0);
  assert.equal(toAgentRunSnapshot(real).pid, observed, 'Hive snapshot must report actual shell PID');
  real.process.stop();
  while (!exited) {
   if (Date.now() > deadline) throw Error('Real PTY did not exit after stop');
   await new Promise(resolve => setTimeout(resolve, 20));
  }
  assert.equal(real.process.isStopped(), true);
  assert.throws(() => process.kill(observed, 0), (error: NodeJS.ErrnoException) => error.code === 'ESRCH');
  console.log('PTY_LIVE_PID_NATIVE_POSIX_OK: actual shell PID, snapshot, stop and exit.');
 } finally { if (!exited) real.process.stop(); }
} else {
 console.log('PTY_LIVE_PID_NATIVE_POSIX_SKIPPED: Windows native PID needs its own CI fixture.');
}

/** macOS native owner, controlled peers only. HIVE_DESKTOP_SHELL_OWNER must name the built native executable. */
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdtemp, readFile, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { setTimeout as delay } from 'node:timers/promises';
import { probeCodexLimits } from '../src/codex-limits.js';

assert.equal(process.platform, 'darwin');
const owner = process.env.HIVE_DESKTOP_SHELL_OWNER;
assert(owner, 'Pass the newly built native owner executable');
if (process.argv[2] === '--probe') {
  await probeCodexLimits(process.argv[3]!, new AbortController().signal);
  process.exit(0);
}
const dir = await mkdtemp(join(tmpdir(), 'hive-probe-owner-'));
const ownedPids = new Set<number>();
const receipts: unknown[] = [];
const unrelated = spawn(process.execPath, ['-e', 'setInterval(()=>{},1000)'], { detached: true, stdio: 'ignore' });
const alive = (pid: number) => { try { process.kill(pid, 0); return true; } catch { return false; } };
async function until(test: () => Promise<boolean> | boolean, label: string) {
  for (let i = 0; i < 500; i++) { if (await test()) return; await delay(10); }
  throw new Error(`Timed out: ${label}`);
}
async function peer(name: string, mode: string) {
  const path = join(dir, `${name}.cjs`), pids = join(dir, `${name}.json`);
  await writeFile(path, `#!/usr/bin/env node
const fs=require('fs'),{spawn}=require('child_process');
const helper=spawn(process.execPath,['-e',"process.on('SIGTERM',()=>{});setInterval(()=>{},1000)"],{stdio:['ignore',process.stdout,process.stderr]});
fs.writeFileSync(${JSON.stringify(pids)},JSON.stringify({host:process.ppid,root:process.pid,helper:helper.pid}));
const rl=require('readline').createInterface({input:process.stdin});
rl.on('line',line=>{const x=JSON.parse(line);if(x.method==='initialize')console.log(JSON.stringify({id:x.id,result:{userAgent:'fixture'}}));if(x.method==='account/read')console.log(JSON.stringify({id:x.id,result:{requiresOpenaiAuth:true,account:{type:'chatgpt'}}}));if(x.method==='account/rateLimits/read' && ${JSON.stringify(mode)}==='normal')console.log(JSON.stringify({id:x.id,result:{rateLimits:{limitId:'codex',primary:{usedPercent:22,windowDurationMins:300,resetsAt:null}}}}));});
rl.on('close',()=>{process.stdout.write('PROBE_TAIL\\n',()=>process.exit(${mode === 'nonzero' ? 7 : 0}));});
`, { mode: 0o700 });
  return { path, pids };
}
async function loadPids(path: string) {
  await until(async () => { try { await readFile(path); return true; } catch { return false; } }, 'fixture startup');
  const pids = JSON.parse(await readFile(path, 'utf8')) as { host: number; root: number; helper: number };
  Object.values(pids).forEach(pid => ownedPids.add(pid));
  return pids;
}
async function gone(pids: { host: number; root: number; helper: number }) {
  await until(() => Object.values(pids).every(pid => !alive(pid)), 'owned scope cleanup');
  Object.values(pids).forEach(pid => ownedPids.delete(pid));
  assert(alive(unrelated.pid!));
}
try {
  const normal = await peer('normal', 'normal');
  const result = await probeCodexLimits(normal.path, new AbortController().signal);
  assert.equal(result.status, 'ok'); assert.equal(result.windows[0]?.usedPercent, 22);
  const normalPids = await loadPids(normal.pids); await gone(normalPids);
  receipts.push({ case: 'normal-root-exits-first', result, allExited: true });

  const nonzero = await peer('nonzero', 'nonzero');
  const host = spawn(owner, ['--hive-owned-cli', String(process.pid), nonzero.path], { detached: true, stdio: ['pipe', 'pipe', 'pipe'] });
  let output = ''; host.stdout.on('data', chunk => { output += chunk; });
  const hostExit = new Promise<number | null>((resolve, reject) => { host.once('error', reject); host.once('close', resolve); });
  const nonzeroPids = await loadPids(nonzero.pids); host.stdin.end();
  assert.equal(await hostExit, 7); assert(output.includes('PROBE_TAIL\n')); await gone(nonzeroPids);
  receipts.push({ case: 'nonzero-exit-and-pipe-drain', exitCode: 7, tailPreserved: true, allExited: true });

  for (const cancel of [false, true]) {
    const target = await peer(cancel ? 'cancelled' : 'timedout', 'hold');
    const controller = new AbortController();
    const pending = probeCodexLimits(target.path, controller.signal);
    const pids = await loadPids(target.pids);
    if (cancel) controller.abort();
    const failed = await pending;
    assert.equal(failed.status, cancel ? 'cancelled' : 'failed');
    assert.deepEqual(failed.windows, []); await gone(pids);
    receipts.push({ case: cancel ? 'cancelled-probe' : 'timedout-probe', status: failed.status, allExited: true });
  }

  for (const group of [false, true]) {
    const target = await peer(group ? 'group-kill' : 'node-kill', 'hold');
    const node = spawn(process.execPath, [...process.execArgv, fileURLToPath(import.meta.url), '--probe', target.path], { detached: true, stdio: 'ignore', env: process.env });
    ownedPids.add(node.pid!);
    const nodeExit = new Promise<void>(resolve => node.once('close', () => resolve()));
    const pids = await loadPids(target.pids);
    assert.notEqual(pids.host, node.pid);
    process.kill(group ? -node.pid! : node.pid!, 'SIGKILL'); await nodeExit;
    ownedPids.delete(node.pid!);
    await gone(pids);
    receipts.push({ case: group ? 'node-process-group-killed' : 'node-only-killed', allExited: true, unrelatedAlive: true });
  }
  console.log(JSON.stringify({ status: 'CODEX_PROBE_OWNER_OK', receipts }, null, 2));
} finally {
  for (const pid of [...ownedPids, unrelated.pid!]) { try { process.kill(pid, 'SIGKILL'); } catch {} }
  await rm(dir, { recursive: true, force: true });
}

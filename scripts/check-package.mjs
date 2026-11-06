import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import { once } from 'node:events';
import { mkdtemp, rm, realpath } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { createInterface } from 'node:readline';

const i = process.argv.indexOf('--bundle');
if (i < 0 || !process.argv[i + 1]) throw new Error('Pass --bundle PATH to the portable output directory');
const base = resolve(process.argv[i + 1]);
const resources = process.platform === 'darwin' ? join(base, 'Hive Desktop.app', 'Contents', 'Resources') : join(base, 'resources');
const node = join(resources, 'node', process.platform === 'darwin' ? 'bin/node' : 'node.exe');
const runtime = join(resources, 'runtime');
const directory = await mkdtemp(join(tmpdir(), 'hive-desktop-package-check-'));
const children = [];
function start(args) {
  const child = spawn(node, args, { cwd: directory, stdio: ['pipe', 'pipe', 'pipe'], detached: process.platform !== 'win32' });
  children.push(child);
  return child;
}
function deadline(promise, label, ms = 30_000) {
  let timer;
  return Promise.race([promise, new Promise((_, reject) => {
    timer = setTimeout(() => reject(new Error(`Timeout waiting for ${label} after ${ms}ms`)), ms);
  })]).finally(() => clearTimeout(timer));
}
function progress(message) { console.log(`[package-check] ${message}`); }
function stop(child) {
  if (!child.pid || child.exitCode !== null || child.signalCode !== null) return;
  progress(`Cleaning up child process tree ${child.pid}`);
  if (process.platform === 'win32') {
    const result = spawnSync('taskkill', ['/pid', String(child.pid), '/t', '/f'], { encoding: 'utf8', timeout: 5000 });
    if (result.status !== 0) { console.error(`taskkill failed: ${result.stderr || result.error || result.status}`); child.kill(); }
  } else {
    try { process.kill(-child.pid, 'SIGKILL'); } catch (error) { if (error.code !== 'ESRCH') throw error; }
  }
}
try {
  // Resolve the PTY from Hive's own dependency graph and actually launch a shell.
  progress('Starting PTY probe with bundled Node');
  const code = `const {createRequire}=require('node:module');console.log('PTY: resolving Hive dependency');const r=createRequire(process.argv[1]+'/package.json');const h=createRequire(r.resolve('@tt-a1i/hive/package.json'));const p=h('@lydell/node-pty');const shell=process.platform==='win32'?process.env.ComSpec:'/bin/sh';const args=process.platform==='win32'?['/c','echo HIVE_PTY_OK']:['-c','printf HIVE_PTY_OK'];console.log('PTY: spawning shell');const t=p.spawn(shell,args,{cwd:process.cwd(),env:process.env});console.log('PTY: spawn returned');let out='';t.onData(x=>{out+=x;console.log('PTY: received data')});t.onExit(e=>{console.log('PTY: onExit '+JSON.stringify(e));if(e.exitCode!==0||!out.includes('HIVE_PTY_OK'))process.exitCode=1;else console.log('PTY OK')});`;
  const pty = start(['-e', code, runtime]);
  let ptyOutput = ''; pty.stdout.on('data', x => { ptyOutput += x; process.stdout.write(x); }); pty.stderr.on('data', x => { ptyOutput += x; process.stderr.write(x); });
  const [ptyExit] = await deadline(once(pty, 'exit'), 'PTY probe process exit'); assert.equal(ptyExit, 0, ptyOutput); assert.match(ptyOutput, /PTY OK/);
  progress('PTY probe completed; starting Hive runtime');
  const child = start([join(runtime, 'dist', 'main.js'), '--data-dir', join(directory, 'state')]);
  let stderr = ''; child.stderr.on('data', x => { stderr += x; process.stderr.write(x); });
  const pending = new Map(); let next = 0;
  createInterface({ input: child.stdout }).on('line', line => { const value = JSON.parse(line); const waiter = pending.get(value.id); if (waiter) { pending.delete(value.id); value.error ? waiter.reject(new Error(JSON.stringify(value.error))) : waiter.resolve(value.result); } });
  const exited = once(child, 'exit');
  child.once('exit', code => { for (const waiter of pending.values()) waiter.reject(new Error(`Runtime exited ${code}: ${stderr}`)); });
  const request = (method, params = {}) => new Promise((resolve, reject) => {
    const id = ++next; const timer = setTimeout(() => { pending.delete(id); reject(new Error(`Timeout ${method}: ${stderr}`)); }, 30000);
    pending.set(id, { resolve: value => { clearTimeout(timer); resolve(value); }, reject: error => { clearTimeout(timer); reject(error); } });
    child.stdin.write(`${JSON.stringify({ id, method, params })}\n`);
  });
  progress('Initializing Hive runtime');
  const init = await request('initialize'); assert.ok(init.hivePort > 0);
  assert.deepEqual(await request('hive.request', { method: 'GET', path: '/api/workspaces' }), []);
  progress('Creating workspace without orchestrator');
  const workspace = await request('hive.request', { method: 'POST', path: '/api/workspaces', body: { path: await realpath(directory), name: 'Portable smoke', autostart_orchestrator: false } });
  assert.equal(workspace.orchestrator_start.run_id, null);
  const listed = await request('hive.request', { method: 'GET', path: '/api/workspaces' }); assert.equal(listed[0].id, workspace.id);
  progress('Shutting down Hive runtime');
  assert.deepEqual(await request('shutdown'), { ok: true }); assert.equal((await deadline(exited, 'runtime process exit'))[0], 0);
  await assert.rejects(fetch(`http://127.0.0.1:${init.hivePort}/api/ui/session`, { signal: AbortSignal.timeout(5000) }), error => error.cause?.code === 'ECONNREFUSED');
  console.log('Portable smoke passed: bundled Node, real PTY, Hive HTTP + SQLite workspace and shutdown; no model request.');
} finally { for (const child of children) stop(child); await rm(directory, { recursive: true, force: true, maxRetries: 3, retryDelay: 200 }); }

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
function start(args, stdin = 'pipe') {
  const child = spawn(node, args, { cwd: directory, stdio: [stdin, 'pipe', 'pipe'], detached: process.platform !== 'win32' });
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
  const code = `const {createRequire}=require('node:module');console.log('PTY: baseline resources '+JSON.stringify(process.getActiveResourcesInfo()));console.log('PTY: resolving Hive dependency');const r=createRequire(process.argv[1]+'/package.json');const h=createRequire(r.resolve('@tt-a1i/hive/package.json'));const p=h('@lydell/node-pty');const shell=process.platform==='win32'?process.env.ComSpec:'/bin/sh';const args=process.platform==='win32'?['/c','echo HIVE_PTY_OK']:['-c','printf HIVE_PTY_OK'];console.log('PTY: spawning shell');const t=p.spawn(shell,args,{cwd:process.cwd(),env:process.env});console.log('PTY: spawn returned');let out='';t.onData(x=>{out+=x;console.log('PTY: received data')});t.onExit(e=>{console.log('PTY: onExit '+JSON.stringify(e));const ok=e.exitCode===0&&out.includes('HIVE_PTY_OK');console.log('PTY: active resources at exit '+JSON.stringify(process.getActiveResourcesInfo()));setTimeout(()=>console.log('PTY: active resources after 1s '+JSON.stringify(process.getActiveResourcesInfo())),1000).unref();if(ok)console.log('PTY OK');else process.exitCode=1});`;
  const pty = start(['-e', code, runtime], 'ignore');
  let ptyOutput = ''; pty.stdout.on('data', x => { ptyOutput += x; process.stdout.write(x); }); pty.stderr.on('data', x => { ptyOutput += x; process.stderr.write(x); });
  const [ptyExit] = await deadline(once(pty, 'exit'), 'PTY probe process exit'); assert.equal(ptyExit, 0, ptyOutput); assert.match(ptyOutput, /PTY OK/);
  progress('PTY probe completed; starting real Hive agent-manager probe');
  const managerCode = `const {createRequire}=require('node:module');const {pathToFileURL}=require('node:url');const {join,dirname}=require('node:path');const assert=require('node:assert/strict');const stopping=process.argv[2]==='stop';console.log('Hive PTY: baseline resources '+JSON.stringify(process.getActiveResourcesInfo()));const r=createRequire(process.argv[1]+'/package.json');const hiveDir=dirname(r.resolve('@tt-a1i/hive/package.json'));import(pathToFileURL(join(hiveDir,'dist/src/server/agent-manager.js'))).then(async ({createAgentManager})=>{const manager=createAgentManager();const shell=process.platform==='win32'?process.env.ComSpec:'/bin/sh';const command=stopping?process.execPath:shell;const args=stopping?['-e','console.log("HIVE_MANAGER_OK");setInterval(()=>{},1000)']:process.platform==='win32'?['/c','echo HIVE_MANAGER_OK']:['-c','printf HIVE_MANAGER_OK'];const run=await manager.startAgent({agentId:'package-check',command,args,cwd:process.cwd(),onExit:e=>{const result=manager.getRun(e.runId);console.log('Hive PTY: onExit '+JSON.stringify(e));if(!stopping)assert.equal(e.exitCode,0);assert.match(result.output,/HIVE_MANAGER_OK/);manager.stopRun(e.runId);manager.removeRun(e.runId);console.log('Hive PTY: resources at exit '+JSON.stringify(process.getActiveResourcesInfo()));setTimeout(()=>console.log('Hive PTY: resources after 2.5s '+JSON.stringify(process.getActiveResourcesInfo())),2500).unref();console.log('HIVE MANAGER OK')}});console.log('Hive PTY: run '+run.runId);if(stopping){const timer=setInterval(()=>{if(manager.getRun(run.runId).output.includes('HIVE_MANAGER_OK')){clearInterval(timer);console.log('Hive PTY: stopping live process');manager.stopRun(run.runId)}},50)}}).catch(e=>{console.error(e);process.exitCode=1});`;
  for (const mode of ['natural', 'stop']) {
    progress(`Running Hive agent-manager ${mode} probe`);
    const manager = start(['-e', managerCode, runtime, mode], 'ignore');
    let managerOutput = ''; manager.stdout.on('data', x => { managerOutput += x; process.stdout.write(x); }); manager.stderr.on('data', x => { managerOutput += x; process.stderr.write(x); });
    const [managerExit] = await deadline(once(manager, 'exit'), `Hive agent-manager ${mode} probe process exit`); assert.equal(managerExit, 0, managerOutput); assert.match(managerOutput, /HIVE MANAGER OK/);
  }
  progress('Hive agent-manager probe completed; starting Hive runtime');
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

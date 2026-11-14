import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import { once } from 'node:events';
import { mkdtemp, rm, realpath, readFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createInterface } from 'node:readline';
import { createRequire } from 'node:module';

const i = process.argv.indexOf('--bundle');
if (i < 0 || !process.argv[i + 1]) throw new Error('Pass --bundle PATH to the portable output directory');
const base = resolve(process.argv[i + 1]);
const resources = process.platform === 'darwin' ? join(base, 'Hive Desktop.app', 'Contents', 'Resources') : join(base, 'resources');
const node = join(resources, 'node', process.platform === 'darwin' ? 'bin/node' : 'node.exe');
const runtime = join(resources, 'runtime');
const directory = await mkdtemp(join(tmpdir(), 'hive-desktop-package-check-'));
const children = [];
function start(args, stdin = 'pipe', env = process.env) {
  const child = spawn(node, args, { cwd: directory, env, stdio: [stdin, 'pipe', 'pipe'], detached: process.platform !== 'win32' });
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
  progress('Checking T3 Code license notice');
  assert.deepEqual(
    await readFile(join(resources, 'licenses/LICENSE-T3CODE')),
    await readFile(join(dirname(fileURLToPath(import.meta.url)), '../apps/desktop/licenses/LICENSE-T3CODE')),
    'The packaged T3 Code notice must match its preserved source license',
  );
  if (process.platform === 'darwin') {
    progress('Checking native application identity and decoded icon');
    const info = spawnSync('/usr/bin/plutil', ['-convert', 'json', '-o', '-', join(base, 'Hive Desktop.app/Contents/Info.plist')], { encoding: 'utf8' });
    assert.equal(info.status, 0, info.stderr);
    const plist = JSON.parse(info.stdout);
    const metadata = JSON.parse(await readFile(join(dirname(fileURLToPath(import.meta.url)), '../package.json'), 'utf8'));
    assert.equal(plist.CFBundleShortVersionString, metadata.version.match(/^\d+\.\d+\.\d+/)[0]);
    assert.equal(plist.CFBundleVersion, plist.CFBundleShortVersionString);
    assert.equal(plist.CFBundleExecutable, 'hive-desktop');
    assert.equal(plist.CFBundleIdentifier, 'com.hive.desktop.dev');
    assert.equal(plist.CFBundlePackageType, 'APPL');
    assert.equal(plist.NSHighResolutionCapable, true);
    assert.match(plist.LSMinimumSystemVersion, /^\d+\.\d+(\.\d+)?$/);
    assert.equal(plist.CFBundleIconFile, 'HiveDesktop.icns');
    const iconset = join(directory, 'verified.iconset');
    const icon = spawnSync('/usr/bin/iconutil', ['-c', 'iconset', '-o', iconset, join(resources, plist.CFBundleIconFile)], { encoding: 'utf8' });
    assert.equal(icon.status, 0, icon.stderr);
    const png = await readFile(join(iconset, 'icon_512x512.png'));
    assert.equal(png.subarray(0, 8).toString('hex'), '89504e470d0a1a0a');
    assert.equal(png.readUInt32BE(16), 512); assert.equal(png.readUInt32BE(20), 512);
  }
  progress('Checking compiled image payload contract with bundled Node');
  const imageProbe = start(['--input-type=module', '-e', `
    import assert from 'node:assert/strict';import {pathToFileURL} from 'node:url';
    const {promptImages,imageMessage}=await import(pathToFileURL(process.argv[1]+'/dist/prompt-images.js'));
    const image={type:'image',mimeType:'image/png',data:'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGP4z8AAAAMBAQDJ/pLvAAAAAElFTkSuQmCC'};
    assert.deepEqual(imageMessage('original',promptImages([image])),[{type:'text',text:'original'},image]);
    assert.throws(()=>promptImages([{...image,data:'invalid'}]));console.log('IMAGE_PAYLOAD_OK');
  `, runtime]);
  let imageOutput='';imageProbe.stdout.on('data',chunk=>{imageOutput+=chunk;});
  imageProbe.stderr.on('data',chunk=>process.stderr.write(chunk));
  assert.equal((await deadline(once(imageProbe,'close'),'image payload probe'))[0],0);
  assert.ok(imageOutput.includes('IMAGE_PAYLOAD_OK'));
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
  const piAgentDir = join(directory, 'pi-auth');
  const child = start([join(runtime, 'dist', 'main.js'), '--data-dir', join(directory, 'state')], 'pipe', {
    ...process.env, PI_CODING_AGENT_DIR: piAgentDir, PI_OFFLINE: '1',
  });
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
  const runtimeMetadata = JSON.parse(await readFile(join(runtime, 'package.json'), 'utf8'));
  const runtimeRequire = createRequire(join(runtime, 'package.json'));
  const hiveMetadata = JSON.parse(await readFile(runtimeRequire.resolve('@tt-a1i/hive/package.json'), 'utf8'));
  assert.equal(runtimeMetadata.name, '@hive-desktop/runtime');
  assert.equal(hiveMetadata.name, '@tt-a1i/hive');
  const actualNode = spawnSync(node, ['-p', 'JSON.stringify({nodeVersion:process.version,platform:process.platform,arch:process.arch})'], { encoding: 'utf8', timeout: 10_000 });
  assert.equal(actualNode.status, 0, actualNode.stderr);
  assert.equal(init.version, runtimeMetadata.version);
  assert.deepEqual(init.runtimeInfo, { version: runtimeMetadata.version, hiveVersion: hiveMetadata.version, ...JSON.parse(actualNode.stdout) });
  assert.equal(init.piAgentDir, piAgentDir);
  const accounts = await request('pi.auth.providers');
  assert.equal(accounts.agentDir, piAgentDir);
  assert.equal(accounts.login, null);
  assert.ok(accounts.providers.some(provider => provider.id === 'openai'));
  assert.ok(accounts.providers.find(provider => provider.id === 'anthropic').methods.some(method => method.type === 'api_key'));
  assert.ok(accounts.providers.every(provider => provider.storedType === null));
  assert.equal(await request('pi.auth.status'), null);
  const customConfig = await request('pi.models.get');
  assert.deepEqual(customConfig.providers, []);
  const configured = await request('pi.models.change', { revision: customConfig.revision, change: {
    action: 'provider', id: 'package-custom', name: 'Package fixture', baseUrl: 'https://example.test/v1', api: 'openai-responses', firstModelId: 'gpt-6.1-terra',
  } });
  assert.equal(configured.conflict, false);
  assert.equal(configured.snapshot.providers[0].models[0].id, 'gpt-6.1-terra');
  assert.ok((await request('pi.auth.providers')).providers.some(provider => provider.id === 'package-custom'));
  await assert.rejects(request('pi.auth.start', { providerId: 'invalid-provider' }));
  assert.deepEqual(await request('hive.request', { method: 'GET', path: '/api/workspaces' }), []);
  progress('Creating workspace without orchestrator');
  const workspace = await request('hive.request', { method: 'POST', path: '/api/workspaces', body: { path: await realpath(directory), name: 'Portable smoke', autostart_orchestrator: false } });
  assert.equal(workspace.orchestrator_start.run_id, null);
  const taskPath = `/api/workspaces/${workspace.id}/tasks`;
  const original = await request('hive.request', { method: 'GET', path: taskPath });
  const saveTasks = (base, content) => request('workspace.saveTasks', { workspaceId: workspace.id, base, content });
  assert.deepEqual(await saveTasks(original.content, 'First edit'), { content: 'First edit' });
  const conflict = await saveTasks(original.content, 'Stale edit');
  assert.deepEqual(conflict, { conflict: true, currentContent: 'First edit' });
  await request('hive.request', { method: 'PUT', path: taskPath, body: { content: 'Another edit' } });
  const newer = await saveTasks(conflict.currentContent, 'First edit + draft');
  assert.deepEqual(newer, { conflict: true, currentContent: 'Another edit' });
  assert.deepEqual(await saveTasks(newer.currentContent, 'Another edit + draft'), { content: 'Another edit + draft' });
  assert.deepEqual(await saveTasks('Another edit + draft', ''), { content: '' });
  assert.deepEqual(await saveTasks('Outdated', 'Stale'), { conflict: true, currentContent: '' });
  const listed = await request('hive.request', { method: 'GET', path: '/api/workspaces' }); assert.equal(listed[0].id, workspace.id);
  for (const backend of ['pi', 'codex', 'pi-rpc', 'claude']) {
    const original = await request('chat.createRecord', { workspaceId: workspace.id, backend });
    const renamed = await request('chat.rename', { sessionId: original.id, title: ' 发布检查 🐝 ' });
    assert.deepEqual(renamed, { ...original, title: '发布检查 🐝', customTitle: true });
    const pin = await request('chat.pin', { sessionId: original.id, pinned: true });
    assert.deepEqual(pin.session, { ...renamed, pinned: true });
    assert.equal(pin.sessions[0].id, original.id);
    assert.equal(pin.session.updatedAt, original.updatedAt);
    const globalPins=await request('chat.pinned');
    assert.equal(globalPins.length,1);
    assert.equal(globalPins[0].session.id,original.id);
    assert.equal(globalPins[0].workspaceName,'Portable smoke');
    await request('chat.pin', { sessionId: original.id, pinned: false });
    const sessions = await request('chat.list', { workspaceId: workspace.id });
    assert.equal(sessions.find(session => session.id === original.id).title, '发布检查 🐝');
    await assert.rejects(request('chat.rename', { sessionId: original.id, title: '   ' }));
    assert.equal((await request('chat.list', { workspaceId: workspace.id })).find(session => session.id === original.id).title, '发布检查 🐝');
  }
  const found = await request('chat.search', { query: '发布检查' });
  assert.equal(found.total, 4);
  assert.deepEqual(new Set(found.matches.map(hit => hit.session.backend)), new Set(['pi', 'codex', 'pi-rpc', 'claude']));
  assert.ok(found.matches.every(hit => hit.workspaceName === 'Portable smoke'));
  assert.equal((await request('chat.search', { query: '外部 Pi' })).total, 1);
  assert.equal((await request('chat.search', { query: '没有这条记录' })).total, 0);
  progress('Shutting down Hive runtime');
  assert.deepEqual(await request('shutdown'), { ok: true }); assert.equal((await deadline(exited, 'runtime process exit'))[0], 0);
  await assert.rejects(fetch(`http://127.0.0.1:${init.hivePort}/api/ui/session`, { signal: AbortSignal.timeout(5000) }), error => error.cause?.code === 'ECONNREFUSED');
  console.log('Portable smoke passed: bundled Node, real PTY, Pi account metadata, Hive HTTP + SQLite workspace and shutdown; no model request.');
} finally { for (const child of children) stop(child); await rm(directory, { recursive: true, force: true, maxRetries: 3, retryDelay: 200 }); }

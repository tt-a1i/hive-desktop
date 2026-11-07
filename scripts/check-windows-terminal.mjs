/** Extracted Windows package: real Hive shell, ConPTY and desktop JSONL bridge; no model calls. */
import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { once } from 'node:events';
import { mkdir, mkdtemp, readFile, realpath, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { createInterface } from 'node:readline';

if (process.platform !== 'win32') throw new Error('This check requires Windows and real ConPTY');
const index = process.argv.indexOf('--bundle');
if (index < 0 || !process.argv[index + 1]) throw new Error('Pass --bundle PATH');
const resources = join(resolve(process.argv[index + 1]), 'resources');
const node = join(resources, 'node', 'node.exe');
const main = join(resources, 'runtime', 'dist', 'main.js');
const directory = await mkdtemp(join(tmpdir(), 'hive-windows-terminal-'));
const project = join(directory, 'project');
await mkdir(project);
const probe = join(directory, 'read-console.cjs');
const resultFile = join(directory, 'input.txt');
const marker = `HIVE_READY_${randomUUID().replaceAll('-', '')}`;
const resultMarker = `HIVE_RECEIVED_${randomUUID().replaceAll('-', '')}`;
await writeFile(probe, `const fs=require('node:fs'); const readline=require('node:readline'); console.log(${JSON.stringify(marker)}); readline.createInterface({input:process.stdin}).once('line', line=>{fs.writeFileSync(${JSON.stringify(resultFile)},line,'utf8');console.log(${JSON.stringify(resultMarker)});process.exit(0)});`);
const child = spawn(node, [main], { cwd: directory, env: { ...process.env, HIVE_DESKTOP_DATA_DIR: join(directory, 'state') }, stdio: ['pipe', 'pipe', 'pipe'] });
let stderr = ''; child.stderr.on('data', chunk => { stderr += chunk; });
const exited = once(child, 'exit');
const pending = new Map(); let next = 0; const events = [];
createInterface({ input: child.stdout }).on('line', line => {
  let value; try { value = JSON.parse(line); } catch { console.error('Invalid sidecar JSON:', line); return; }
  const waiter = pending.get(value.id);
  if (waiter) { pending.delete(value.id); value.error ? waiter.reject(new Error(JSON.stringify(value.error))) : waiter.resolve(value.result); }
  else if (value.event === 'terminal') events.push(value.data);
});
child.once('exit', code => { for (const waiter of pending.values()) waiter.reject(new Error(`Sidecar exited ${code}: ${stderr}`)); pending.clear(); });
const request = (method, params = {}) => new Promise((resolve, reject) => {
  const id = ++next; const timer = setTimeout(() => { pending.delete(id); reject(new Error(`Timeout ${method}: ${stderr}`)); }, 30000);
  pending.set(id, { resolve: value => { clearTimeout(timer); resolve(value); }, reject: error => { clearTimeout(timer); reject(error); } });
  child.stdin.write(`${JSON.stringify({ id, method, params })}\n`);
});
async function waitFor(fn, label, timeout = 30000) {
  const until = Date.now() + timeout;
  while (Date.now() < until) { const result = await fn(); if (result) return result; await new Promise(resolve => setTimeout(resolve, 30)); }
  throw new Error(`Timeout waiting for ${label}; sidecar stderr: ${stderr}`);
}
let cursor = 0; let output = Buffer.alloc(0); let attachmentId;
async function drain() {
  while (cursor < events.length) {
    const event = events[cursor++];
    if (event.attachmentId !== attachmentId) continue;
    if (event.type === 'error') throw new Error(`Terminal error: ${event.message}`);
    if (event.type === 'output') {
      const chunk = Buffer.from(event.bytes); output = Buffer.concat([output, chunk]);
      await request('terminal.ack', { attachmentId, sequence: event.sequence, bytes: chunk.length });
    }
  }
  return output;
}
const send = text => request('terminal.input', { attachmentId, bytes: [...Buffer.from(text, 'utf8')] });
let success = false;
try {
  const init = await request('initialize'); assert.ok(init.hivePort > 0);
  const workspace = await request('hive.request', { method: 'POST', path: '/api/workspaces', body: { path: await realpath(project), name: 'Windows terminal smoke', autostart_orchestrator: false } });
  assert.equal(workspace.orchestrator_start.run_id, null);
  const { runId } = await request('terminal.start', { workspaceId: workspace.id }); assert.ok(runId);
  attachmentId = randomUUID();
  assert.equal((await request('terminal.attach', { workspaceId: workspace.id, runId, attachmentId, cols: 100, rows: 30 })).ok, true);
  const restore = await waitFor(() => events.find(event => event.attachmentId === attachmentId && event.type === 'restore'), 'restore');
  assert.equal((await request('terminal.restoreComplete', { attachmentId, sequence: restore.sequence })).ready, true);
  // Set the console code page before passing text to cmd's cooked console, then run a real stdin consumer.
  await send(`chcp 65001 >nul\r"${node}" "${probe}"\r`);
  await waitFor(async () => (await drain()).includes(Buffer.from(marker)), 'console consumer ready');
  const text = `中文输入-${randomUUID()}`;
  await send(`${text}\r`);
  await waitFor(async () => (await drain()).includes(Buffer.from(resultMarker)), 'console consumer completion');
  assert.equal(await readFile(resultFile, 'utf8'), text, 'child process must receive exact Unicode, not just console echo');
  await send('exit\r');
  const exitEvent = await waitFor(async () => { await drain(); return events.find(event => event.attachmentId === attachmentId && event.type === 'exit'); }, 'natural shell exit');
  assert.equal(exitEvent.code, 0);
  assert.equal((await request('shutdown')).ok, true);
  let timer;
  const [code] = await Promise.race([exited, new Promise((_, reject) => { timer = setTimeout(() => reject(new Error('Timeout waiting for sidecar exit')), 30000); })]).finally(() => clearTimeout(timer));
  assert.equal(code, 0);
  await assert.rejects(fetch(`http://127.0.0.1:${init.hivePort}/api/ui/session`, { signal: AbortSignal.timeout(5000) }), error => error.cause?.code === 'ECONNREFUSED');
  success = true;
  console.log(JSON.stringify({ pass: true, platform: process.platform, realExtractedSidecar: true, realHiveAndConPty: true, cookedConsoleUnicodeInputVerifiedByChild: true, naturalShellExit: true, sidecarExit: true, noModel: true, guiTested: false }));
} catch (error) {
  console.error(JSON.stringify({ pass: false, error: String(error), stack: error.stack, stderr, terminalEvents: events.map(({ bytes, ...event }) => ({ ...event, ...(bytes ? { byteCount: bytes.length } : {}) })) }));
  throw error;
} finally {
  child.stdin.end();
  if (child.pid && child.exitCode === null && child.signalCode === null) {
    const killed = spawnSync('taskkill', ['/pid', String(child.pid), '/t', '/f'], { encoding: 'utf8', timeout: 5000 });
    if (killed.status !== 0) console.error(`Cleanup failed: ${killed.stderr || killed.error || killed.status}`);
  }
  if (success) await rm(directory, { recursive: true, force: true, maxRetries: 3, retryDelay: 200 });
  else console.error(`Diagnostic data retained at ${directory}`);
}

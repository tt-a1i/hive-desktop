/** Windows only: real official CLI controls, never creates a thread/turn or sends a model prompt. */
import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { mkdir, mkdtemp, readFile, readdir, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { CodexTransport } from '../src/agents/codex-transport.js';
import { ExternalPiTransport } from '../src/agents/external-pi-transport.js';
import { OwnedCliProcess } from '../src/agents/owned-cli-process.js';
if (process.platform !== 'win32') throw new Error('This check requires actual Windows; static support is not a Windows verification');
const prefix = process.env.HIVE_WINDOWS_NATIVE_CLI_PREFIX;
assert.ok(prefix, 'Set HIVE_WINDOWS_NATIVE_CLI_PREFIX to an official npm install prefix containing spaces');
assert.ok(prefix.includes(' '), 'The installed CLI prefix must contain a space');
const directory = await mkdtemp(join(process.env.RUNNER_TEMP ?? process.env.TEMP ?? process.cwd(), 'hive native CLI space '));
const evidence = resolve('../../.artifacts/windows-native-clis.json');
const previousPath = process.env.PATH;
process.env.PATH = `${prefix};${previousPath ?? ''}`;
const receipts: unknown[] = [];
const installed = { pi: JSON.parse(await readFile(join(prefix, 'node_modules', '@earendil-works', 'pi-coding-agent', 'package.json'), 'utf8')).version as string,
  codex: JSON.parse(await readFile(join(prefix, 'node_modules', '@openai', 'codex', 'package.json'), 'utf8')).version as string };
assert.equal(installed.pi, '1.4.0'); assert.equal(installed.codex, '0.166.0');
type ProcessInfo = { ProcessId: number; ParentProcessId: number; Name: string; CreationDate: string };
const powershell = join(process.env.SystemRoot!, 'System32', 'WindowsPowerShell', 'v1.0', 'powershell.exe');
async function processes(): Promise<ProcessInfo[]> {
  const stdout = await new Promise<string>((resolve, reject) => execFile(powershell,
    ['-NoProfile', '-NonInteractive', '-Command', 'Get-CimInstance Win32_Process | Select-Object ProcessId,ParentProcessId,Name,CreationDate | ConvertTo-Json -Compress'],
    { windowsHide: true, timeout: 15000, maxBuffer: 4 * 1024 * 1024 }, (error, stdout) => error ? reject(error) : resolve(stdout)));
  const result = JSON.parse(stdout); return Array.isArray(result) ? result : [result];
}
async function ownedTree(pid: number): Promise<ProcessInfo[]> {
  const all = await processes();
  const owned = new Set([pid]);
  for (let changed = true; changed;) {
    changed = false;
    for (const process of all) if (owned.has(process.ParentProcessId) && !owned.has(process.ProcessId)) { owned.add(process.ProcessId); changed = true; }
  }
  return all.filter(process => owned.has(process.ProcessId));
}
async function assertGone(owned: ProcessInfo[]): Promise<void> {
  const now = await processes();
  assert.deepEqual(now.filter(current => owned.some(old => old.ProcessId === current.ProcessId && old.CreationDate === current.CreationDate)), [], 'Owned CLI descendants survived awaited close');
}
async function findCodexExe(root: string): Promise<string | undefined> {
  for (const entry of await readdir(root, { withFileTypes: true })) {
    const path = join(root, entry.name);
    if (entry.isFile() && entry.name.toLowerCase() === 'codex.exe') return path;
    if (entry.isDirectory()) { const found = await findCodexExe(path); if (found) return found; }
  }
}
async function codex(command: string, unexpected = false) {
  let closed = 0;
  let notify!: () => void;
  const closedEvent = new Promise<void>(resolve => { notify = resolve; });
  const transport = await CodexTransport.start({ command, cwd: directory, requestTimeoutMs: 20000, onClose() { closed++; notify(); } });
  let owned: ProcessInfo[] = [];
  try {
    owned = await ownedTree(transport.pid!);
    const result = await transport.sendRequest<{ data: unknown[] }>('model/list', { limit: 100 });
    assert.ok(Array.isArray(result.data));
    if (unexpected) {
      const native = owned.find(process => process.Name.toLowerCase() === 'codex.exe');
      assert.ok(native, 'Official native Codex descendant must be visible');
      process.kill(native.ProcessId, 'SIGKILL');
      await deadline(closedEvent, 'Codex unexpected exit');
      await assert.rejects(transport.sendRequest('model/list', {}));
    }
  } finally { await transport.close(); }
  assert.equal(closed, 1);
  await assertGone(owned);
  receipts.push({ backend: 'codex', command, unexpected, initialized: true, owned });
}
async function pi(command: string, unexpected = false) {
  let notify!: () => void;
  const closedEvent = new Promise<void>(resolve => { notify = resolve; });
  const events: string[] = [];
  const transport = await ExternalPiTransport.start({ command, cwd: directory, sessionDir: directory,
    extensionPath: resolve('dist/agents/external-pi-extension.js'), requestTimeoutMs: 20000,
    onEvent: event => { events.push(event.type); }, onClose() { notify(); } });
  let owned: ProcessInfo[] = [];
  try {
    owned = await ownedTree(transport.pid!);
    const state = await transport.request<{ sessionFile: string; sessionId: string; isStreaming: boolean }>('get_state');
    assert.equal(state.isStreaming, false); assert.ok(state.sessionFile.includes(' ')); assert.ok(state.sessionId);
    const messages = await transport.request<{ messages: unknown[] }>('get_messages');
    assert.deepEqual(messages.messages, []);
    if (unexpected) {
      const native = [...owned].reverse().find(process => process.ProcessId !== transport.pid && process.Name.toLowerCase() === 'node.exe');
      assert.ok(native, 'Official Pi Node descendant must be visible');
      process.kill(native.ProcessId, 'SIGKILL');
      await deadline(closedEvent, 'Pi unexpected exit');
      await assert.rejects(transport.request('get_state'));
    }
  } finally { await transport.close(); }
  assert.equal(events.includes('agent_start'), false);
  await assertGone(owned);
  receipts.push({ backend: 'pi-rpc', command, unexpected, initialized: true, modelPrompts: 0, owned });
}
async function deadline<T>(operation: Promise<T>, label: string): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  try { return await Promise.race([operation, new Promise<never>((_, reject) => { timer = setTimeout(() => reject(new Error(`${label} timed out`)), 20000); })]); }
  finally { clearTimeout(timer); }
}
try {
  for (const command of ['codex', join(prefix, 'codex.cmd')]) await codex(command);
  for (const command of ['pi', join(prefix, 'pi.cmd')]) await pi(command);
  const nativeCodex = await findCodexExe(join(prefix, 'node_modules', '@openai'));
  assert.ok(nativeCodex, 'Official native Codex .exe not found in the npm prefix');
  await codex(nativeCodex);
  await codex(join(prefix, 'codex.cmd'), true);
  await pi(join(prefix, 'pi.cmd'), true);
  // Synthetic stubborn native process tree proves forced /T cleanup separately from official controls.
  const fixture = join(directory, 'stubborn tree.cjs');
  await writeFile(fixture, `const {spawn}=require('node:child_process'); const child=spawn(process.execPath,['-e','setInterval(()=>{},1000)'],{stdio:'inherit'}); console.log(JSON.stringify({pid:process.pid,child:child.pid})); process.stdin.resume(); setInterval(()=>{},1000);`);
  const owned = new OwnedCliProcess(process.execPath, [fixture], { cwd: directory });
  let tree: ProcessInfo[] = [];
  try {
    owned.child.stderr.resume(); owned.child.stdin.on('error', () => {});
    await deadline(new Promise<void>((resolve, reject) => { owned.child.stdout.once('data', () => resolve()); owned.child.once('error', reject); }), 'stubborn tree readiness');
    tree = await ownedTree(owned.child.pid!);
    assert.ok(tree.length >= 2);
  } finally { await owned.close(25); }
  await assertGone(tree);
  receipts.push({ scope: 'synthetic native Node process tree, not CLI/model behavior', forcedTreeTermination: true, owned: tree });
  await mkdir(resolve('../../.artifacts'), { recursive: true });
  await writeFile(evidence, JSON.stringify({ pass: true, platform: process.platform, inference: false, installed, directory, receipts,
    unverified: 'Descendants after arbitrary external force-kill of the root wrapper are not recoverable by taskkill /T once its PID is gone.' }, null, 2));
  console.log(JSON.stringify({ pass: true, evidence, cases: receipts.length, inference: false }));
} catch (error) {
  await mkdir(resolve('../../.artifacts'), { recursive: true });
  await writeFile(evidence, JSON.stringify({ pass: false, platform: process.platform, directory, error: String(error), receipts }, null, 2));
  throw error;
} finally { if (previousPath === undefined) delete process.env.PATH; else process.env.PATH = previousPath; }

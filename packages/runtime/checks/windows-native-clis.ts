/** Windows only: real official CLI controls, never creates a thread/turn or sends a model prompt. */
import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { mkdir, mkdtemp, readFile, readdir, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
if (process.platform !== 'win32') throw new Error('This check requires actual Windows; static support is not a Windows verification');
// Exercise the built transport and its .js Job host, as shipped in the runtime package.
const { CodexTransport } = await import(new URL('../dist/agents/codex-transport.js', import.meta.url).href) as typeof import('../src/agents/codex-transport.js');
const { ExternalPiTransport } = await import(new URL('../dist/agents/external-pi-transport.js', import.meta.url).href) as typeof import('../src/agents/external-pi-transport.js');
const { OwnedCliProcess } = await import(new URL('../dist/agents/owned-cli-process.js', import.meta.url).href) as typeof import('../src/agents/owned-cli-process.js');
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
type ObservedTransport = { readonly pid: number | undefined; readonly lifecycleDiagnostics: unknown; close(): Promise<void> };
async function closeObserved(transport: ObservedTransport, receipt: Record<string, unknown>, owned: ProcessInfo[], mark: (stage: string, details?: Record<string, unknown>) => void): Promise<ProcessInfo[]> {
  try {
    const before = await ownedTree(transport.pid!);
    receipt.beforeStdinEofProcesses = before;
    owned = [...new Map([...owned, ...before].map(process => [`${process.ProcessId}:${process.CreationDate}`, process])).values()];
  } catch (error) { receipt.beforeStdinEofSnapshotError = String(error); }
  mark('close.requested', { lifecycle: transport.lifecycleDiagnostics });
  try { await transport.close(); mark('close.resolved'); }
  catch (error) {
    receipt.closeError = String(error);
    mark('close.rejected', { lifecycle: transport.lifecycleDiagnostics });
    try {
      const now = await processes();
      receipt.survivingOriginalProcesses = now.filter(current => owned.some(old => old.ProcessId === current.ProcessId && old.CreationDate === current.CreationDate));
      receipt.currentProcessesLinkedToOriginalPid = await ownedTree(transport.pid!);
    } catch (snapshotError) { receipt.afterFailureSnapshotError = String(snapshotError); }
    throw error;
  } finally { receipt.lifecycle = transport.lifecycleDiagnostics; }
  return owned;
}
function beginCase(backend: string, command: string, unexpected: boolean) {
  const stages: Record<string, unknown>[] = [];
  const receipt: Record<string, unknown> = { backend, command, unexpected, stages };
  receipts.push(receipt);
  const mark = (stage: string, details: Record<string, unknown> = {}) => {
    stages.push({ stage, at: new Date().toISOString(), ...details });
    console.log(JSON.stringify({ backend, stage, command, unexpected }));
  };
  return { receipt, mark };
}
async function codex(command: string, unexpected = false) {
  const { receipt, mark } = beginCase('codex', command, unexpected);
  let closed = 0;
  let notify!: () => void;
  const closedEvent = new Promise<void>(resolve => { notify = resolve; });
  mark('initialize.requested');
  const transport = await CodexTransport.start({ command, cwd: directory, requestTimeoutMs: 20000, onClose(info) { closed++; mark('transport.onClose', { code: info.code, signal: info.signal }); notify(); } });
  receipt.initialized = true;
  mark('initialize.resolved', { lifecycle: transport.lifecycleDiagnostics });
  let owned: ProcessInfo[] = [];
  try {
    owned = await ownedTree(transport.pid!); receipt.afterInitializeProcesses = owned;
    mark('model/list.requested');
    const result = await transport.sendRequest<{ data: unknown[] }>('model/list', { limit: 100 });
    assert.ok(Array.isArray(result.data));
    mark('model/list.resolved', { modelCount: result.data.length });
    if (unexpected) {
      const native = owned.find(process => process.Name.toLowerCase() === 'codex.exe');
      assert.ok(native, 'Official native Codex descendant must be visible');
      mark('native.kill.requested', { pid: native.ProcessId });
      process.kill(native.ProcessId, 'SIGKILL');
      await deadline(closedEvent, 'Codex unexpected exit');
      await assert.rejects(transport.sendRequest('model/list', {}));
    }
  } catch (error) { receipt.operationError = String(error); throw error; }
  finally { owned = await closeObserved(transport, receipt, owned, mark); }
  assert.equal(closed, 1);
  await assertGone(owned);
  receipt.owned = owned; receipt.pass = true;
}
async function pi(command: string, unexpected = false) {
  const { receipt, mark } = beginCase('pi-rpc', command, unexpected);
  let notify!: () => void;
  const closedEvent = new Promise<void>(resolve => { notify = resolve; });
  const events: string[] = [];
  mark('initialize.requested');
  const transport = await ExternalPiTransport.start({ command, cwd: directory, sessionDir: directory,
    extensionPath: resolve('dist/agents/external-pi-extension.js'), requestTimeoutMs: 20000,
    onEvent: event => { events.push(event.type); }, onClose() { mark('transport.onClose'); notify(); } });
  receipt.initialized = true;
  mark('initialize.resolved', { lifecycle: transport.lifecycleDiagnostics });
  let owned: ProcessInfo[] = [];
  try {
    owned = await ownedTree(transport.pid!); receipt.afterInitializeProcesses = owned;
    mark('get_state.requested');
    const state = await transport.request<{ sessionFile: string; sessionId: string; isStreaming: boolean }>('get_state');
    assert.equal(state.isStreaming, false); assert.ok(state.sessionFile.includes(' ')); assert.ok(state.sessionId);
    mark('get_state.resolved');
    const messages = await transport.request<{ messages: unknown[] }>('get_messages');
    assert.deepEqual(messages.messages, []);
    mark('get_messages.resolved');
    if (unexpected) {
      const native = [...owned].reverse().find(process => process.ProcessId !== transport.pid && process.Name.toLowerCase() === 'node.exe');
      assert.ok(native, 'Official Pi Node descendant must be visible');
      mark('native.kill.requested', { pid: native.ProcessId });
      process.kill(native.ProcessId, 'SIGKILL');
      await deadline(closedEvent, 'Pi unexpected exit');
      await assert.rejects(transport.request('get_state'));
    }
  } catch (error) { receipt.operationError = String(error); throw error; }
  finally { owned = await closeObserved(transport, receipt, owned, mark); }
  assert.equal(events.includes('agent_start'), false);
  await assertGone(owned);
  receipt.owned = owned; receipt.pass = true; receipt.modelPrompts = 0;
}
async function deadline<T>(operation: Promise<T>, label: string): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  try { return await Promise.race([operation, new Promise<never>((_, reject) => { timer = setTimeout(() => reject(new Error(`${label} timed out`)), 20000); })]); }
  finally { clearTimeout(timer); }
}
async function naturalRootExitFixture(): Promise<void> {
  // Explicitly detached child escapes Node's ordinary child job, but must remain in our session Job.
  const fixture = join(directory, 'natural root leaves detached child.cjs');
  await writeFile(fixture, `const {spawn}=require('node:child_process'); const child=spawn(process.execPath,['-e','setInterval(()=>{},1000)'],{stdio:'inherit',detached:true,windowsHide:true}); child.unref(); console.log(JSON.stringify({pid:process.pid,child:child.pid})); process.stdin.resume(); process.stdin.once('end',()=>{process.exitCode=17;});`);
  const owned = new OwnedCliProcess(process.execPath, [fixture], { cwd: directory });
  const receipt: Record<string, unknown> = { scope: 'synthetic natural root exit with detached inherited-stdio child; no CLI/model behavior' };
  receipts.push(receipt);
  let tree: ProcessInfo[] = [];
  try {
    owned.child.stderr.resume(); owned.child.stdin.on('error', () => {});
    await deadline(new Promise<void>((resolve, reject) => { owned.child.stdout.once('data', () => resolve()); owned.child.once('error', reject); }), 'natural root fixture readiness');
    tree = await ownedTree(owned.child.pid!); receipt.beforeStdinEofProcesses = tree;
    assert.ok(tree.length >= 3, 'Launcher, native fixture root and detached child must all be present');
  } finally { try { await owned.close(); } finally { receipt.lifecycle = owned.diagnostics; } }
  assert.equal(owned.child.exitCode, 17, 'Launcher must naturally preserve the native root exit code');
  assert.equal(owned.diagnostics.events.some(event => event.type === 'grace.expired'), false, 'Natural Job cleanup must not rely on forced shutdown');
  await assertGone(tree);
  receipt.pass = true; receipt.naturalExitCode = 17; receipt.owned = tree;
}
try {
  await naturalRootExitFixture();
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
  // Kill only the launcher with stdin still open: /T must not be what reaps its descendants.
  const killed = new OwnedCliProcess(process.execPath, [fixture], { cwd: directory });
  const killedReceipt: Record<string, unknown> = { scope: 'synthetic launcher-only death with stubborn child and grandchild; no CLI/model behavior', killMethod: 'process.kill(launcher PID, SIGKILL), no /T, before stdin EOF' };
  receipts.push(killedReceipt);
  let killedTree: ProcessInfo[] = [];
  try {
    killed.child.stderr.resume(); killed.child.stdin.on('error', () => {});
    await deadline(new Promise<void>((resolve, reject) => { killed.child.stdout.once('data', () => resolve()); killed.child.once('error', reject); }), 'launcher-death fixture readiness');
    killedTree = await ownedTree(killed.child.pid!);
    assert.ok(killedTree.filter(process => process.Name.toLowerCase() === 'node.exe').length >= 3, 'Launcher, stubborn child and grandchild must be present');
    killedReceipt.beforeKillProcesses = killedTree;
    killedReceipt.beforeKillLifecycle = killed.diagnostics;
    assert.equal(killed.diagnostics.stdin.finished, false, 'Do not close stdin before killing the launcher');
    process.kill(killed.child.pid!, 'SIGKILL');
    await killed.close();
    assert.equal(killed.diagnostics.events.some(event => event.type === 'grace.expired'), false, 'Launcher-only death must not rely on taskkill /T');
    await assertGone(killedTree);
    killedReceipt.pass = true;
  } finally { try { await killed.close(); } finally { killedReceipt.lifecycle = killed.diagnostics; } }

  await mkdir(resolve('../../.artifacts'), { recursive: true });
  await writeFile(evidence, JSON.stringify({ pass: true, platform: process.platform, inference: false, runtimeArtifact: 'compiled dist/agents/*.js including owned-windows-cli-host.js', installed, directory, receipts,
    ownership: 'Windows-only launcher self-assigns a kill-on-close Job before spawning the original CLI. Actual natural/abnormal exit and PID+creation-time cleanup assertions retained.' }, null, 2));
  console.log(JSON.stringify({ pass: true, evidence, cases: receipts.length, inference: false }));
} catch (error) {
  await mkdir(resolve('../../.artifacts'), { recursive: true });
  await writeFile(evidence, JSON.stringify({ pass: false, platform: process.platform, directory, error: String(error), receipts }, null, 2));
  throw error;
} finally { if (previousPath === undefined) delete process.env.PATH; else process.env.PATH = previousPath; }

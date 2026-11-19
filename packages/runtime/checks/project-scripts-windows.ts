/** Native Windows cmd/PTY/JSONL acceptance. No model, account, GUI, or inferred cwd from terminal text. */
import assert from 'node:assert/strict';
import {execFileSync, spawn} from 'node:child_process';
import {createHash, randomUUID} from 'node:crypto';
import {once} from 'node:events';
import {access, mkdir, mkdtemp, readFile, readdir, realpath, rename, rm, writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {basename, dirname, join, resolve, win32} from 'node:path';
import {createInterface} from 'node:readline';
import {fileURLToPath} from 'node:url';
import {Script} from 'node:vm';
import {resolveSavedScriptLaunch} from '@tt-a1i/hive/dist/src/server/workspace-shell-runtime.js';
import type {ProjectScriptRun} from '../src/project-script-runs.js';

const workerProgram = String.raw`
const fs = require('node:fs'), cp = require('node:child_process'), path = require('node:path');
function parentPid() {
 const powershell = path.join(process.env.SystemRoot || 'C:\\Windows', 'System32', 'WindowsPowerShell', 'v1.0', 'powershell.exe');
 const query = '$row = Get-CimInstance Win32_Process -Filter "ProcessId=' + process.pid + '"; if ($null -eq $row) {exit 73}; [Console]::Out.Write($row.ParentProcessId)';
 let output;
 try {output = cp.execFileSync(powershell, ['-NoProfile', '-NonInteractive', '-Command', query], {encoding: 'utf8', windowsHide: true, timeout: 15000, stdio: ['ignore', 'pipe', 'pipe']}).trim();}
 catch {throw Error('Win32 process parent observation failed');}
 const pid = Number(output);
 if (!/^[1-9][0-9]*$/.test(output) || !Number.isSafeInteger(pid)) throw Error('Win32 process parent observation returned an invalid PID');
 return pid;
}
const [mode, arg] = process.argv.slice(2), cwd = fs.realpathSync(process.cwd()), observedParent = parentPid();
const observed = {cwd, cmdPid: observedParent, nodePid: process.pid, nodeReportedParentPid: process.ppid, parentPidSource: 'Win32_Process.ParentProcessId'};
if (mode === 'report') {
 fs.appendFileSync('fixture-counter.txt', 'once\n'); fs.writeFileSync('fixture-report.json', JSON.stringify({...observed, tag: arg}));
} else if (mode === 'interactive') {
 fs.writeFileSync(arg, JSON.stringify(observed));
} else if (mode === 'child') {
 fs.writeFileSync(arg + '.child.json', JSON.stringify({cwd, pid: process.pid, parentPid: observedParent, parentPidSource: observed.parentPidSource})); setInterval(() => {}, 1000);
} else if (mode === 'tree') {
 const child = cp.spawn(process.execPath, [__filename, 'child', arg], {stdio: 'inherit', detached: false});
 child.on('error', () => process.exit(71)); fs.writeFileSync(arg + '.json', JSON.stringify({...observed, childPid: child.pid})); setInterval(() => {}, 1000);
} else process.exit(72);
`;
new Script(workerProgram, {filename: 'script-worker.cjs'}); // Parse only; native execution remains Windows-only.

type ProcessReport = {cwd: string; cmdPid: number; nodePid: number; childPid?: number; parentPidSource: string; nodeReportedParentPid?: number};
function report(value: unknown): ProcessReport {
 assert.ok(value && typeof value === 'object'); const row = value as ProcessReport;
 assert.equal(typeof row.cwd, 'string'); assert.ok(win32.isAbsolute(row.cwd)); assert.equal(row.parentPidSource, 'Win32_Process.ParentProcessId');
 for (const pid of [row.cmdPid, row.nodePid, ...(row.childPid === undefined ? [] : [row.childPid])]) assert.ok(Number.isSafeInteger(pid) && pid > 0);
 assert.notEqual(row.cmdPid, row.nodePid); if (row.childPid !== undefined) assert.ok(row.childPid !== row.nodePid && row.childPid !== row.cmdPid);
 return row;
}
function sameDirectory(actual: string, expected: string) {assert.equal(win32.normalize(actual.trim()).toLowerCase(), win32.normalize(expected).toLowerCase());}
const nativePlan = resolveSavedScriptLaunch('exit /b 23', 'C:\\fixture', {ComSpec: 'C:\\Windows\\System32\\cmd.exe'}, 'win32');
assert.deepEqual(nativePlan, {command: 'C:\\Windows\\System32\\cmd.exe', args: ['/d', '/s', '/c', 'exit /b 23'], cwd: 'C:\\fixture'});
assert.throws(() => resolveSavedScriptLaunch('echo fixture', '\\\\server\\share', {}, 'win32'), /SCRIPT_UNSUPPORTED_CWD/);
assert.equal(report({cwd: 'C:\\fixture', cmdPid: 101, nodePid: 102, childPid: 103, parentPidSource: 'Win32_Process.ParentProcessId'}).childPid, 103);
assert.throws(() => report({cwd: 'C:\\fixture', cmdPid: 0, nodePid: 102, parentPidSource: 'Win32_Process.ParentProcessId'}));
assert.throws(() => report({cwd: 'relative', cmdPid: 101, nodePid: 102, parentPidSource: 'Win32_Process.ParentProcessId'}));
sameDirectory('c:\\FIXTURE\\tree\r\n', 'C:\\fixture\\tree');
const receiptIndex = process.argv.indexOf('--receipt');
if (receiptIndex >= 0 && !process.argv[receiptIndex + 1]) throw Error('--receipt requires a path');
const receiptPath = receiptIndex < 0 ? undefined : resolve(process.argv[receiptIndex + 1]!);
const receipt: Record<string, unknown> = {platform: process.platform, pass: false, realWindowsPty: false, launchAndReportContract: true, workerSyntax: true, parentPidSource: 'Win32_Process.ParentProcessId', noModelOrAccount: true};
async function writeReceipt() {if (receiptPath) {await mkdir(dirname(receiptPath), {recursive: true}); await writeFile(receiptPath, JSON.stringify(receipt, null, 2));}}
if (process.platform !== 'win32') {
 receipt.skipped = 'Windows native cmd/PTY validation requires Windows'; await writeReceipt();
 console.log('PROJECT_SCRIPTS_WINDOWS_SKIPPED: launch/report contracts passed; no Windows native acceptance performed.');
} else {
 const shell = process.env.ComSpec ?? process.env.COMSPEC ?? 'cmd.exe'; assert.equal(basename(shell).toLowerCase(), 'cmd.exe');
 const directory = await realpath(await mkdtemp(join(tmpdir(), 'hive-script-win-')));
 const project = join(directory, 'project space'), other = join(directory, 'other space'), agent = join(directory, 'agent'), hold = join(directory, 'hold'), data = join(directory, 'state');
 await Promise.all([project, other, agent, hold].map(path => mkdir(path)));
 const environment = {...process.env}; for (const key of Object.keys(environment)) if (key.toUpperCase().startsWith('GIT_')) delete environment[key];
 const gitconfig = join(directory, 'gitconfig'); await writeFile(gitconfig, '');
 Object.assign(environment, {GIT_CONFIG_GLOBAL: gitconfig, GIT_CONFIG_NOSYSTEM: '1', GIT_TERMINAL_PROMPT: '0', PI_CODING_AGENT_DIR: agent, PI_OFFLINE: '1', ComSpec: shell});
 const git = (...args: string[]) => execFileSync('git', ['-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.test', '-c', 'commit.gpgSign=false', ...args], {cwd: project, env: environment, stdio: 'pipe'});
 git('init', '--template=', '--initial-branch=main'); await writeFile(join(project, 'same.txt'), 'ROOT'); git('add', '.'); git('commit', '-m', 'fixture');
 // Commands are fixture constants and generated fixture paths only, never user input.
 const quote = (path: string) => {assert.ok(!/["%!\r\n]/.test(path), 'Fixture paths must not invoke cmd expansion'); return `"${path}"`;};
 const worker = join(directory, 'script-worker.cjs');
 await writeFile(worker, workerProgram);
 // Prevent the unrelated startup update check from contacting the npm registry.
 const guard = join(directory, 'offline.cjs');
 await writeFile(guard, `const native=globalThis.fetch;globalThis.fetch=(input,...rest)=>{const url=new URL(typeof input==='string'||input instanceof URL?input:input.url);if(!['127.0.0.1','localhost','[::1]'].includes(url.hostname))return Promise.reject(Error('Windows fixture blocks outbound fetch'));return native(input,...rest);};`);
 const entry = process.env.HIVE_DESKTOP_TEST_RUNTIME ?? fileURLToPath(new URL('../dist/main.js', import.meta.url));
 const runtimeEnv: NodeJS.ProcessEnv = {...environment, HIVE_DESKTOP_DATA_DIR: data};
 for (const key of Object.keys(runtimeEnv)) if (key.toUpperCase() === 'PATH') delete runtimeEnv[key];
 runtimeEnv.PATH = dirname(process.execPath) + ';' + (environment.PATH ?? environment.Path ?? '');
 const child = spawn(process.execPath, ['--require', guard, entry], {cwd: directory, stdio: ['pipe','pipe','pipe'], env: runtimeEnv});
 const exited = once(child, 'exit'); let sequence = 0, diagnostics = ''; const pending = new Map<number, {resolve(value: any): void; reject(error: Error): void}>();
 const terminalEvents: any[] = []; child.stderr.on('data', bytes => {diagnostics += String(bytes);});
 const lines = createInterface({input: child.stdout});
 lines.on('line', line => {const frame = JSON.parse(line), request = pending.get(frame.id); if (request) {pending.delete(frame.id); request.resolve(frame);} else if (frame.event === 'terminal') terminalEvents.push(frame.data);});
 child.on('exit', () => {for (const request of pending.values()) request.reject(Error('Runtime exited before response: ' + diagnostics)); pending.clear();});
 const frame = (method: string, params: Record<string, unknown> = {}) => new Promise<any>((resolveValue, reject) => {const id = ++sequence, timer = setTimeout(() => {pending.delete(id); reject(Error('RPC timed out: ' + method));}, 30000);
  pending.set(id, {resolve: value => {clearTimeout(timer); resolveValue(value);}, reject: error => {clearTimeout(timer); reject(error);}}); child.stdin.write(JSON.stringify({id, method, params}) + '\n');});
 const rpc = async (method: string, params: Record<string, unknown> = {}) => {const result = await frame(method, params); if (result.error) throw Error(`${result.error.code}: ${result.error.message}`); return result.result;};
 const wait = async <T>(read: () => T | Promise<T>, ready: (value: T) => boolean, label: string): Promise<T> => {const until = Date.now() + 20000; for (;;) {const value = await read(); if (ready(value)) return value; if (Date.now() > until) throw Error('Timed out: ' + label); await new Promise(resolveValue => setTimeout(resolveValue, 25));}};
 const readJson = (file: string) => wait(async () => {try {return JSON.parse(await readFile(file, 'utf8'));} catch (error) {if ((error as NodeJS.ErrnoException).code === 'ENOENT' || error instanceof SyntaxError) return undefined; throw error;}}, value => value !== undefined, file);
 const alive = (pid: number) => {try {process.kill(pid, 0); return true;} catch (error) {if ((error as NodeJS.ErrnoException).code === 'ESRCH') return false; throw error;}};
 const owned = new Set<number>();
 const remember = (value: ProcessReport) => {for (const pid of [value.cmdPid, value.nodePid, ...(value.childPid === undefined ? [] : [value.childPid])]) owned.add(pid); return value;};
 const treePids = (value: ProcessReport) => [value.cmdPid, value.nodePid, value.childPid!];
 const allAlive = (pids: number[]) => pids.every(alive);
 const assertGone = (pids: number[], label: string) => {for (const pid of pids) assert.equal(alive(pid), false, `${label}: PID ${pid} must already have exited`);};
 const complete = (workspaceId: string, runId: string) => wait(async () => (await rpc('script.get', {workspaceId, runId, offset: 0, limit: 1})).run as ProjectScriptRun, run => !['starting','running'].includes(run.status), 'native script exit');
 const save = async (workspaceId: string, name: string, command: string) => {const snapshot = await rpc('script.settings.list', {workspaceId}); const next = await rpc('script.settings.save', {workspaceId, expectedRevision: snapshot.revision, script: {name, command}}); return next.scripts.at(-1);};
 const request = (workspaceId: string, script: {id: string; version: string}, sessionId?: string) => ({workspaceId, scriptId: script.id, expectedVersion: script.version, requestId: randomUUID(), ...(sessionId === undefined ? {} : {sessionId})});
 async function tree(workspaceId: string, name: string, sessionId?: string, moveToHold = false) {
  const prefix = join(directory, name), command = `${moveToHold ? `cd /d ${quote(hold)} && ` : ''}node.exe ${quote(worker)} tree ${quote(prefix)}`;
  const script = await save(workspaceId, name, command), input = request(workspaceId, script, sessionId), run = await rpc('script.run', input) as ProjectScriptRun;
  const processes = remember(report(await readJson(prefix + '.json'))), descendant = await readJson(prefix + '.child.json');
  assert.ok(Number.isSafeInteger(processes.childPid)); assert.equal(descendant.pid, processes.childPid); assert.equal(descendant.parentPid, processes.nodePid); assert.equal(descendant.parentPidSource, 'Win32_Process.ParentProcessId'); assert.ok(allAlive(treePids(processes)));
  assert.equal(typeof run.nativeRunId, 'string'); const native = await rpc('hive.request', {method: 'GET', path: `/api/runtime/runs/${run.nativeRunId}`});
  if (native.pid !== processes.cmdPid) console.error('WINDOWS_SCRIPT_PID_DIAGNOSTIC', JSON.stringify({kind: 'script', nativeRunId: run.nativeRunId, nativePid: native.pid, cimParentPid: processes.cmdPid, nodePid: processes.nodePid, nodeReportedParentPid: processes.nodeReportedParentPid}));
  assert.equal(native.pid, processes.cmdPid, 'Win32 CIM parent must be the actual Hive-owned cmd PTY'); assert.equal(native.agent_id, `${workspaceId}:script:${run.runId}:shell`);
  if (moveToHold) sameDirectory(processes.cwd, await realpath(hold));
  return {run, input, processes};
 }
 async function interactive(workspaceId: string, name: string) {
  const run = await rpc('terminal.start', {workspaceId}), attachmentId = randomUUID();
  await rpc('terminal.attach', {workspaceId, runId: run.runId, attachmentId, cols: 100, rows: 30});
  const restored = await wait(() => terminalEvents.find(event => event.attachmentId === attachmentId && event.type === 'restore') as {workspaceId: string; runId: string; sequence: number} | undefined, value => value !== undefined, 'interactive restore');
  assert.ok(restored); assert.equal(restored.workspaceId, workspaceId); assert.equal(restored.runId, run.runId); assert.ok(Number.isSafeInteger(restored.sequence));
  await rpc('terminal.restoreComplete', {attachmentId, sequence: restored.sequence});
  const output = join(directory, name + '.json'); await rpc('terminal.input', {attachmentId, bytes: [...Buffer.from(`node.exe ${quote(worker)} interactive ${quote(output)}\r\n`)]});
  const result = report(await readJson(output)); owned.add(result.cmdPid); assert.ok(alive(result.cmdPid));
  const native = await rpc('hive.request', {method: 'GET', path: `/api/runtime/runs/${run.runId}`});
  if (native.pid !== result.cmdPid) console.error('WINDOWS_SCRIPT_PID_DIAGNOSTIC', JSON.stringify({kind: 'interactive', nativeRunId: run.runId, nativePid: native.pid, cimParentPid: result.cmdPid, nodePid: result.nodePid, nodeReportedParentPid: result.nodeReportedParentPid}));
  assert.equal(native.pid, result.cmdPid, 'Win32 CIM parent must be the actual Hive-owned interactive cmd PTY');
  // Consume raw output for native flow control. Its ANSI text proves neither cwd nor PID.
  for (const event of terminalEvents.filter(event => event.attachmentId === attachmentId && event.type === 'output')) await rpc('terminal.ack', {attachmentId, sequence: event.sequence, bytes: event.bytes.length});
  await rpc('terminal.detach', {attachmentId}); return result.cmdPid;
 }
 let shutdown = false;
 try {
  await rpc('initialize');
  const ws = await rpc('hive.request', {method: 'POST', path: '/api/workspaces', body: {path: project, name: 'Windows scripts', autostart_orchestrator: false}});
  const otherWs = await rpc('hive.request', {method: 'POST', path: '/api/workspaces', body: {path: other, name: 'Other Windows scripts', autostart_orchestrator: false}});
  const a = await rpc('chat.createRecord', {workspaceId: ws.id, backend: 'codex', isolation: 'worktree'}), b = await rpc('chat.createRecord', {workspaceId: ws.id, backend: 'codex', isolation: 'worktree'});
  await writeFile(join(a.execution.path, 'same.txt'), 'A'); await writeFile(join(b.execution.path, 'same.txt'), 'B');
  const observations: Record<string, unknown>[] = [];
  for (const [tag, cwd, sessionId] of [['ROOT', project, undefined], ['A', a.execution.path, a.id], ['B', b.execution.path, b.id]] as const) {
   const command = `cd>fixture-cwd.txt && type same.txt>fixture-read.txt && >fixture-relative.txt echo ${tag} && node.exe ${quote(worker)} report ${tag}`;
   const script = await save(ws.id, tag, command), input = request(ws.id, script, sessionId), started = await rpc('script.run', input), done = await complete(ws.id, started.runId);
   assert.equal(done.status, 'succeeded'); assert.equal(done.exitCode, 0); sameDirectory(done.cwd, await realpath(cwd));
   sameDirectory(await readFile(join(cwd, 'fixture-cwd.txt'), 'utf8'), await realpath(cwd));
   assert.equal(await readFile(join(cwd, 'fixture-read.txt'), 'utf8'), tag); assert.equal((await readFile(join(cwd, 'fixture-relative.txt'), 'utf8')).trim(), tag);
   const observed = report(await readJson(join(cwd, 'fixture-report.json'))); sameDirectory(observed.cwd, await realpath(cwd)); assertGone([observed.cmdPid, observed.nodePid], 'completed script');
   assert.deepEqual(await rpc('script.run', input), done); assert.equal(await readFile(join(cwd, 'fixture-counter.txt'), 'utf8'), 'once\n');
   observations.push({tag, cwd: done.cwd, exitCode: done.exitCode, cmdPid: observed.cmdPid});
  }
  const failedScript = await save(ws.id, 'Native cmd nonzero', 'exit /b 23'), failedStart = await rpc('script.run', request(ws.id, failedScript));
  const failed = await complete(ws.id, failedStart.runId); assert.equal(failed.status, 'failed'); assert.equal(failed.exitCode, 23);
  const mainShellPid = await interactive(ws.id, 'main-interactive'), otherShellPid = await interactive(otherWs.id, 'other-interactive');
  const keep = await tree(otherWs.id, 'keep-other'), cancelled = await tree(ws.id, 'cancel-A', a.id, true);
  sameDirectory(cancelled.run.cwd, await realpath(a.execution.path));
  const receiptName = createHash('sha256').update(JSON.stringify([ws.id, cancelled.input.requestId])).digest('hex') + '.json';
  await rm(join(data, 'project-script-runs', receiptName));
  const replay = await rpc('script.run', cancelled.input);
  assert.equal(replay.runId, cancelled.run.runId);
  assert.equal((await rpc('script.get', {workspaceId: ws.id, runId: replay.runId})).run.nativeRunId, cancelled.run.nativeRunId);
  // cmd and its descendants deliberately moved to hold, so Windows no longer locks A.
  // This permits an actual missing directory, rather than merely hiding .git metadata.
  await rename(a.execution.path, a.execution.path + '.missing');
  try {
   const missing = await frame('script.run', {...cancelled.input, requestId: randomUUID()}); assert.ok(missing.error); assert.equal(missing.result, undefined);
   const stopped = await rpc('script.stop', {workspaceId: ws.id, runId: cancelled.run.runId}); assert.equal(stopped.status, 'cancelled'); assert.equal(stopped.cancelRequested, true); assert.ok(Number.isInteger(stopped.exitCode));
   assertGone(treePids(cancelled.processes), 'stop with missing source'); assert.ok(allAlive([...treePids(keep.processes), mainShellPid, otherShellPid]));
  } finally {await rename(a.execution.path + '.missing', a.execution.path);}
  const rootTree = await tree(ws.id, 'delete-root'), bTree = await tree(ws.id, 'delete-B', b.id);
  await rpc('workspace.remove', {workspaceId: ws.id});
  assertGone([...treePids(rootTree.processes), ...treePids(bTree.processes), mainShellPid], 'workspace deletion'); assert.ok(allAlive([...treePids(keep.processes), otherShellPid]));
  for (const run of [rootTree.run, bTree.run]) {const stopped = (await rpc('script.get', {workspaceId: ws.id, runId: run.runId, offset: 0, limit: 1})).run; assert.equal(stopped.status, 'cancelled'); assert.ok(Number.isInteger(stopped.exitCode));}
  await rpc('shutdown'); shutdown = true; assertGone([...treePids(keep.processes), otherShellPid], 'runtime close'); assert.equal((await exited)[0], 0, diagnostics);
  const persisted = await Promise.all((await readdir(join(data, 'project-script-runs'))).filter(name => name.endsWith('.json')).map(async name => JSON.parse(await readFile(join(data, 'project-script-runs', name), 'utf8'))));
  const final = persisted.find(run => run.runId === keep.run.runId); assert.ok(final); assert.equal(final.status, 'cancelled'); assert.equal(final.cancelRequested, true); assert.ok(Number.isInteger(final.exitCode));
  assert.equal(await readFile(join(project, 'same.txt'), 'utf8'), 'ROOT'); assert.equal(await readFile(join(a.execution.path, 'same.txt'), 'utf8'), 'A'); assert.equal(await readFile(join(b.execution.path, 'same.txt'), 'utf8'), 'B');
  Object.assign(receipt, {pass: true, realWindowsPty: true, entry, shell: 'cmd.exe', observations, sameRequestOnce: true, liveReplayAfterReceiptLoss: true, nativeNonzeroExitCode: 23,
   cancellationWithActuallyMissingSource: true, workspaceDeletionWaitsForPidTrees: true, closeWaitsForPidTrees: true, otherWorkspaceAndInteractivePreserved: true});
  console.log('PROJECT_SCRIPTS_WINDOWS_OK: actual cmd/PTY/JSONL, root/A/B file IO, native exit codes, idempotency, stop/delete/close PID trees; no models or accounts.');
 } catch (error) {receipt.error = error instanceof Error ? error.stack ?? error.message : String(error); receipt.diagnostics = diagnostics.slice(-12000); throw error;}
 finally {
  if (!shutdown && child.exitCode === null && child.signalCode === null) {child.stdin.end(); await Promise.race([exited, new Promise(resolveValue => setTimeout(resolveValue, 20000))]);}
  let forcedCleanup = false;
  for (const pid of owned) if (alive(pid)) {forcedCleanup = true; try {execFileSync('taskkill.exe', ['/PID', String(pid), '/T', '/F'], {stdio: 'ignore'});} catch {}}
  if (child.exitCode === null && child.signalCode === null) {forcedCleanup = true; try {execFileSync('taskkill.exe', ['/PID', String(child.pid), '/T', '/F'], {stdio: 'ignore'});} catch {}}
  lines.close(); if (forcedCleanup) {receipt.pass = false; receipt.forcedCleanup = true;}
  await writeReceipt(); await rm(directory, {recursive: true, force: true, maxRetries: 10, retryDelay: 250});
  if (forcedCleanup && !receipt.error) throw Error('Fixture required forced cleanup; native process ownership did not pass');
 }
}

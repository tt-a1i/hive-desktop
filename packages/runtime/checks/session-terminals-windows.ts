/** Windows: real Git worktrees, Hive, PTYs and WebSockets. No agent or model. */
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { mkdir, mkdtemp, readFile, realpath, rename, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { createRequire } from 'node:module';
import { basename, dirname, join, resolve } from 'node:path';
import { createDesktopRuntime } from '../src/desktop-runtime.js';
import type { TerminalEvent } from '../src/terminal-connection.js';

// Resolve the same headless terminal implementation already owned by Hive.
// No direct workspace dependency or alternate VT parser is introduced.
const ownRequire = createRequire(import.meta.url);
const hiveRequire = createRequire(ownRequire.resolve('@tt-a1i/hive/package.json'));
const { Terminal } = hiveRequire('@xterm/headless');
const { SerializeAddon } = hiveRequire('@xterm/addon-serialize');
function newScreen(cols = 120, rows = 35) {
 return new Terminal({cols, rows, scrollback: 10000, allowProposedApi: true});
}
async function feed(screen: InstanceType<typeof Terminal>, bytes: Uint8Array | string) {
 await new Promise<void>(resolve => screen.write(bytes, resolve));
}
function screenLines(screen: InstanceType<typeof Terminal>): string[] {
 const buffer = screen.buffer.active;
 const lines: string[] = [];
 for (let index = 0; index < buffer.length; index++) {
  const line = buffer.getLine(index)!;
  // Full preceding cells matter when the following line is a soft wrap.
  const nextWrapped = buffer.getLine(index + 1)?.isWrapped ?? false;
  const text = line.translateToString(!nextWrapped);
  if (line.isWrapped && lines.length) lines[lines.length - 1] += text;
  else lines.push(text);
 }
 return lines;
}
function observation(screen: InstanceType<typeof Terminal>, tag: string): {cwd: string; content: string} | undefined {
 const lines = screenLines(screen);
 const end = lines.findIndex(line => line.trim() === `${tag}_END`);
 const data = lines.findIndex(line => line.trim() === `${tag}_DATA`);
 const cwd = lines.findIndex(line => line.startsWith(`${tag}_PWD:`));
 if (cwd < 0 || data !== cwd + 1 || end !== data + 2) return;
 return {cwd: lines[cwd]!.slice(`${tag}_PWD:`.length).trim(), content: lines[data + 1]!.trim()};
}
// Exercise real VT cursor overwrite and soft wrapping. A command echo alone
// cannot satisfy the independent output marker checks.
const sample = newScreen(12, 8);
const serializer = new SerializeAddon(); sample.loadAddon(serializer);
try {
 await feed(sample, 'echo T_END\r\n');
 assert.equal(observation(sample, 'T'), undefined);
 await feed(sample, 'T_PWD:C:\\long path\\tree\r\nT_DATA\r\nrooX');
 await feed(sample, '\bt\r\nT_END\r\n');
 assert.deepEqual(observation(sample, 'T'), {cwd: 'C:\\long path\\tree', content: 'root'});
 const restoredSample = newScreen(12, 8);
 try { await feed(restoredSample, serializer.serialize()); assert.deepEqual(observation(restoredSample, 'T'), {cwd: 'C:\\long path\\tree', content: 'root'}); }
 finally { restoredSample.dispose(); }
} finally { sample.dispose(); }
// A real Windows stream captured before resize includes absolute 80x24 cursor
// coordinates. Replay it without an initial snapshot and verify headless VT
// handling; it must not be mistaken for a complete parsed observation.
const rawRegression = newScreen(80, 24);
try {
 await feed(rawRegression, await readFile(new URL('./fixtures/windows-conpty-resize-80x24.bin', import.meta.url)));
 const lines = screenLines(rawRegression);
 assert.ok(lines.some(line => line.trim() === 'HIVE5c68d3d0b36d46a79a8a76af0db9f52a_DATA'));
 assert.ok(lines.some(line => line.trim() === 'one'));
 assert.equal(observation(rawRegression, 'HIVE5c68d3d0b36d46a79a8a76af0db9f52a'), undefined);
} finally { rawRegression.dispose(); }

const receiptArg = process.argv.indexOf('--receipt');
if (receiptArg >= 0 && !process.argv[receiptArg + 1]) throw Error('--receipt requires a path');
const receiptPath = receiptArg < 0 ? undefined : resolve(process.argv[receiptArg + 1]!);
const receipt: Record<string, unknown> = { platform: process.platform, realPty: false, pass: false, noModel: true };
async function writeReceipt() {
 if (receiptPath) { await mkdir(dirname(receiptPath), {recursive: true}); await writeFile(receiptPath, JSON.stringify(receipt, null, 2)); }
}
if (process.platform !== 'win32') {
 receipt.skipped = 'Windows native PTY validation requires Windows';
 await writeReceipt();
 console.log('SESSION_TERMINALS_WINDOWS_SKIPPED: requires Windows; no native PTY validation performed.');
} else {
 const shell = process.env.ComSpec ?? process.env.COMSPEC ?? 'cmd.exe';
 assert.equal(basename(shell).toLowerCase(), 'cmd.exe', 'Fixture requires the actual Windows cmd shell');
 const directory = await realpath(await mkdtemp(join(tmpdir(), 'hive-session-terminals-win-')));
 const repo = join(directory, 'repo'), data = join(directory, 'state'), agent = join(directory, 'agent');
 await mkdir(repo); await mkdir(agent);
 const oldAgent = process.env.PI_CODING_AGENT_DIR, oldOffline = process.env.PI_OFFLINE;
 process.env.PI_CODING_AGENT_DIR = agent; process.env.PI_OFFLINE = '1';
 const git = (...args: string[]) => execFileSync('git', ['-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.test', '-c', 'commit.gpgSign=false', ...args], {cwd: repo, stdio: 'pipe'});
 const events: TerminalEvent[] = [];
 let runtime: Awaited<ReturnType<typeof createDesktopRuntime>> | undefined;
 const pids: number[] = [];
 const screens = new Map<string, InstanceType<typeof Terminal>>();
 const rawOutput = new Map<string, Buffer>();
 const restoreOutput = new Map<string, Buffer>();
 const resizeStatus = new Map<string, {cols: number; rows: number; sent: boolean}>();
 const wait = async <T>(read: () => T | Promise<T>, ready: (value: T) => boolean, label: string): Promise<T> => {
  const end = Date.now() + 15000;
  for (;;) { const value = await read(); if (ready(value)) return value; if (Date.now() > end) throw Error('Timed out: ' + label); await new Promise(resolve => setTimeout(resolve, 25)); }
 };
 const alive = (pid: number) => { try { process.kill(pid, 0); return true; } catch (error) { if ((error as NodeJS.ErrnoException).code === 'ESRCH') return false; throw error; } };
 try {
  git('init', '--template='); await writeFile(join(repo, 'same.txt'), 'root'); git('add', '.'); git('commit', '-m', 'fixture');
  runtime = await createDesktopRuntime(data, () => {}, {executableEnvironment: {}, onTerminalEvent: event => events.push(event)});
  const terminal = runtime.terminal;
  const workspace = await runtime.hive.request('POST', '/api/workspaces', {path: repo, name: 'windows-session-terminals', autostart_orchestrator: false}) as {id: string};
  const one = await runtime.chats.createRecord(workspace.id, 'codex', {isolation: 'worktree'});
  const two = await runtime.chats.createRecord(workspace.id, 'codex', {isolation: 'worktree'});
  const a = await runtime.chats.executionDirectory(workspace.id, one.id), b = await runtime.chats.executionDirectory(workspace.id, two.id);
  await writeFile(join(a, 'same.txt'), 'one'); await writeFile(join(b, 'same.txt'), 'two');
  const root = await terminal.start(workspace.id), arun = await terminal.start(workspace.id, one.id), brun = await terminal.start(workspace.id, two.id);
  assert.equal(new Set([root.runId, arun.runId, brun.runId]).size, 3);
  for (const [sessionId, run] of [[undefined, root], [one.id, arun], [two.id, brun]] as const) {
   const list = await terminal.list(workspace.id, sessionId);
   assert.equal(list.workspaceId, workspace.id); assert.equal(list.sessionId, sessionId);
   assert.deepEqual(list.runs.map(item => item.runId), [run.runId]);
  }
  const positions = new Map<string, number>();
  async function drain(id: string) {
   const screen = screens.get(id); assert.ok(screen);
   const fresh = events.slice(positions.get(id) ?? 0); positions.set(id, events.length);
   for (const event of fresh) if (event.attachmentId === id && event.type === 'output') {
    const bytes = Buffer.from(event.bytes);
    rawOutput.set(id, Buffer.concat([rawOutput.get(id) ?? Buffer.alloc(0), bytes]).subarray(-16000));
    await feed(screen, bytes);
    await terminal.ack({attachmentId: id, sequence: event.sequence, bytes: event.bytes.length});
   }
   return screen;
  }
  const input = (id: string, command: string) => terminal.input({attachmentId: id, bytes: [...Buffer.from(command + '\r\n')]});
  async function attach(runId: string, sessionId?: string) {
   const id = randomUUID();
   await terminal.attach({workspaceId: workspace.id, ...(sessionId === undefined ? {} : {sessionId}), runId, attachmentId: id, cols: 120, rows: 35});
   const event = await wait(() => events.find(item => item.attachmentId === id && item.type === 'restore'), Boolean, 'restore');
   assert.ok(event?.type === 'restore'); assert.equal(event.workspaceId, workspace.id); assert.equal(event.sessionId, sessionId); assert.equal(event.runId, runId);
   const screen = newScreen(event.cols, event.rows); screens.set(id, screen);
   const snapshot = Buffer.from(event.bytes); restoreOutput.set(id, snapshot.subarray(-16000));
   await feed(screen, snapshot);
   await terminal.restoreComplete({attachmentId: id, sequence: event.sequence});
   // Match the desktop client: send the view dimensions after restore is ready.
   // This is a socket-send acknowledgement, not proof that the PTY has applied it.
   resizeStatus.set(id, {cols: event.cols, rows: event.rows, sent: false});
   await terminal.resize({attachmentId: id, cols: event.cols, rows: event.rows});
   resizeStatus.set(id, {cols: event.cols, rows: event.rows, sent: true});
   return {id, screen};
  }
  async function inspect(id: string, path: string, content: string) {
   const tag = 'HIVE' + randomUUID().replaceAll('-', '');
   await input(id, '@echo off');
   await input(id, `echo ${content}>terminal-relative.txt & echo ${tag}_PWD:%CD% & echo ${tag}_DATA & type same.txt & echo. & echo ${tag}_END`);
   let result: {cwd: string; content: string} | undefined;
   try {
    result = await wait(async () => observation(await drain(id), tag), Boolean, 'cmd directory and file');
    assert.ok(result);
    assert.equal(result.cwd.toLowerCase(), path.toLowerCase(), 'Shell must use the requested directory');
   } catch (error) {
    // This owned fixture contains only generated paths/commands, not account data.
    console.error('WINDOWS_PTY_CWD_DIAGNOSTIC', JSON.stringify({tag, expected: path, resize: resizeStatus.get(id), snapshotBase64: (restoreOutput.get(id) ?? Buffer.alloc(0)).toString('base64'), rawBase64: (rawOutput.get(id) ?? Buffer.alloc(0)).toString('base64'), screenLines: screenLines(screens.get(id)!).slice(-50).map(line => line.slice(0, 500))}));
    throw error;
   }
   assert.ok(result);
   assert.equal(result.content, content, 'Shell must read its own same-named file');
   assert.equal((await readFile(join(path, 'terminal-relative.txt'), 'utf8')).trim(), content);
   return tag;
  }
  async function shellPid(id: string) {
   const tag = 'PID' + randomUUID().replaceAll('-', '');
   await input(id, `powershell.exe -NoProfile -NonInteractive -Command "$p=(Get-CimInstance Win32_Process -Filter ('ProcessId=' + $PID)).ParentProcessId; Write-Output ('${tag}:' + $p)"`);
   const pattern = new RegExp(`^${tag}:([0-9]+)$`);
   const result = await wait(async () => screenLines(await drain(id)).map(line => line.trim()).find(line => pattern.test(line)), Boolean, 'shell process id');
   assert.ok(result); const pid = Number(result.match(pattern)![1]);
   assert.ok(Number.isSafeInteger(pid) && pid > 0 && alive(pid)); pids.push(pid); return pid;
  }
  const rootView = await attach(root.runId); await inspect(rootView.id, repo, 'root'); const rootPid = await shellPid(rootView.id);
  const first = await attach(arun.runId, one.id); const marker = await inspect(first.id, a, 'one'); const aPid = await shellPid(first.id);
  await terminal.detach({attachmentId: first.id});
  const second = await attach(brun.runId, two.id); await inspect(second.id, b, 'two'); const bPid = await shellPid(second.id);
  assert.equal(new Set([rootPid, aPid, bPid]).size, 3);
  const restored = await attach(arun.runId, one.id); assert.ok(observation(restored.screen, marker), 'Reattached shell must restore completed prior output');
  await assert.rejects(terminal.attach({workspaceId: workspace.id, sessionId: two.id, runId: arun.runId, attachmentId: randomUUID(), cols: 80, rows: 24}), /workspace and session/);
  await assert.rejects(terminal.attach({workspaceId: workspace.id, runId: arun.runId, attachmentId: randomUUID(), cols: 80, rows: 24}), /workspace and session/);
  await assert.rejects(terminal.stop({workspaceId: workspace.id, sessionId: two.id, runId: arun.runId}), /workspace and session/);
  await assert.rejects(terminal.stop({workspaceId: workspace.id, runId: arun.runId}), /workspace and session/);
  await terminal.stop({workspaceId: workspace.id, sessionId: two.id, runId: brun.runId});
  await wait(() => !alive(bPid), Boolean, 'B shell process exit');
  assert.deepEqual((await terminal.list(workspace.id, two.id)).runs, []);
  await inspect(rootView.id, repo, 'root'); await inspect(restored.id, a, 'one');
  assert.ok(alive(rootPid) && alive(aPid));
  await terminal.detach({attachmentId: restored.id});
  // Hide the worktree's Git pointer rather than renaming a directory held by cmd.exe.
  const gitPointer = join(a, '.git'); await rename(gitPointer, gitPointer + '.missing');
  try {
   const unavailable = await terminal.list(workspace.id, one.id);
   assert.equal(unavailable.workspaceId, workspace.id); assert.equal(unavailable.sessionId, one.id);
   assert.equal(typeof unavailable.sourceUnavailable, 'string'); assert.deepEqual(unavailable.runs.map(run => run.runId), [arun.runId]);
   await assert.rejects(terminal.start(workspace.id, one.id));
   await assert.rejects(terminal.attach({workspaceId: workspace.id, sessionId: one.id, runId: arun.runId, attachmentId: randomUUID(), cols: 80, rows: 24}));
   await assert.rejects(terminal.stop({workspaceId: workspace.id, sessionId: two.id, runId: arun.runId}), /workspace and session/);
   await terminal.stop({workspaceId: workspace.id, sessionId: one.id, runId: arun.runId});
   await wait(() => !alive(aPid), Boolean, 'A shell process exit with unavailable source');
   await assert.rejects(terminal.list(workspace.id, one.id));
  } finally { await rename(gitPointer + '.missing', gitPointer); }
  await runtime.close(); runtime = undefined;
  await wait(() => pids.every(pid => !alive(pid)), Boolean, 'all shell processes exit');
  assert.ok(events.filter(event => event.type === 'closed').every(event => event.workspaceId === workspace.id));
  assert.equal(await readFile(join(repo, 'same.txt'), 'utf8'), 'root');
  Object.assign(receipt, {pass: true, realPty: true, shell: 'cmd.exe', worktrees: 2, distinctShells: 3,
   relativeFiles: true, sessionOwnership: true, reconnectSnapshot: true, independentStop: true,
   unavailableSourceCleanup: true, allProcessesExited: true});
  console.log('SESSION_TERMINALS_WINDOWS_OK: real cmd PTYs, worktrees, ownership, reconnect and process cleanup; no agent or model.');
 } finally {
  try { await runtime?.close(); }
  finally {
   for (const screen of screens.values()) screen.dispose();
   if (oldAgent === undefined) delete process.env.PI_CODING_AGENT_DIR; else process.env.PI_CODING_AGENT_DIR = oldAgent;
   if (oldOffline === undefined) delete process.env.PI_OFFLINE; else process.env.PI_OFFLINE = oldOffline;
   await writeReceipt();
   await rm(directory, {recursive: true, force: true, maxRetries: 10, retryDelay: 250});
  }
 }
}

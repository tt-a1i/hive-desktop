/** Windows: real Git worktrees, Hive, PTYs and WebSockets. No agent or model. */
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { mkdir, mkdtemp, readFile, realpath, rename, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { basename, dirname, join, resolve } from 'node:path';
import { createDesktopRuntime } from '../src/desktop-runtime.js';
import type { TerminalEvent } from '../src/terminal-connection.js';

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
  const positions = new Map<string, number>(), texts = new Map<string, string>();
  async function drain(id: string) {
   const fresh = events.slice(positions.get(id) ?? 0); positions.set(id, events.length);
   for (const event of fresh) if (event.attachmentId === id && event.type === 'output') {
    texts.set(id, (texts.get(id) ?? '') + Buffer.from(event.bytes).toString());
    await terminal.ack({attachmentId: id, sequence: event.sequence, bytes: event.bytes.length});
   }
   return texts.get(id) ?? '';
  }
  const input = (id: string, command: string) => terminal.input({attachmentId: id, bytes: [...Buffer.from(command + '\r\n')]});
  async function attach(runId: string, sessionId?: string) {
   const id = randomUUID();
   await terminal.attach({workspaceId: workspace.id, ...(sessionId === undefined ? {} : {sessionId}), runId, attachmentId: id, cols: 120, rows: 35});
   const event = await wait(() => events.find(item => item.attachmentId === id && item.type === 'restore'), Boolean, 'restore');
   assert.ok(event?.type === 'restore'); assert.equal(event.workspaceId, workspace.id); assert.equal(event.sessionId, sessionId); assert.equal(event.runId, runId);
   await terminal.restoreComplete({attachmentId: id, sequence: event.sequence});
   return {id, snapshot: Buffer.from(event.bytes).toString()};
  }
  async function inspect(id: string, path: string, content: string) {
   const tag = 'HIVE' + randomUUID().replaceAll('-', '');
   await input(id, '@echo off');
   await input(id, `echo ${content}>terminal-relative.txt & echo ${tag}_PWD:%CD% & echo ${tag}_DATA & type same.txt & echo. & echo ${tag}_END`);
   const result = await wait(() => drain(id), text => text.includes(`${tag}_END`), 'cmd directory and file');
   assert.ok(result.toLowerCase().includes(`${tag}_PWD:${path}`.toLowerCase()), `Wrong shell directory: ${result}`);
   assert.match(result, new RegExp(`${tag}_DATA\\s*${content}\\s*${tag}_END`));
   assert.equal((await readFile(join(path, 'terminal-relative.txt'), 'utf8')).trim(), content);
   return tag;
  }
  async function shellPid(id: string) {
   const tag = 'PID' + randomUUID().replaceAll('-', '');
   await input(id, `powershell.exe -NoProfile -NonInteractive -Command "$p=(Get-CimInstance Win32_Process -Filter ('ProcessId=' + $PID)).ParentProcessId; Write-Output ('${tag}:' + $p)"`);
   const result = await wait(() => drain(id), text => new RegExp(`${tag}:[0-9]+`).test(text), 'shell process id');
   const pid = Number(result.match(new RegExp(`${tag}:([0-9]+)`))![1]);
   assert.ok(Number.isSafeInteger(pid) && pid > 0 && alive(pid)); pids.push(pid); return pid;
  }
  const rootView = await attach(root.runId); await inspect(rootView.id, repo, 'root'); const rootPid = await shellPid(rootView.id);
  const first = await attach(arun.runId, one.id); const marker = await inspect(first.id, a, 'one'); const aPid = await shellPid(first.id);
  await terminal.detach({attachmentId: first.id});
  const second = await attach(brun.runId, two.id); await inspect(second.id, b, 'two'); const bPid = await shellPid(second.id);
  assert.equal(new Set([rootPid, aPid, bPid]).size, 3);
  const restored = await attach(arun.runId, one.id); assert.ok(restored.snapshot.includes(marker), 'Reattached shell must restore prior output');
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
   if (oldAgent === undefined) delete process.env.PI_CODING_AGENT_DIR; else process.env.PI_CODING_AGENT_DIR = oldAgent;
   if (oldOffline === undefined) delete process.env.PI_OFFLINE; else process.env.PI_OFFLINE = oldOffline;
   await writeReceipt();
   await rm(directory, {recursive: true, force: true, maxRetries: 10, retryDelay: 250});
  }
 }
}

/** Real Git worktrees + Hive PTYs and paired sockets. Does not launch an agent or model. */
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { mkdir, mkdtemp, readFile, realpath, rename, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createDesktopRuntime } from '../src/desktop-runtime.js';
import type { TerminalEvent } from '../src/terminal-connection.js';
import { Terminals } from '../src/terminal.js';
if (process.platform === 'win32') throw Error('POSIX shell fixture is not Windows native validation');
const directory = await realpath(await mkdtemp(join(tmpdir(), 'hive-session-terminals-')));
console.log('SESSION_TERMINALS_EVIDENCE', directory);
const repo = join(directory, 'repo'), data = join(directory, 'state');
await mkdir(repo);
const git = (...args: string[]) => execFileSync('git', ['-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.test', '-c', 'commit.gpgSign=false', ...args], {cwd: repo, stdio: 'pipe'});
git('init', '--template='); await writeFile(join(repo, 'same.txt'), 'root'); git('add', '.'); git('commit', '-m', 'fixture');
const events: TerminalEvent[] = [];
const runtime = await createDesktopRuntime(data, () => {}, {executableEnvironment: {}, onTerminalEvent: event => events.push(event)});
const terminal = runtime.terminal;
const wait = async <T>(read: () => T | Promise<T>, ready: (value: T) => boolean, label: string): Promise<T> => {
 const end = Date.now() + 10000;
 for (;;) { const value = await read(); if (ready(value)) return value; if (Date.now() > end) throw Error('Timed out: ' + label); await new Promise(resolve => setTimeout(resolve, 10)); }
};
const positions = new Map<string, number>(), texts = new Map<string, string>();
async function drain(id: string) {
 const fresh = events.slice(positions.get(id) ?? 0); positions.set(id, events.length);
 for (const event of fresh) if (event.attachmentId === id && event.type === 'output') {
  texts.set(id, (texts.get(id) ?? '') + Buffer.from(event.bytes).toString());
  await terminal.ack({attachmentId: id, sequence: event.sequence, bytes: event.bytes.length});
 }
 return texts.get(id) ?? '';
}
const input = (attachmentId: string, command: string) => terminal.input({attachmentId, bytes: [...Buffer.from(command + '\n')]});
const octal = (text: string) => [...Buffer.from(text)].map(byte => '\\' + byte.toString(8).padStart(3, '0')).join('');
const pids: number[] = [];
let closed = false;
try {
 const workspace = await runtime.hive.request('POST', '/api/workspaces', {path: repo, name: 'session-terminals', autostart_orchestrator: false}) as {id: string};
 const one = await runtime.chats.createRecord(workspace.id, 'codex', {isolation: 'worktree'});
 const two = await runtime.chats.createRecord(workspace.id, 'codex', {isolation: 'worktree'});
 const a = await runtime.chats.executionDirectory(workspace.id, one.id), b = await runtime.chats.executionDirectory(workspace.id, two.id);
 await writeFile(join(a, 'same.txt'), 'one'); await writeFile(join(b, 'same.txt'), 'two');
 assert.deepEqual(await terminal.list(workspace.id, one.id), {workspaceId: workspace.id, sessionId: one.id, runs: []});
 assert.deepEqual(await terminal.list(workspace.id), {workspaceId: workspace.id, runs: []});
 await assert.rejects(terminal.list(workspace.id, 'missing-session'));
 // Once a shell has spawned, returning its identity must not re-resolve its source.
 let resolutions = 0;
 const onceResolved = new Terminals(runtime.hive, id => runtime.chats.assertWorkspaceAvailable(id), () => {}, async (id, sessionId) => {
  if (++resolutions > 1) throw Error('source became unavailable after spawn');
  return runtime.chats.executionDirectory(id, sessionId);
 });
 const onceRun = await onceResolved.start(workspace.id, one.id); assert.equal(resolutions, 1);
 await onceResolved.stop({workspaceId: workspace.id, sessionId: one.id, runId: onceRun.runId}); await onceResolved.close();
 // If the live registry read fails after spawn, the just-created PTY must be removed.
 let failRegistryRead = true; let failedPid: number | null = null;
 const failedStartup = new Terminals({...runtime.hive, startSessionShell: async (id, context) => {
  const run = await runtime.hive.startSessionShell(id, context) as {runId: string; pid: number | null}; failedPid = run.pid; return run;
 }, request: async (method, path, body) => {
  if (method === 'GET' && path.endsWith('/runs') && failRegistryRead) {failRegistryRead = false; throw Error('post-spawn registry read fixture');}
  return runtime.hive.request(method, path, body);
 }}, id => runtime.chats.assertWorkspaceAvailable(id), () => {}, (id, sessionId) => runtime.chats.executionDirectory(id, sessionId));
 await assert.rejects(failedStartup.start(workspace.id, one.id), /post-spawn registry read fixture/);
 assert.deepEqual(await terminal.list(workspace.id, one.id), {workspaceId: workspace.id, sessionId: one.id, runs: []});
 assert.ok(Number.isInteger(failedPid) && failedPid! > 0);
 await wait(() => {try {process.kill(failedPid!, 0); return false;} catch (error) {if ((error as NodeJS.ErrnoException).code === 'ESRCH') return true; throw error;}}, Boolean, 'failed startup shell process exit');
 await failedStartup.close();
 const root = await terminal.start(workspace.id), arun = await terminal.start(workspace.id, one.id), brun = await terminal.start(workspace.id, two.id);
 assert.equal(new Set([root.runId, arun.runId, brun.runId]).size, 3);
 const workspaces = await runtime.hive.request('GET', '/api/workspaces') as {id: string; path: string}[];
 assert.equal(workspaces.length, 1); assert.equal(workspaces[0]!.id, workspace.id); assert.equal(workspaces[0]!.path, repo);
 assert.deepEqual((await terminal.list(workspace.id)).runs.map(run => run.runId), [root.runId]);
 assert.deepEqual((await terminal.list(workspace.id, one.id)).runs.map(run => run.runId), [arun.runId]);
 assert.deepEqual((await terminal.list(workspace.id, two.id)).runs.map(run => run.runId), [brun.runId]);
 const attach = async (runId: string, sessionId?: string) => {
  const id = randomUUID();
  await terminal.attach({workspaceId: workspace.id, ...(sessionId === undefined ? {} : {sessionId}), runId, attachmentId: id, cols: 100, rows: 30});
  const restore = await wait(() => events.find(event => event.attachmentId === id && event.type === 'restore'), Boolean, 'restore');
  assert.ok(restore?.type === 'restore'); assert.equal(restore.sessionId, sessionId);
  await terminal.restoreComplete({attachmentId: id, sequence: restore.sequence});
  return {id, snapshot: Buffer.from(restore.bytes).toString()};
 };
 const inspect = async (id: string, path: string, content: string) => {
  const tag = 'OBS_' + randomUUID();
  await input(id, `stty -echo; printf '${octal(content)}' > terminal-relative.txt; printf '${octal(tag)}:%s:%s:%s\\n' "$$" "$(pwd -P)" "$(cat same.txt)"`);
  const result = await wait(() => drain(id), value => value.includes(`${tag}:`) && value.includes(`:${path}:${content}`), 'directory and relative file');
  const match = result.match(new RegExp(tag + ':(\\d+):')); assert.ok(match); pids.push(Number(match[1])); assert.equal(await readFile(join(path, 'terminal-relative.txt'), 'utf8'), content); return tag;
 };
 const rootView = await attach(root.runId); await inspect(rootView.id, repo, 'root');
 const first = await attach(arun.runId, one.id); const aMarker = await inspect(first.id, a, 'one');
 await terminal.detach({attachmentId: first.id});
 const second = await attach(brun.runId, two.id); await inspect(second.id, b, 'two');
 const restored = await attach(arun.runId, one.id); assert.ok(restored.snapshot.includes(aMarker)); await inspect(restored.id, a, 'one');
 await assert.rejects(terminal.attach({workspaceId: workspace.id, sessionId: two.id, runId: arun.runId, attachmentId: randomUUID(), cols: 80, rows: 24}), /workspace and session/);
 await assert.rejects(terminal.attach({workspaceId: workspace.id, runId: arun.runId, attachmentId: randomUUID(), cols: 80, rows: 24}), /workspace and session/);
 await assert.rejects(terminal.stop({workspaceId: workspace.id, sessionId: two.id, runId: arun.runId}), /workspace and session/);
 await assert.rejects(terminal.stop({workspaceId: workspace.id, runId: arun.runId}), /workspace and session/);
 await terminal.stop({workspaceId: workspace.id, sessionId: two.id, runId: brun.runId});
 assert.deepEqual(await terminal.list(workspace.id, two.id), {workspaceId: workspace.id, sessionId: two.id, runs: []});
 await inspect(rootView.id, repo, 'root'); await inspect(restored.id, a, 'one');
 await rename(b, b + '.missing');
 try {
  await assert.rejects(terminal.list(workspace.id, two.id), /worktree|工作树|ENOENT/i);
  await assert.rejects(terminal.start(workspace.id, two.id), /worktree|工作树|ENOENT/i);
  await mkdir(b);
  try { await assert.rejects(terminal.list(workspace.id, two.id), /worktree|工作树|git/i); await assert.rejects(terminal.start(workspace.id, two.id), /worktree|工作树|git/i); }
  finally { await rm(b, {recursive: true}); }
 }
 finally { await rename(b + '.missing', b); }
 assert.deepEqual(await terminal.list(workspace.id, two.id), {workspaceId: workspace.id, sessionId: two.id, runs: []});
 // Cleaning up an owned shell must remain possible after its view and source disappear.
 await terminal.detach({attachmentId: restored.id});
 await rename(a, a + '.missing');
 try {
  const unavailable = await terminal.list(workspace.id, one.id);
  assert.equal(unavailable.workspaceId, workspace.id); assert.equal(unavailable.sessionId, one.id); assert.equal(typeof unavailable.sourceUnavailable, 'string');
  assert.deepEqual(unavailable.runs.map(run => run.runId), [arun.runId]);
  await assert.rejects(terminal.start(workspace.id, one.id), /worktree|工作树|ENOENT/i);
  await assert.rejects(terminal.attach({workspaceId: workspace.id, sessionId: one.id, runId: arun.runId, attachmentId: randomUUID(), cols: 80, rows: 24}), /worktree|工作树|ENOENT/i);
  await assert.rejects(terminal.stop({workspaceId: workspace.id, sessionId: two.id, runId: arun.runId}), /workspace and session/);
  await assert.doesNotReject(terminal.stop({workspaceId: workspace.id, sessionId: one.id, runId: arun.runId}), 'detached owned shell cleanup must not need its worktree');
  await assert.rejects(terminal.list(workspace.id, one.id), /worktree|工作树|ENOENT/i);
 } finally {await rename(a + '.missing', a);}
 const liveB = await terminal.start(workspace.id, two.id); const finalB = await attach(liveB.runId, two.id); await inspect(finalB.id, b, 'two');
 await runtime.removeWorkspace(workspace.id);
 await wait(() => pids.every(pid => {try {process.kill(pid, 0); return false;} catch (error) {if ((error as NodeJS.ErrnoException).code === 'ESRCH') return true; throw error;}}), Boolean, 'all workspace shells exit');
 assert.ok(events.filter(event => event.type === 'closed').every(event => event.workspaceId === workspace.id));
 await runtime.close(); closed = true;
 // Snapshot restore requires a live background and PTY. A new background has no original live shells.
 const reopened = await createDesktopRuntime(data, () => {}, {executableEnvironment: {}});
 try {
  const freshWorkspace = await reopened.hive.request('POST', '/api/workspaces', {path: repo, name: 'closed-background', autostart_orchestrator: false}) as {id: string};
  const live = await reopened.terminal.start(freshWorkspace.id);
  await reopened.close();
  const again = await createDesktopRuntime(data, () => {}, {executableEnvironment: {}});
  try {assert.equal((await again.terminal.list(freshWorkspace.id)).runs.length, 0); await assert.rejects(again.terminal.attach({workspaceId: freshWorkspace.id, runId: live.runId, attachmentId: randomUUID(), cols: 80, rows: 24}));}
  finally {await again.close();}
 } finally {await reopened.close();}
 assert.equal(await readFile(join(repo, 'same.txt'), 'utf8'), 'root');
 await writeFile(join(directory, 'receipt.json'), JSON.stringify({pass: true, noModel: true, realPty: true, managedWorktreeCwd: true, relativeFileIsolation: true, sessionOwnership: true, listIdentityEcho: true, listValidatesExecutionDirectory: true, detachSnapshot: true, independentStop: true, detachedMissingSourceStop: true, sourceUnavailableListsOnlyOwnedLiveRuns: true, postSpawnFailureCleanup: true, missingWorktreeRejected: true, workspaceRemovalAllShells: true, backgroundRestartHasNoLiveShells: true, windowsNativeValidated: false}, null, 2));
 console.log('SESSION_TERMINALS_OK Evidence:', directory);
} finally {if (!closed) await runtime.close();}

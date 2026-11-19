/** Actual desktop JSONL/Hive plus real local bare Git; gh/GitHub reads are controlled process fixtures, not account validation. */
import assert from 'node:assert/strict';
import { execFileSync, spawn } from 'node:child_process';
import { once } from 'node:events';
import { createRequire } from 'node:module';
import { access, chmod, mkdir, mkdtemp, readFile, realpath, rename, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createInterface } from 'node:readline';
import { fileURLToPath } from 'node:url';

if (process.platform === 'win32') throw Error('This fixture uses POSIX executable wrappers and hooks; it is not Windows native validation');
const directory = await realpath(await mkdtemp(join(tmpdir(), 'hive-git-remotes-wire-')));
console.log('GIT_REMOTES_WIRE_EVIDENCE', directory);
const repo = join(directory, 'project'), outside = join(directory, 'unrelated'), bare = join(directory, 'remote.git'), foreignBare = join(directory, 'unrelated.git');
const bin = join(directory, 'trusted-bin'), agent = join(directory, 'agent'), statePath = join(directory, 'peer-state.json'), log = join(directory, 'peer-calls.jsonl');
const entered = join(directory, 'push-entered'), release = join(directory, 'push-release'), finished = join(directory, 'push-finished');
await Promise.all([repo, outside, bin, agent].map(path => mkdir(path)));
const env = {...process.env};
for (const key of Object.keys(env)) if (key.toUpperCase().startsWith('GIT_')) delete env[key];
const config = join(directory, 'gitconfig'); await writeFile(config, ''); Object.assign(env, {GIT_CONFIG_GLOBAL: config, GIT_CONFIG_NOSYSTEM: '1', GIT_TERMINAL_PROMPT: '0'});
const gitPath = execFileSync('/usr/bin/which', ['git'], {env, encoding: 'utf8'}).trim();
const git = (cwd: string, ...args: string[]) => execFileSync(gitPath, ['-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.test', '-c', 'commit.gpgSign=false', ...args], {cwd, env, encoding: 'utf8', stdio: 'pipe'}).trim();
for (const [cwd, remote] of [[repo, bare], [outside, foreignBare]]) {
 git(cwd!, 'init', '--template=', '--initial-branch=main'); git(cwd!, 'init', '--bare', '--template=', remote!); git(cwd!, 'remote', 'add', 'origin', remote!);
 await writeFile(join(cwd!, '.gitignore'), '.hive/\n'); await writeFile(join(cwd!, 'file.txt'), 'base'); git(cwd!, 'add', '.'); git(cwd!, 'commit', '-m', 'base');
}
git(repo, 'remote', 'add', 'github', 'https://user:wire-private@tail-secret@github.com/fixture-owner/fixture-repo.git');
const rootHead = git(repo, 'rev-parse', 'HEAD'), rootIndex = await readFile(join(repo, '.git/index')), unrelatedIndex = await readFile(join(outside, '.git/index'));
let peerState: any = {git: gitPath, log, baseOid: rootHead, remoteOid: null, exists: false, mode: 'normal', title: '', body: ''};
const savePeer = () => writeFile(statePath, JSON.stringify(peerState)); const readPeer = async () => {peerState = JSON.parse(await readFile(statePath, 'utf8'));}; await savePeer(); await writeFile(log, '');
const peer = fileURLToPath(new URL('./fixtures/git-remotes-wire-peer.cjs', import.meta.url));
const quote = (value: string) => "'" + value.replaceAll("'", "'\\''") + "'";
for (const name of ['git', 'gh']) {const wrapper = join(bin, name); await writeFile(wrapper, `#!/bin/sh\nexec ${quote(process.execPath)} ${quote(peer)} ${name} "$@"\n`); await chmod(wrapper, 0o700);}
const forbidden = join(directory, 'FORBIDDEN_PAYLOAD_PROGRAM_RAN'), badProgram = join(directory, 'payload-program');
await writeFile(badProgram, `#!/bin/sh\nprintf wrong > ${quote(forbidden)}\nexit 1\n`); await chmod(badProgram, 0o700);
// Hive checks npm updates at startup; block outbound Node traffic in this test
// process while leaving the actual loopback HTTP/WebSocket runtime untouched.
const networkGuard = join(directory, 'no-outbound-network.cjs');
await writeFile(networkGuard, `const net=require('node:net');const local=h=>h===undefined||['localhost','127.0.0.1','::1','[::1]'].includes(h);const connect=net.Socket.prototype.connect;net.Socket.prototype.connect=function(...a){const first=Array.isArray(a[0])?a[0][0]:a[0];const host=first&&typeof first==='object'?first.host:typeof first==='number'&&typeof a[1]==='string'?a[1]:undefined;if(!local(host))throw Error('fixture blocks outbound network');return connect.apply(this,a);};const nativeFetch=globalThis.fetch;globalThis.fetch=async function(input,...rest){const u=new URL(typeof input==='string'||input instanceof URL?input:input.url);if(!local(u.hostname))throw Error('fixture blocks outbound fetch');return nativeFetch(input,...rest);};`);
const entry = process.env.HIVE_DESKTOP_TEST_RUNTIME ?? fileURLToPath(new URL('../dist/main.js', import.meta.url));
const child = spawn(process.execPath, ['--require', networkGuard, ...(entry.endsWith('.ts') ? ['--import', createRequire(import.meta.url).resolve('tsx')] : []), entry], {cwd: directory, stdio: ['pipe', 'pipe', 'pipe'], env: {...env,
 PATH: bin + ':' + (env.PATH ?? ''), HIVE_DESKTOP_DATA_DIR: join(directory, 'state'), PI_CODING_AGENT_DIR: agent, PI_OFFLINE: '1',
 HIVE_GIT_REMOTE_WIRE_STATE: statePath, GH_REPO: 'wrong-owner/wrong-repo', GH_HOST: 'wrong.example'}});
const exited = once(child, 'exit'); let diagnostics = '', sequence = 0;
child.stderr.on('data', chunk => {diagnostics += String(chunk);});
const pending = new Map<number, {resolve(value: any): void; reject(error: Error): void}>();
const reader = createInterface({input: child.stdout});
reader.on('line', line => {const response = JSON.parse(line), request = pending.get(response.id); if (request) {pending.delete(response.id); request.resolve(response);}});
child.on('exit', () => {for (const request of pending.values()) request.reject(Error('Runtime exited before reply: ' + diagnostics)); pending.clear();});
async function frame(method: string, params: Record<string, unknown> = {}) {
 return new Promise<any>((resolve, reject) => {const id = ++sequence; const timer = setTimeout(() => {pending.delete(id); reject(Error('RPC timed out: ' + method));}, 60000);
  pending.set(id, {resolve: value => {clearTimeout(timer); resolve(value);}, reject: error => {clearTimeout(timer); reject(error);}}); child.stdin.write(JSON.stringify({id, method, params}) + '\n');});
}
async function rpc(method: string, params: Record<string, unknown> = {}) {const response = await frame(method, params); if (response.error) throw Error(response.error.message); return response.result;}
const waitFor = async (read: () => Promise<boolean>, label: string) => {const deadline = Date.now() + 25000; while (!await read()) {if (Date.now() > deadline) throw Error('Timed out: ' + label); await new Promise(resolve => setTimeout(resolve, 20));}};
const exists = (path: string) => access(path).then(() => true, () => false);
const refused = (response: any) => {assert.notEqual(response.result?.ok, true); assert.ok(response.error || response.result?.error, 'Refusal must contain a diagnostic');};
const callLog = async () => (await readFile(log, 'utf8')).trim().split('\n').filter(Boolean).map(line => JSON.parse(line));
const overrides = {path: outside, projectRoot: outside, gitExecutable: badProgram, ghExecutable: badProgram, command: badProgram};
let shuttingDown = false;
try {
 await rpc('initialize');
 const workspace = await rpc('hive.request', {method: 'POST', path: '/api/workspaces', body: {path: repo, name: 'Remote wire fixture', autostart_orchestrator: false}});
 const otherWorkspace = await rpc('hive.request', {method: 'POST', path: '/api/workspaces', body: {path: outside, name: 'Unrelated workspace', autostart_orchestrator: false}});
 const session = await rpc('chat.createRecord', {workspaceId: workspace.id, backend: 'codex', isolation: 'worktree'}), worktree = session.execution.path;
 await writeFile(join(worktree, 'file.txt'), 'session change'); git(worktree, 'add', 'file.txt'); git(worktree, 'commit', '-m', 'session only');
 const scope = {workspaceId: workspace.id, sessionId: session.id, ...overrides};
 const listed = await rpc('git.remotes', scope); assert.equal(listed.ok, true); assert.equal(listed.workspace.projectRoot, await realpath(worktree)); assert.ok(listed.remotes.some((item: any) => item.name === 'origin'));
 assert.ok(!JSON.stringify(listed).includes('wire-private')); assert.ok(!JSON.stringify(listed).includes('tail-secret'));
 const target = {...scope, remote: 'origin', branch: 'session-published'};
 const first = await rpc('git.push.preview', target); assert.equal(first.ok, true); assert.equal(first.preview.workspace.projectRoot, await realpath(worktree)); assert.equal(first.preview.headOid, git(worktree, 'rev-parse', 'HEAD')); assert.equal(first.preview.canPush, true);
 await writeFile(join(worktree, 'file.txt'), 'changed after preview'); git(worktree, 'add', 'file.txt'); git(worktree, 'commit', '-m', 'session changed after preview');
 const stale = await rpc('git.push.apply', {...target, expectedVersion: first.preview.version}); assert.equal(stale.ok, false); assert.equal(stale.pushed, false); assert.equal(stale.applied, false); assert.equal(stale.error.code, 'STALE_PREVIEW');
 assert.throws(() => git(bare, 'rev-parse', '--verify', 'refs/heads/session-published'));
 const fresh = await rpc('git.push.preview', target), sessionHead = git(worktree, 'rev-parse', 'HEAD');
 const pushed = await rpc('git.push.apply', {...target, expectedVersion: fresh.preview.version}); assert.equal(pushed.ok, true); assert.equal(pushed.pushed, true); assert.equal(pushed.headOid, sessionHead); assert.equal(pushed.remoteOid, sessionHead); assert.equal(git(bare, 'rev-parse', 'refs/heads/session-published'), sessionHead);
 assert.equal(git(repo, 'rev-parse', 'HEAD'), rootHead); assert.deepEqual(await readFile(join(repo, '.git/index')), rootIndex); assert.deepEqual(await readFile(join(outside, '.git/index')), unrelatedIndex); assert.throws(() => git(foreignBare, 'show-ref'));
 const rootPreview = await rpc('git.push.preview', {workspaceId: workspace.id, remote: 'origin', branch: 'root-only', ...overrides}); assert.equal(rootPreview.ok, true); assert.equal(rootPreview.preview.workspace.projectRoot, repo); assert.equal(rootPreview.preview.headOid, rootHead);
 refused(await frame('git.remotes', {...scope, workspaceId: otherWorkspace.id})); refused(await frame('git.push.preview', {...target, workspaceId: otherWorkspace.id}));
 await rename(worktree, worktree + '.missing');
 try {refused(await frame('git.remotes', scope)); refused(await frame('git.push.preview', target)); refused(await frame('git.push.apply', {...target, expectedVersion: fresh.preview.version}));}
 finally {await rename(worktree + '.missing', worktree);}
 const invalid = await rpc('git.push.preview', {...scope, remote: '--force', branch: 'bad'}); assert.equal(invalid.ok, false); assert.ok(invalid.error);
 const invalidApply = await rpc('git.push.apply', {...target, expectedVersion: 'invalid'}); assert.equal(invalidApply.ok, false); assert.equal(invalidApply.pushed, false); assert.equal(invalidApply.applied, false);
 const prTarget = {...scope, remote: 'github', base: 'main', headBranch: 'session-pr'};
 let prPreview = await rpc('git.pr.preview', prTarget); assert.equal(prPreview.ok, true); assert.equal(prPreview.preview.pushed, false); assert.equal(prPreview.preview.headBranch, 'session-pr'); assert.equal(prPreview.preview.repository.fullName, 'fixture-owner/fixture-repo');
 const title = 'Wire title $(touch FORBIDDEN) `literal`', body = 'Wire body\n\n中文第二段 $HOME\n';
 const unpushed = await rpc('git.pr.create', {...prTarget, expectedVersion: prPreview.preview.version, title, body}); assert.equal(unpushed.ok, false); assert.equal(unpushed.applied, false); assert.equal(unpushed.operation, 'create'); assert.equal(unpushed.error.code, 'PUSH_REQUIRED');
 peerState.remoteOid = sessionHead; await savePeer(); prPreview = await rpc('git.pr.preview', prTarget); assert.equal(prPreview.ok, true); assert.equal(prPreview.preview.pushed, true);
 const created = await rpc('git.pr.create', {...prTarget, expectedVersion: prPreview.preview.version, title, body, draft: true}); assert.equal(created.ok, true); assert.equal(created.applied, true); assert.equal(created.operation, 'create'); assert.equal(created.url, 'https://github.com/fixture-owner/fixture-repo/pull/17');
 await readPeer(); assert.equal(peerState.body, body); assert.equal(peerState.title, title); assert.equal(await exists(peerState.bodyFile), false);
 prPreview = await rpc('git.pr.preview', prTarget); assert.equal(prPreview.preview.existing[0].number, 17); assert.equal(prPreview.preview.existing[0].isDraft, true);
 const updated = await rpc('git.pr.update', {...prTarget, expectedVersion: prPreview.preview.version, number: 17, title: 'Updated wire title', body: 'Updated\nmultiline body\n'}); assert.equal(updated.ok, true); assert.equal(updated.applied, true); assert.equal(updated.operation, 'update');
 await readPeer(); assert.equal(peerState.isDraft, true);
 const badUpdate = await rpc('git.pr.update', {...prTarget, expectedVersion: 'invalid', number: 17, title, body}); assert.equal(badUpdate.ok, false); assert.equal(badUpdate.applied, false); assert.equal(badUpdate.operation, 'update');
 await readPeer(); peerState.mode = 'auth-failure'; await savePeer(); const denied = await rpc('git.pr.preview', prTarget); assert.equal(denied.ok, false); assert.ok(denied.error.message.includes('HTTP 401')); for (const secret of ['wire-private-token', 'wire-private-query', 'wire-private', 'tail-secret']) assert.ok(!JSON.stringify(denied).includes(secret));
 peerState.mode = 'partial'; peerState.exists = false; await savePeer(); prPreview = await rpc('git.pr.preview', prTarget);
 const beforeMutations = (await callLog()).filter(call => call.kind === 'gh' && call.args[0] === 'pr' && call.args[1] === 'create').length;
 const uncertain = await rpc('git.pr.create', {...prTarget, expectedVersion: prPreview.preview.version, title, body}); assert.equal(uncertain.ok, false); assert.equal(uncertain.applied, 'unknown'); assert.equal(uncertain.operation, 'create');
 assert.equal((await callLog()).filter(call => call.kind === 'gh' && call.args[0] === 'pr' && call.args[1] === 'create').length, beforeMutations + 1, 'A partial remote write must not be retried automatically');
 assert.equal(await exists(forbidden), false); assert.equal(git(repo, 'rev-parse', 'HEAD'), rootHead); assert.deepEqual(await readFile(join(repo, '.git/index')), rootIndex);
 // A real pre-push hook gates the native mutation while shutdown must wait.
 await mkdir(join(repo, '.git/hooks'), {recursive: true}); await writeFile(join(repo, '.git/hooks/pre-push'), `#!/bin/sh\nprintf entered > ${quote(entered)}\nwhile [ ! -e ${quote(release)} ]; do sleep 0.05; done\nprintf finished > ${quote(finished)}\n`); await chmod(join(repo, '.git/hooks/pre-push'), 0o700);
 const closingTarget = {...scope, remote: 'origin', branch: 'closing-push'}, closingPreview = await rpc('git.push.preview', closingTarget);
 let pushReturned = false; const pushing = rpc('git.push.apply', {...closingTarget, expectedVersion: closingPreview.preview.version}).then(value => {pushReturned = true; return value;});
 await waitFor(() => exists(entered), 'real native pre-push hook'); let shutdownReturned = false; shuttingDown = true;
 const shutdown = rpc('shutdown').then(value => {shutdownReturned = true; return value;}); await new Promise(resolve => setTimeout(resolve, 150));
 assert.equal(shutdownReturned, false); assert.equal(pushReturned, false); assert.equal(child.exitCode, null);
 await writeFile(release, 'release'); const completed = await pushing; assert.equal(completed.ok, true); assert.equal(completed.pushed, true); await shutdown; assert.equal((await exited)[0], 0, diagnostics);
 assert.equal(await readFile(finished, 'utf8'), 'finished'); assert.equal(git(bare, 'rev-parse', 'refs/heads/closing-push'), sessionHead);
 await writeFile(join(directory, 'receipt.json'), JSON.stringify({pass: true, actualJsonl: true, runtimeEntry: entry, realLocalBareGit: true, controlledGithubOnly: true, noHostedAccountOrModel: true, outboundNodeNetworkBlocked: true, payloadPathsAndProgramsIgnored: true, worktreeAndRootIsolation: true, missingAndForeignSourceRejected: true, stalePreconditionsNotApplied: true, prCreateUpdateAndUnknown: true, privateBodyFile: true, shutdownWaitsForRealPush: true, windowsNativeValidated: false}, null, 2));
 console.log('GIT_REMOTES_WIRE_OK', directory);
} catch (error) {await writeFile(join(directory, 'failure.json'), JSON.stringify({error: String(error), stack: (error as Error).stack, diagnostics}, null, 2)); throw error;}
finally {
 await writeFile(release, 'release').catch(() => {});
 if (child.exitCode === null && child.signalCode === null) {if (!shuttingDown) child.stdin.end(); await Promise.race([exited, new Promise(resolve => setTimeout(resolve, 8000))]); if (child.exitCode === null && child.signalCode === null) {child.kill('SIGTERM'); await exited;}}
 reader.close();
}

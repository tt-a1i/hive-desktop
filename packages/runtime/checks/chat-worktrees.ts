/** Real Git and Hive metadata; no agent/model invocation. */
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { chmod, mkdir, mkdtemp, readFile, realpath, rename, rm, stat, symlink, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createDesktopRuntime } from '../src/desktop-runtime.js';
import { DeliveryQueue } from '../src/delivery-queue.js';
import { ChatIndex } from '../src/chat-index.js';

const dir = await realpath(await mkdtemp(join(tmpdir(), 'hive-chat-worktree-')));
const data = join(dir, 'state'), repo = join(dir, 'repo'), other = join(dir, 'other');
const oldEnv = { GIT_CONFIG_GLOBAL: process.env.GIT_CONFIG_GLOBAL, GIT_CONFIG_NOSYSTEM: process.env.GIT_CONFIG_NOSYSTEM, PI_CODING_AGENT_DIR: process.env.PI_CODING_AGENT_DIR, PI_OFFLINE: process.env.PI_OFFLINE, HIVE_FIXTURE_ENTERED: process.env.HIVE_FIXTURE_ENTERED, HIVE_FIXTURE_RELEASE: process.env.HIVE_FIXTURE_RELEASE, HIVE_FIXTURE_BACKEND_MARKER: process.env.HIVE_FIXTURE_BACKEND_MARKER };
let runtime: Awaited<ReturnType<typeof createDesktopRuntime>> | undefined;
let events = 0;
try {
 await mkdir(repo); await mkdir(other); await mkdir(join(dir, 'pi'));
 await writeFile(join(dir, 'gitconfig'), '');
 const marker = join(dir, 'backend-launched');
 const backend = join(dir, 'fake-codex');
 await writeFile(backend, '#!/bin/sh\nprintf launched > "$HIVE_FIXTURE_BACKEND_MARKER"\nexit 1\n');
 await chmod(backend, 0o700);
 process.env.HIVE_FIXTURE_BACKEND_MARKER = marker;
 process.env.GIT_CONFIG_GLOBAL = join(dir, 'gitconfig'); process.env.GIT_CONFIG_NOSYSTEM = '1';
 process.env.PI_CODING_AGENT_DIR = join(dir, 'pi'); process.env.PI_OFFLINE = '1';
 const git = (...args: string[]) => execFileSync('git', ['-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.test', '-c', 'commit.gpgSign=false', ...args], { cwd: repo, encoding: 'utf8', stdio: 'pipe' }).trim();
 git('init', '--template='); await writeFile(join(repo, 'same.txt'), 'base'); git('add', '.'); git('commit', '-m', 'fixture');
 runtime = await createDesktopRuntime(data, () => { events++; }, {executableEnvironment: {HIVE_CODEX_COMMAND: backend}});
 const project = await runtime.hive.request('POST', '/api/workspaces', {path: repo, name: 'fixture', autostart_orchestrator: false}) as {id: string};
 const another = await runtime.hive.request('POST', '/api/workspaces', {path: other, name: 'other', autostart_orchestrator: false}) as {id: string};
 const legacy = await runtime.chats.createRecord(project.id, 'codex');
 const one = await runtime.chats.createRecord(project.id, 'codex', {isolation: 'worktree'});
 const two = await runtime.chats.createRecord(project.id, 'codex', {isolation: 'worktree'});
 assert.equal((await runtime.chats.sessionExecution(legacy.id)).path, repo);
 const a = await runtime.chats.sessionExecution(one.id), b = await runtime.chats.sessionExecution(two.id);
 assert.equal(a.kind, 'worktree'); assert.notEqual(a.path, b.path);
 assert.notEqual(one.execution!.branch, two.execution!.branch);
 assert.equal(await runtime.chats.executionDirectory(project.id, one.id), a.path);
 const registrationBeforeReuse = git('worktree', 'list', '--porcelain');
 const removalBeforeReuse = await runtime.chats.previewWorktreeRemoval(project.id, one.id, async () => false);
 const requestId = randomUUID();
 const releaseRemovalLease = runtime.executionMutations.acquire(project.id);
 try {
  await assert.rejects(runtime.chats.createInWorktreeRecord(project.id, one.id, requestId), /temporarily unavailable/);
  assert.ok(!runtime.chats.list(project.id).some(record => record.id === requestId));
 } finally { releaseRemovalLease(); }
 const shared = await runtime.chats.createInWorktreeRecord(project.id, one.id, requestId);
 assert.equal(shared.id, requestId);
 assert.deepEqual(shared.profile, one.profile);
 assert.deepEqual(shared.execution, {...one.execution!, ownerSessionId: one.id});
 assert.equal(shared.executionPath, a.path);
 assert.equal(shared.identityState, 'new');
 assert.equal(shared.agentId, undefined);
 assert.equal(shared.originAgentId, undefined);
 assert.deepEqual(await runtime.chats.createInWorktreeRecord(project.id, one.id, requestId), shared);
 assert.equal(git('worktree', 'list', '--porcelain'), registrationBeforeReuse);
 await assert.rejects(runtime.chats.createInWorktreeRecord(project.id, two.id, requestId), {code: 'WORKTREE_REUSE_CONFLICT'});
 await assert.rejects(runtime.chats.createInWorktreeRecord(another.id, one.id, randomUUID()), {code: 'WORKTREE_SOURCE_INVALID'});
 await assert.rejects(runtime.chats.createInWorktreeRecord(project.id, legacy.id, randomUUID()), {code: 'WORKTREE_SOURCE_INVALID'});
 const flattened = await runtime.chats.createInWorktreeRecord(project.id, shared.id, randomUUID());
 assert.deepEqual(flattened.execution, shared.execution);
 assert.equal(git('worktree', 'list', '--porcelain'), registrationBeforeReuse);
 assert.deepEqual((await runtime.chats.pendingWorktrees(project.id)).pending, []);
 assert.equal(await runtime.initialization.get(project.id, shared.id), null);
 const newDraft = await runtime.chats.open(shared.id);
 assert.ok('passive' in newDraft && newDraft.passive);
 assert.equal(newDraft.sessionFile, null);
 assert.equal(newDraft.codexThreadId, undefined);
 assert.deepEqual(newDraft.history, []);
 assert.equal(newDraft.executionPath, a.path);
 await assert.rejects(runtime.chats.applyWorktreeRemoval(project.id, one.id, removalBeforeReuse.version, async () => false), {code: 'WORKTREE_SHARED'});
 await assert.rejects(runtime.chats.previewWorktreeRemoval(project.id, shared.id, async () => false), {code: 'WORKTREE_NOT_OWNER'});
 await assert.rejects(runtime.chats.recoverWorktreeRecord(project.id, shared.id), {code: 'WORKTREE_REUSE_RECOVERY'});
 assert.equal((await runtime.chats.gitCheckoutBranch(project.id, 'other-branch', true, undefined, one.id)).ok, false);
 assert.equal((await runtime.chats.gitCheckoutBranch(project.id, 'other-branch', true, undefined, shared.id)).ok, false);
 const nestedAliasPath = join(a.path, 'alias-scope');
 await mkdir(nestedAliasPath);
 for (const [name, path] of [['registered-checkout', a.path], ['registered-subdirectory', nestedAliasPath]]) {
  const alias = await runtime.hive.request('POST', '/api/workspaces', {path, name, autostart_orchestrator: false}) as {id: string};
  const blocked = await runtime.chats.gitCheckoutBranch(alias.id, name, true);
  assert.equal(blocked.ok, false);
  if (!blocked.ok) assert.equal(blocked.error.code, 'WORKTREE_SHARED');
  assert.ok((await runtime.removeWorkspace(alias.id)).ok);
 }
 assert.equal(git('-C', a.path, 'symbolic-ref', '--short', 'HEAD'), one.execution!.branch);
 await rm(nestedAliasPath, {recursive: true});

 const bare = join(dir, 'fixture-remote.git');
 git('init', '--bare', '--template=', bare);
 git('remote', 'add', 'fixture', bare);
 await writeFile(join(a.path, 'shared-push.txt'), 'shared commit');
 git('-C', a.path, 'add', 'shared-push.txt');
 git('-C', a.path, 'commit', '-m', 'Shared working directory only');
 const push = await runtime.chats.gitPushPreview(project.id, 'fixture', one.execution!.branch, shared.id);
 assert.ok(push.ok);
 const pushed = await runtime.chats.gitPushApply(project.id, 'fixture', one.execution!.branch, push.preview.version, shared.id);
 assert.ok(pushed.ok);
 assert.equal(git('--git-dir', bare, 'rev-parse', `refs/heads/${one.execution!.branch}`), git('-C', a.path, 'rev-parse', 'HEAD'));
 assert.match(await readFile(join(data, 'worktrees', one.id, 'verified-push.json'), 'utf8'), /fixture/);
 // Git's registered path can be an alias of the same physical worktree.
 // Repoint only the fixture's real Git registry entry, then verify ownership
 // against the actual filesystem and reject a different existing worktree.
 if (process.platform !== 'win32') {
  const gitDir = execFileSync('git', ['-C', a.path, 'rev-parse', '--absolute-git-dir'], {encoding: 'utf8'}).trim();
  const pointer = join(gitDir, 'gitdir'); const original = await readFile(pointer, 'utf8');
  const alias = join(dir, 'worktree-alias'); await symlink(a.path, alias, 'dir');
  const foreign = join(dir, 'foreign-worktree');
  git('worktree', 'add', '-b', 'foreign-fixture', foreign); await rename(foreign, foreign + '.missing');
  try {
   await writeFile(pointer, join(alias, '.git') + '\n');
   assert.ok(git('worktree', 'list', '--porcelain').includes(`worktree ${alias}`));
   assert.equal(await runtime.chats.executionDirectory(project.id, one.id), a.path);
   await writeFile(pointer, join(b.path, '.git') + '\n');
   await assert.rejects(runtime.chats.executionDirectory(project.id, one.id), /not registered|工作树/i);
  } finally {
   await writeFile(pointer, original);
   await rename(foreign + '.missing', foreign);
   git('worktree', 'remove', '--', foreign);
   await rm(alias);
  }
 }
 await writeFile(join(a.path, 'same.txt'), 'one'); await writeFile(join(b.path, 'same.txt'), 'two');
 assert.equal(await readFile(join(repo, 'same.txt'), 'utf8'), 'base');
 assert.equal(await readFile(join(a.path, 'same.txt'), 'utf8'), 'one'); assert.equal(await readFile(join(b.path, 'same.txt'), 'utf8'), 'two');
 const sharedAttachment = await runtime.chats.projectFilesAttach(project.id, 'same.txt', shared.id);
 assert.equal(sharedAttachment.attachment.projectFile?.sourceSessionId, shared.id);
 assert.equal(await readFile(sharedAttachment.filePath!, 'utf8'), 'one');
 assert.equal((await runtime.terminal.list(project.id, shared.id)).sessionId, shared.id);
 await runtime.chats.setArchived(shared.id, true);
 await runtime.chats.setArchived(flattened.id, true);
 await assert.rejects(runtime.chats.previewWorktreeRemoval(project.id, one.id, async () => false), {code: 'WORKTREE_SHARED'});
 await runtime.close(); runtime = undefined;
 const seededIndex = new ChatIndex(join(data, 'chats'));
 seededIndex.save({...seededIndex.get(one.id), identityState: 'known', originAgentId: `${project.id}:orchestrator`, codexThreadId: 'owner-native-history'});
 runtime = await createDesktopRuntime(data, () => { events++; }, {executableEnvironment: {HIVE_CODEX_COMMAND: backend}});
 assert.deepEqual(await runtime.chats.sessionExecution(one.id), a); assert.deepEqual(await runtime.chats.sessionExecution(two.id), b);
 assert.equal((await runtime.chats.sessionExecution(shared.id)).path, a.path);
 assert.equal((await runtime.chats.createInWorktreeRecord(project.id, one.id, requestId)).id, shared.id);
 assert.equal(runtime.chats.list(project.id).filter(record => record.id === shared.id).length, 1);
 await assert.rejects(runtime.chats.previewWorktreeRemoval(project.id, one.id, async () => false), {code: 'WORKTREE_SHARED'});
 assert.equal((await runtime.chats.history!.capabilities(project.id, one.id)).rewindCode, false);
 await assert.rejects(runtime.chats.history!.preview({workspaceId: project.id, sessionId: one.id,
  boundary: {nativeUserItemId: 'owned-boundary'}, code: {kind: 'checkpoint', checkpointId: randomUUID()}, rewind: true}), {code: 'SHARED_CHECKOUT'});
 await runtime.chats.setArchived(shared.id, false);
 const queue = new DeliveryQueue({directory: join(data, 'chats', 'deliveries'), sessionId: one.id, startPaused: true,
  execute: async () => { throw Error('must not execute'); }, completedIds: () => [], onError: e => { throw e; }});
 await queue.enqueue('preserve queued text', true, {version: 1, origin: 'user', displayText: 'preserve queued text'});
 const spool = join(data, 'chats', 'deliveries', createHash('sha256').update(one.id).digest('hex') + '.json');
 const before = await readFile(spool, 'utf8'); const eventsBefore = events;
 await rename(a.path, a.path + '.moved');
 const missingShared = await runtime.chats.open(shared.id);
 assert.ok('readOnly' in missingShared && missingShared.readOnly);
 assert.equal(missingShared.executionPath, a.path);
 await assert.rejects(runtime.chats.sessionExecution(shared.id), /工作树|worktree|ENOENT/i);
 await assert.rejects(runtime.chats.prompt(shared.id, 'must remain unsent'), /工作树|worktree|ENOENT/i);
 await assert.rejects(runtime.terminal.list(project.id, shared.id), /工作树|worktree|ENOENT/i);
 const missing = await runtime.chats.open(one.id);
 assert.ok('readOnly' in missing && missing.readOnly && 'identityError' in missing);
 assert.equal(missing.identityError?.code, 'IDENTITY_MISMATCH');
 assert.equal(missing.executionPath, a.path);
 await assert.rejects(runtime.chats.sessionExecution(one.id), /工作树|worktree|ENOENT/i);
 await mkdir(a.path);
 const replacement = await runtime.chats.open(one.id);
 assert.ok('readOnly' in replacement && replacement.readOnly && 'identityError' in replacement);
 assert.equal(replacement.identityError?.code, 'IDENTITY_MISMATCH');
 assert.equal(replacement.executionPath, a.path);
 await assert.rejects(runtime.chats.sessionExecution(one.id), /工作树|worktree|git/i);
 await rm(a.path, { recursive: true });
 assert.equal(await readFile(spool, 'utf8'), before); assert.equal(queue.pending[0]!.text, 'preserve queued text');
 assert.equal(queue.busy, false); assert.equal(events, eventsBefore);
 assert.equal(await stat(marker).then(() => true, () => false), false, 'backend must not start');
 await rename(a.path + '.moved', a.path);
 const index = join(data, 'chats', 'index.json');
 await rename(index, index + '.held'); await mkdir(index);
 try { await assert.rejects(runtime.chats.createRecord(project.id, 'codex', {isolation: 'worktree'}), /工作树已保留/); }
 finally { await rm(index, {recursive: true}); await rename(index + '.held', index); }
 const pending = (await runtime.chats.pendingWorktrees(project.id)).pending;
 assert.equal(pending.length, 1); const orphan = pending[0]!;
 await writeFile(join(orphan.path, 'owned-code.txt'), 'keep');
 await assert.rejects(runtime.chats.recoverWorktreeRecord(another.id, orphan.sessionId, 'codex'));
 const recovered = await runtime.chats.recoverWorktreeRecord(project.id, orphan.sessionId, 'codex');
 assert.equal(recovered.id, orphan.sessionId); assert.equal(recovered.execution!.path, orphan.path); assert.equal(recovered.execution!.branch, orphan.branch);
 assert.equal(await readFile(join(orphan.path, 'owned-code.txt'), 'utf8'), 'keep');
 assert.equal((await runtime.chats.pendingWorktrees(project.id)).pending.length, 0);
 if (process.platform !== 'win32') {
  const hooks = join(dir, 'hooks'); await mkdir(hooks);
  await writeFile(join(hooks, 'post-checkout'), '#!/bin/sh\nprintf entered > "$HIVE_FIXTURE_ENTERED"\nwhile [ ! -e "$HIVE_FIXTURE_RELEASE" ]; do sleep 0.05; done\n');
  await chmod(join(hooks, 'post-checkout'), 0o700); git('config', 'core.hooksPath', hooks);
  process.env.HIVE_FIXTURE_ENTERED = join(dir, 'entered'); process.env.HIVE_FIXTURE_RELEASE = join(dir, 'release');
  const creating = runtime.chats.createRecord(project.id, 'codex', {isolation: 'worktree'});
  const result = creating.then(value => ({value}), error => ({error}));
  try {
   const deadline = Date.now() + 10000;
   while (!(await stat(process.env.HIVE_FIXTURE_ENTERED).then(() => true, () => false))) {
    if (Date.now() > deadline) throw Error('checkout hook did not start');
    await new Promise(resolve => setTimeout(resolve, 25));
   }
   let closed = false;
   const closing = runtime.close().then(() => { closed = true; });
   await new Promise(resolve => setTimeout(resolve, 100));
   assert.equal(closed, false, 'close must wait for active worktree creation');
   await writeFile(process.env.HIVE_FIXTURE_RELEASE, 'release');
   const creation = await result; assert.ok('error' in creation, 'closing must not publish the new chat');
   await closing; runtime = undefined;
   runtime = await createDesktopRuntime(data, () => { events++; }, {executableEnvironment: {HIVE_CODEX_COMMAND: backend}});
   const afterClose = (await runtime.chats.pendingWorktrees(project.id)).pending;
   assert.equal(afterClose.length, 1); assert.equal(runtime.chats.list(project.id).some(record => record.id === afterClose[0]!.sessionId), false);
  } finally { await writeFile(join(dir, 'release'), 'release'); await result; }
 }
 console.log('CHAT_WORKTREES_OK');
} finally {
 if (runtime) await runtime.close();
 for (const [key, value] of Object.entries(oldEnv)) { if (value === undefined) delete process.env[key]; else process.env[key] = value; }
 await rm(dir, {recursive: true, force: true, maxRetries: 5});
}

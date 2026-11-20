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
const oldEnv = { GIT_CONFIG_GLOBAL: process.env.GIT_CONFIG_GLOBAL, GIT_CONFIG_NOSYSTEM: process.env.GIT_CONFIG_NOSYSTEM, PI_CODING_AGENT_DIR: process.env.PI_CODING_AGENT_DIR, PI_OFFLINE: process.env.PI_OFFLINE, HIVE_FIXTURE_ENTERED: process.env.HIVE_FIXTURE_ENTERED, HIVE_FIXTURE_RELEASE: process.env.HIVE_FIXTURE_RELEASE, HIVE_FIXTURE_BACKEND_MARKER: process.env.HIVE_FIXTURE_BACKEND_MARKER, CLAUDE_CONFIG_DIR: process.env.CLAUDE_CONFIG_DIR, CLAUDE_CODE_PROJECT_DIR_NAME: process.env.CLAUDE_CODE_PROJECT_DIR_NAME };
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
 const freshBase = git('rev-parse', 'HEAD');
 const freshSource = await runtime.chats.createRecord(project.id, 'codex', {isolation: 'worktree'});
 const sourcePath = freshSource.execution!.path;
 await writeFile(join(repo, 'project-advance.txt'), 'project-only commit');
 git('add', 'project-advance.txt'); git('commit', '-m', 'Project advanced');
 await writeFile(join(sourcePath, 'source-commit.txt'), 'source-only commit');
 git('-C', sourcePath, 'add', 'source-commit.txt'); git('-C', sourcePath, 'commit', '-m', 'Source advanced');
 await writeFile(join(sourcePath, 'same.txt'), 'source dirty change');
 await writeFile(join(sourcePath, 'source-untracked.txt'), 'source untracked change');
 const createFresh = (id: string, source = freshSource.id) => runtime!.chats.createFreshWorktreeDraftRecord(project.id, source, id);
 await assert.rejects(createFresh(randomUUID(), legacy.id), {code: 'WORKTREE_FRESH_DRAFT_UNSUPPORTED'});
 await assert.rejects(createFresh(randomUUID(), shared.id), {code: 'WORKTREE_FRESH_DRAFT_UNSUPPORTED'});
 assert.equal(runtime.chats.list(project.id).find(record => record.id === freshSource.id)?.canStartFreshWorktreeDraft, true);
 assert.equal(runtime.chats.list(project.id).find(record => record.id === shared.id)?.canStartFreshWorktreeDraft, undefined);
 const freshId = randomUUID();
 const [freshDraft, repeated] = await Promise.all([createFresh(freshId), createFresh(freshId)]);
 assert.deepEqual(repeated, freshDraft);
 assert.equal(freshDraft.id, freshId);
 assert.deepEqual(freshDraft.profile, freshSource.profile);
 assert.equal(freshDraft.backend, freshSource.backend);
 assert.equal(freshDraft.canStartFreshWorktreeDraft, true);
 assert.notEqual(freshDraft.executionPath, sourcePath);
 assert.notEqual(freshDraft.execution?.branch, freshSource.execution?.branch);
 assert.equal(freshDraft.execution?.ownerSessionId, undefined);
 assert.equal(git('-C', freshDraft.executionPath, 'rev-parse', 'HEAD'), freshBase);
 assert.equal(git('-C', freshDraft.executionPath, 'status', '--porcelain'), '');
 assert.equal(await readFile(join(freshDraft.executionPath, 'same.txt'), 'utf8'), 'base');
 assert.equal(await readFile(join(sourcePath, 'same.txt'), 'utf8'), 'source dirty change');
 for (const file of ['project-advance.txt', 'source-commit.txt', 'source-untracked.txt'])
  await assert.rejects(stat(join(freshDraft.executionPath, file)), {code: 'ENOENT'});
 await assert.rejects(createFresh(freshId, two.id), {code: 'WORKTREE_FRESH_DRAFT_CONFLICT'});
 await rename(sourcePath, sourcePath + '.moved');
 try {
  await assert.rejects(createFresh(randomUUID()), {code: 'IDENTITY_MISMATCH'});
  assert.deepEqual(await createFresh(freshId), freshDraft);
 } finally { await rename(sourcePath + '.moved', sourcePath); }
 const passiveFresh = await runtime.chats.open(freshId);
 assert.ok('passive' in passiveFresh && passiveFresh.passive && !passiveFresh.readOnly);
 assert.equal(passiveFresh.agentId, undefined);
 assert.equal(passiveFresh.codexThreadId, undefined);
 assert.deepEqual(passiveFresh.history, []);
 const freshRecoveryId = randomUUID();
 const draftIndexPath = join(data, 'chats', 'index.json');
 await rename(draftIndexPath, draftIndexPath + '.held'); await mkdir(draftIndexPath);
 try { await assert.rejects(createFresh(freshRecoveryId), /工作树已保留/); }
 finally { await rm(draftIndexPath, {recursive: true}); await rename(draftIndexPath + '.held', draftIndexPath); }
 assert.equal((await runtime.chats.pendingWorktrees(project.id)).pending.some(item => item.sessionId === freshRecoveryId), true);
 const releaseBorrower = (id: string, owner = one.id) => runtime!.chats.releaseWorktreeUse(project.id, id, owner,
  () => runtime!.terminal.hasActiveSessionShell(project.id, id));
 await assert.rejects(releaseBorrower(shared.id, two.id), {code: 'WORKTREE_SOURCE_INVALID'});
 await assert.rejects(releaseBorrower(one.id), {code: 'WORKTREE_SOURCE_INVALID'});
 const exclusive = runtime.executionMutations.acquire(project.id);
 try { await assert.rejects(releaseBorrower(shared.id), {code: 'WORKTREE_IN_USE'}); }
 finally { exclusive(); }
 const pendingBorrower = new DeliveryQueue({directory: join(data, 'chats', 'deliveries'), sessionId: shared.id, startPaused: true,
  execute: async () => { throw Error('must not execute'); }, completedIds: () => [], onError: e => { throw e; }});
 await pendingBorrower.enqueue('pending borrower input', true);
 await assert.rejects(releaseBorrower(shared.id), {code: 'WORKTREE_IN_USE'});
 pendingBorrower.discardPending(new Set(pendingBorrower.pending.map(item => item.id)));
 const ownedShell = await runtime.terminal.start(project.id, shared.id);
 try { await assert.rejects(releaseBorrower(shared.id), {code: 'WORKTREE_IN_USE'}); }
 finally { await runtime.terminal.stop({workspaceId: project.id, sessionId: shared.id, runId: ownedShell.runId}); }
 let enterRelease!: () => void;
 let finishRelease!: () => void;
 const releaseStarted = new Promise<void>(resolve => { enterRelease = resolve; });
 const releaseMayFinish = new Promise<void>(resolve => { finishRelease = resolve; });
 const releaseInFlight = runtime.chats.releaseWorktreeUse(project.id, shared.id, one.id, async () => {
  enterRelease();
  await releaseMayFinish;
  return false;
 });
 await releaseStarted;
 try { await assert.rejects(runtime.chats.applyWorktreeRemoval(project.id, one.id, removalBeforeReuse.version, async () => false), {code: 'WORKTREE_IN_USE'}); }
 finally { finishRelease(); }
 const releasedShared = await releaseInFlight;
 assert.equal(releasedShared.worktreeReleased, true);
 assert.equal(releasedShared.executionPath, a.path);
 assert.deepEqual(releasedShared.execution, shared.execution);
 await assert.rejects(runtime.chats.previewWorktreeRemoval(project.id, one.id, async () => false), {code: 'WORKTREE_SHARED'});
 await rename(a.path, a.path + '.moved');
 try { assert.equal((await releaseBorrower(flattened.id)).archived, true); }
 finally { await rename(a.path + '.moved', a.path); }
 assert.equal(await readFile(sharedAttachment.filePath!, 'utf8'), 'one');
 assert.equal((await runtime.chats.attachments.get(project.id, [sharedAttachment.attachment.id]))[0]?.filePath, sharedAttachment.filePath);
 await assert.rejects(runtime.chats.projectFileStatus(project.id, sharedAttachment.attachment.id), {code: 'WORKTREE_RELEASED'});
 await assert.rejects(runtime.chats.createInWorktreeRecord(project.id, shared.id, randomUUID()), {code: 'WORKTREE_RELEASED'});
 assert.deepEqual(await runtime.chats.createInWorktreeRecord(project.id, one.id, requestId), releasedShared);
 await assert.rejects(runtime.terminal.start(project.id, shared.id), {code: 'WORKTREE_RELEASED'});
 assert.equal((await runtime.chats.history!.capabilities(project.id, one.id)).rewindCode, true);

 // Restore exact Claude JSONL history after real owner removal and a cold start.
 const claudeConfig = join(dir, 'claude');
 process.env.CLAUDE_CONFIG_DIR = claudeConfig;
 delete process.env.CLAUDE_CODE_PROJECT_DIR_NAME;
 const claudeOwner = await runtime.chats.createRecord(project.id, 'claude', {isolation: 'worktree'});
 const claudeBorrower = await runtime.chats.createInWorktreeRecord(project.id, claudeOwner.id, randomUUID());
 const claudeCwd = claudeOwner.execution!.path;
 const nativeId = randomUUID();
 const claudeProject = join(claudeConfig, 'projects', claudeCwd.replace(/[^a-zA-Z0-9]/g, '-'));
 await mkdir(claudeProject, {recursive: true});
 const transcript = join(claudeProject, `${nativeId}.jsonl`);
 await writeFile(transcript, JSON.stringify({type: 'user', uuid: randomUUID(), parentUuid: null,
  sessionId: nativeId, cwd: claudeCwd, isSidechain: false, timestamp: new Date().toISOString(),
  message: {role: 'user', content: 'saved released Claude history'}}) + '\n');
 await runtime.close(); runtime = undefined;
 const claudeIndex = new ChatIndex(join(data, 'chats'));
 claudeIndex.save({...claudeIndex.get(claudeBorrower.id), identityState: 'known', nativeclaudeSessionId: nativeId, originAgentId: `${project.id}:orchestrator`});
 runtime = await createDesktopRuntime(data, () => { events++; }, {executableEnvironment: {HIVE_CODEX_COMMAND: backend, HIVE_CLAUDE_COMMAND: backend}});
 assert.deepEqual(await createFresh(freshId), freshDraft);
 const recoveredFresh = await createFresh(freshRecoveryId);
 assert.equal(recoveredFresh.id, freshRecoveryId);
 assert.equal(git('-C', recoveredFresh.executionPath, 'rev-parse', 'HEAD'), freshBase);
 const recoveredOpen = await runtime.chats.open(freshRecoveryId);
 assert.ok('passive' in recoveredOpen && recoveredOpen.passive);
 assert.equal(recoveredOpen.codexThreadId, undefined);
 assert.equal(await stat(marker).then(() => true, () => false), false, 'fresh worktree drafts stay offline across recovery');
 assert.deepEqual(await releaseBorrower(shared.id), releasedShared);
 await assert.rejects(runtime.chats.previewWorktreeRemoval(project.id, claudeOwner.id, async () => false), {code:'WORKTREE_SHARED'});
 const claudeRelease = await releaseBorrower(claudeBorrower.id, claudeOwner.id);
 const removal = await runtime.chats.previewWorktreeRemoval(project.id, claudeOwner.id, async () => false);
 await runtime.chats.applyWorktreeRemoval(project.id, claudeOwner.id, removal.version, async () => false);
 await assert.rejects(stat(claudeCwd), {code: 'ENOENT'});
 await runtime.close(); runtime = undefined;
 runtime = await createDesktopRuntime(data, () => { events++; }, {executableEnvironment: {HIVE_CODEX_COMMAND: backend, HIVE_CLAUDE_COMMAND: backend}});
 assert.deepEqual(await releaseBorrower(claudeBorrower.id, claudeOwner.id), claudeRelease);
 const history = await runtime.chats.open(claudeBorrower.id);
 assert.ok('readOnly' in history && history.readOnly && 'identityError' in history);
 assert.equal(history.identityError?.code, 'WORKTREE_RELEASED');
 assert.equal(history.executionPath, claudeCwd);
 assert.match(JSON.stringify(history.history), /saved released Claude history/);
 const page = await runtime.chats.historyPage(claudeBorrower.id, {limit: 5});
 assert.deepEqual(page.historyMessageIds, history.historyMessageIds);
 assert.equal(page.historyPage.totalCount, 1);
 const view = await runtime.chats.openDesktopView(claudeBorrower.id, 1, true, {limit: 5});
 assert.deepEqual(view.historyMessageIds, history.historyMessageIds);
 assert.equal((await runtime.chats.searchMessages(claudeBorrower.id, 1, 'released Claude', new AbortController().signal)).matches.length, 1);
 const savedTranscript = await readFile(transcript, 'utf8');
 await writeFile(transcript, savedTranscript.replaceAll(claudeCwd, repo).replace('saved released Claude history', 'wrong Claude history'));
 try {
  const mismatched = await runtime.chats.open(claudeBorrower.id);
  assert.deepEqual(mismatched.history, []);
  assert.ok('historyIncomplete' in mismatched && mismatched.historyIncomplete);
 } finally { await writeFile(transcript, savedTranscript); }
 assert.equal(await stat(marker).then(() => true, () => false), false, 'released history never starts a backend');
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

import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { existsSync } from 'node:fs';
import { mkdir, mkdtemp, readFile, realpath, rename, rm, rmdir, symlink, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createDesktopRuntime } from '../src/desktop-runtime.js';
import { ChatIndex } from '../src/chat-index.js';
import { createScratchConversation, scratchWorkspace } from '../src/scratch-workspace.js';

const root = await realpath(await mkdtemp(join(tmpdir(), 'hive-desktop-scratch-')));
const data = join(root, 'data');
await mkdir(data);
const requestId = randomUUID();

try {
  const gitRoot = join(root, 'checkout');
  const gitData = join(gitRoot, 'state');
  await mkdir(gitData, { recursive: true });
  await mkdir(join(gitRoot, '.git'));
  const unexpectedRequest = async () => { throw new Error('An unavailable directory must never reach Hive'); };
  await assert.rejects(scratchWorkspace(gitData, unexpectedRequest), /outside a Git working tree/);
  assert.ok(!existsSync(join(gitData, 'scratch')));

  const linkedData = join(root, 'linked');
  const target = join(root, 'outside');
  await mkdir(linkedData);
  await mkdir(target);
  await symlink(target, join(linkedData, 'scratch'), 'junction');
  await assert.rejects(scratchWorkspace(linkedData, unexpectedRequest), /inside the desktop data folder/);

  let runtime = await createDesktopRuntime(data, () => {});
  let workspaceId: string;
  let secondId: string;
  let firstPath: string;
  let secondPath: string;
  const legacyId = randomUUID();
  try {
    assert.deepEqual(await runtime.requestHive('GET', '/api/workspaces'), []);
    await assert.rejects(runtime.chats.createScratchRecord('pi', {id: 'missing', revision: 1}, randomUUID(),
      () => scratchWorkspace(data, runtime.requestHive)));
    assert.ok(!existsSync(join(data, 'scratch')));

    let lostResponse = false;
    const create = () => runtime.chats.createScratchRecord('pi', undefined, requestId,
      () => scratchWorkspace(data, async (method, path, body) => {
        const result = await runtime.requestHive(method, path, body);
        if (method === 'POST' && !lostResponse) {
          lostResponse = true;
          throw new Error('Fixture dropped workspace creation reply');
        }
        return result;
      }));
    const [first, retry] = await Promise.all([create(), create()]);
    workspaceId = first.workspace.id;
    assert.ok(lostResponse);
    assert.equal(first.workspace.path, join(data, 'scratch'));
    assert.equal(retry.workspace.id, workspaceId);
    assert.equal(first.session.id, requestId);
    assert.equal(retry.session.id, requestId);
    assert.equal(first.session.scratch, true);
    assert.equal(first.session.execution, undefined);
    assert.deepEqual(runtime.chats.list(workspaceId).map(session => session.id), [requestId]);
    firstPath = join(first.workspace.path, 'sessions', workspaceId, requestId);
    assert.deepEqual(await runtime.chats.sessionExecution(requestId), {sessionId: requestId,
      workspaceId, path: firstPath, kind: 'scratch'});
    const second = await runtime.chats.createRecord(workspaceId, 'pi');
    secondId = second.id;
    secondPath = join(first.workspace.path, 'sessions', workspaceId, secondId);
    assert.equal(second.scratch, true);
    assert.equal(await runtime.chats.executionDirectory(workspaceId, secondId), secondPath);
    await assert.rejects(runtime.chats.createRecord(workspaceId, 'pi', {isolation: 'worktree'}), /do not use Git worktrees/);

    // Replay a request that already claimed a directory but never published its chat.
    const unpublished = randomUUID();
    const claimed = await createScratchConversation(data, first.workspace, unpublished);
    await writeFile(join(claimed, 'keep.txt'), 'Unpublished folder content.\n');
    const recovered = await runtime.chats.createScratchRecord('pi', undefined, unpublished,
      () => scratchWorkspace(data, runtime.requestHive));
    assert.equal(recovered.session.id, unpublished);
    assert.equal(await runtime.chats.executionDirectory(workspaceId, unpublished), claimed);
    assert.equal(await readFile(join(claimed, 'keep.txt'), 'utf8'), 'Unpublished folder content.\n');
  } finally {
    await runtime.close();
  }

  // Add a record shaped like the previous release's shared-root Scratch chats.
  const index = new ChatIndex(join(data, 'chats'));
  const prior = index.get(requestId);
  const {scratch: _scratch, ...ordinary} = prior;
  index.save({...ordinary, id: legacyId});
  assert.throws(() => index.save({...prior, scratch: undefined}), /ownership cannot be changed/);

  runtime = await createDesktopRuntime(data, () => {});
  try {
    const retry = await runtime.chats.createScratchRecord('pi', undefined, requestId,
      () => scratchWorkspace(data, runtime.requestHive));
    assert.equal(retry.workspace.id, workspaceId);
    assert.equal(retry.session.id, requestId);
    assert.equal(retry.session.sessionFile, null);
    assert.equal(await runtime.chats.executionDirectory(workspaceId, requestId), firstPath);
    assert.equal(await runtime.chats.executionDirectory(workspaceId, secondId), secondPath);
    const legacy = await runtime.chats.createScratchRecord('pi', undefined, legacyId,
      () => scratchWorkspace(data, runtime.requestHive));
    assert.equal(legacy.session.scratch, undefined);
    assert.equal(await runtime.chats.executionDirectory(workspaceId, legacyId), retry.workspace.path);
    const markers = [[firstPath, requestId, 'Keep alpha.\n'], [secondPath, secondId, 'Keep beta.\n']] as const;
    for (const [path, sessionId, content] of markers) {
      await writeFile(join(path, 'notes.txt'), content);
      const {attachment, filePath} = await runtime.chats.projectFilesAttach(workspaceId, 'notes.txt', sessionId);
      assert.equal(attachment.projectFile?.sourceSessionId, sessionId);
      assert.equal(await readFile(filePath!, 'utf8'), content);
      assert.deepEqual(await runtime.chats.projectFileStatus(workspaceId, attachment.id), {state: 'unchanged'});
      assert.ok(JSON.stringify(await runtime.chats.projectFilesPreview(workspaceId, 'notes.txt', sessionId)).includes(content.trim()));
    }
    for (const [path, sessionId] of markers) {
      const original = path + '-retained';
      await rename(path, original);
      await assert.rejects(runtime.chats.executionDirectory(workspaceId, sessionId), /missing or was replaced/);
      await mkdir(path);
      await assert.rejects(runtime.chats.executionDirectory(workspaceId, sessionId), /missing or was replaced/);
      await rmdir(path);
      await symlink(original, path, 'junction');
      await assert.rejects(runtime.chats.projectFilesPreview(workspaceId, 'notes.txt', sessionId), /missing or was replaced/);
      await rm(path);
      await rename(original, path);
      assert.equal(await runtime.chats.executionDirectory(workspaceId, sessionId), path);
    }
    await runtime.removeWorkspace(workspaceId);
    assert.deepEqual(await runtime.requestHive('GET', '/api/workspaces'), []);
    for (const [path, , content] of markers) assert.equal(await readFile(join(path, 'notes.txt'), 'utf8'), content);
    const fresh = await runtime.chats.createScratchRecord('pi', undefined, requestId,
      () => scratchWorkspace(data, runtime.requestHive));
    assert.notEqual(fresh.workspace.id, workspaceId);
    assert.notEqual(await runtime.chats.executionDirectory(fresh.workspace.id, requestId), firstPath);
  } finally {
    await runtime.close();
  }
  console.log('DESKTOP_SCRATCH_OK: isolated directories, legacy cwd, retry, attachment scopes, replacement guards, and file retention');
} finally {
  await rm(root, { recursive: true, force: true });
}

import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { mkdir, mkdtemp, readFile, realpath, rename, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { ChatIndex } from '../src/chat-index.js';
import { createDesktopRuntime } from '../src/desktop-runtime.js';
import { scratchWorkspace } from '../src/scratch-workspace.js';
import { WorkspaceRemovalError } from '../src/workspace-removal.js';
import type { WorkspaceSummary } from '../src/hive-runtime-contract.js';

const root = await realpath(await mkdtemp(join(tmpdir(), 'hive-scratch-project-')));
const data = join(root, 'data');
const nativeId = randomUUID();
const user = 'Original Scratch question';
const reply = 'Original Scratch reply';
const blocked = (error: unknown) => error instanceof WorkspaceRemovalError
  && error.code === 'WORKSPACE_REMOVAL_BLOCKED' && error.details.workspaceRemoved === false;
async function withoutWaitingForRemoval<T>(task: Promise<T>): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  try {
    return await Promise.race([task, new Promise<never>((_, reject) => {
      timer = setTimeout(() => reject(new Error('Independent Scratch request waited for an unrelated workspace drain')), 5000);
    })]);
  } finally { clearTimeout(timer); }
}
let runtime: Awaited<ReturnType<typeof createDesktopRuntime>> | undefined;
try {
  runtime = await createDesktopRuntime(data, () => {});
  const original = await runtime.chats.createScratchRecord('pi-rpc', undefined, randomUUID(),
    () => scratchWorkspace(data, runtime!.requestHive));
  const sourceWorkspace = original.workspace.id;
  const sourceSession = original.session.id;
  const beta = await runtime.chats.createRecord(sourceWorkspace, 'pi-rpc');
  const alphaPath = await runtime.chats.executionDirectory(sourceWorkspace, sourceSession);
  const betaPath = await runtime.chats.executionDirectory(sourceWorkspace, beta.id);
  await writeFile(join(alphaPath, 'notes.txt'), 'Alpha output\n');
  await writeFile(join(betaPath, 'notes.txt'), 'Beta output\n');
  const staged = await runtime.chats.projectFilesAttach(sourceWorkspace, 'notes.txt', sourceSession);

  await assert.rejects(runtime.chats.promoteScratchProject(sourceWorkspace, sourceSession, '\n', runtime.requestHive), /项目名称不能为空/);
  await assert.rejects(runtime.chats.promoteScratchProject(randomUUID(), sourceSession, 'Wrong source', runtime.requestHive), /独立目录/);
  assert.deepEqual(await runtime.chats.listScratchProjectLinks(), {links: []});

  let accepted: WorkspaceSummary | undefined;
  await assert.rejects(runtime.chats.promoteScratchProject(sourceWorkspace, sourceSession, 'Alpha', async (method, path, body) => {
    if (accepted) throw new Error('Fixture disconnected after the project commit');
    const result = await runtime!.requestHive(method, path, body);
    if (method === 'POST') {
      accepted = result as WorkspaceSummary;
      throw new Error('Fixture dropped the project creation reply');
    }
    return result;
  }), /disconnected after the project commit/);
  assert(accepted);
  const alphaProject = accepted;
  assert.equal(alphaProject.path, alphaPath);
  await runtime.close();
  runtime = undefined;

  // A disk-only native transcript provides a real history/identity receipt without starting a backend.
  const sessionFile = join(data, 'chats', 'external-pi', `${nativeId}.jsonl`);
  const now = new Date().toISOString();
  await mkdir(join(data, 'chats', 'external-pi'), {recursive: true});
  const sourceTranscript = [
    {type: 'session', version: 3, id: nativeId, cwd: alphaPath, timestamp: now},
    {type: 'message', id: 'user-1', parentId: null, timestamp: now,
      message: {role: 'user', content: user, timestamp: Date.now()}},
    {type: 'message', id: 'reply-1', parentId: 'user-1', timestamp: now,
      message: {role: 'assistant', content: [{type: 'text', text: reply}], timestamp: Date.now(), stopReason: 'stop'}},
  ].map(entry => JSON.stringify(entry)).join('\n') + '\n';
  await writeFile(sessionFile, sourceTranscript);
  const index = new ChatIndex(join(data, 'chats'));
  index.save({...index.get(sourceSession), sessionFile, externalPiSessionId: nativeId});
  index.bind(sourceSession, `${sourceWorkspace}:orchestrator`);

  runtime = await createDesktopRuntime(data, () => {});
  const cold = await runtime.chats.listScratchProjectLinks();
  assert.deepEqual(cold, {links: [{sourceWorkspaceId: sourceWorkspace, sourceSessionId: sourceSession,
    targetWorkspaceId: alphaProject.id, path: alphaPath}]});
  const repeated = await Promise.all(['New alpha title', 'Another alpha title'].map(name =>
    runtime!.chats.promoteScratchProject(sourceWorkspace, sourceSession, name, runtime!.requestHive)));
  assert(repeated.every(item => item.workspace.id === alphaProject.id && item.workspace.name === 'Alpha'));
  const opened = await runtime.chats.openDesktopView(sourceSession, 1, true, {limit: 120});
  assert.equal(opened.workspaceId, sourceWorkspace);
  assert.equal(opened.executionPath, alphaPath);
  assert.equal(opened.externalPiSessionId, nativeId);
  assert.equal(opened.originAgentId, `${sourceWorkspace}:orchestrator`);
  assert('savedHistoryPassive' in opened && opened.savedHistoryPassive);
  assert(JSON.stringify(opened.history).includes(user) && JSON.stringify(opened.history).includes(reply));
  assert.equal(await readFile(sessionFile, 'utf8'), sourceTranscript);
  assert.deepEqual(await runtime.chats.projectFileStatus(sourceWorkspace, staged.attachment.id), {state: 'unchanged'});
  const [reopenedAttachment] = await runtime.chats.attachments.get(sourceWorkspace, [staged.attachment.id]);
  assert.equal(reopenedAttachment?.attachment.projectFile?.sourceSessionId, sourceSession);
  assert.equal(await readFile(reopenedAttachment!.filePath!, 'utf8'), 'Alpha output\n');
  await assert.rejects(runtime.chats.attachments.get(alphaProject.id, [staged.attachment.id]));

  const ordinary = await runtime.chats.createRecord(alphaProject.id);
  assert.equal(ordinary.scratch, undefined);
  assert.equal(await runtime.chats.executionDirectory(alphaProject.id, ordinary.id), alphaPath);
  await assert.rejects(runtime.removeWorkspace(sourceWorkspace), blocked);
  await assert.rejects(runtime.requestHive('DELETE', `/api/workspaces/${sourceWorkspace}`), blocked);

  // Hold one workspace's cleanup open while another project is linked.
  const betaProject = await runtime.requestHive('POST', '/api/workspaces', {
    path: betaPath, name: 'Already named Beta', autostart_orchestrator: false,
  }) as WorkspaceSummary;
  const independentPath = join(root, 'independent');
  await mkdir(independentPath);
  const independent = await runtime.requestHive('POST', '/api/workspaces', {
    path: independentPath, name: 'Independent project', autostart_orchestrator: false,
  }) as WorkspaceSummary;
  const drainEntered = Promise.withResolvers<void>();
  const resumeDrain = Promise.withResolvers<void>();
  const removal = runtime.chats.removeWorkspace(alphaProject.id, async () => {
    drainEntered.resolve();
    await resumeDrain.promise;
  });
  try {
    assert.throws(() => runtime!.chats.assertWorkspaceAvailable(alphaProject.id), blocked);
    await assert.rejects(runtime.chats.executionDirectory(alphaProject.id, ordinary.id), blocked);
    await withoutWaitingForRemoval(drainEntered.promise);
    await assert.rejects(withoutWaitingForRemoval(runtime.requestHive('DELETE', `/api/workspaces/${alphaProject.id}`)), blocked);
    await assert.rejects(withoutWaitingForRemoval(runtime.chats.promoteScratchProject(sourceWorkspace, sourceSession, 'Alpha again', runtime.requestHive)), blocked);
    const promotedBeta = await withoutWaitingForRemoval(runtime.chats.promoteScratchProject(sourceWorkspace, beta.id, 'Discarded title', runtime.requestHive));
    assert.equal(promotedBeta.workspace.id, betaProject.id);
    assert.equal(promotedBeta.workspace.name, 'Already named Beta');
    const both = await withoutWaitingForRemoval(runtime.chats.listScratchProjectLinks());
    assert.equal(both.links.length, 2);
    assert.equal((await withoutWaitingForRemoval(runtime.removeWorkspace(independent.id))).ok, true);
  } finally {
    resumeDrain.resolve();
    await removal;
  }

  await rename(betaPath, `${betaPath}-intact`);
  await mkdir(betaPath);
  await assert.rejects(runtime.chats.promoteScratchProject(sourceWorkspace, beta.id, 'Replaced', runtime.requestHive), /不存在或已被替换/);
  await rm(betaPath, {recursive: true});
  await rename(`${betaPath}-intact`, betaPath);

  assert.deepEqual((await runtime.chats.listScratchProjectLinks()).links.map(link => link.sourceSessionId), [beta.id]);
  assert.equal(await readFile(join(alphaPath, 'notes.txt'), 'utf8'), 'Alpha output\n');
  assert.equal(await readFile(sessionFile, 'utf8'), sourceTranscript);
  assert.deepEqual(await runtime.chats.projectFileStatus(sourceWorkspace, staged.attachment.id), {state: 'unchanged'});
  // Simulate removal through a Hive boundary that has no desktop link metadata.
  await runtime.hive.request('DELETE', `/api/workspaces/${betaProject.id}`);
  const replacement = await runtime.requestHive('POST', '/api/workspaces', {
    path: betaPath, name: 'Different Beta registration', autostart_orchestrator: false,
  }) as WorkspaceSummary;
  assert.notEqual(replacement.id, betaProject.id);
  assert.deepEqual(await runtime.chats.listScratchProjectLinks(), {links: []});
  await runtime.removeWorkspace(sourceWorkspace);
  assert((await runtime.requestHive('GET', '/api/workspaces') as WorkspaceSummary[]).some(item => item.id === replacement.id));
  assert.equal(await readFile(join(betaPath, 'notes.txt'), 'utf8'), 'Beta output\n');
  console.log('DESKTOP_SCRATCH_PROJECT_OK: cold lost-reply recovery, exact project reuse, original history and attachments, removal protection and file retention');
} finally {
  await runtime?.close();
  await rm(root, {recursive: true, force: true});
}

/** Compiled desktop wire and actual Hive upload storage, with cold recovery. */
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {randomUUID} from 'node:crypto';
import {once} from 'node:events';
import {mkdir, mkdtemp, readFile, rename, rm, unlink, writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join, resolve} from 'node:path';
import {createInterface} from 'node:readline';
import {DatabaseSync} from 'node:sqlite';

const fixture = await mkdtemp(join(tmpdir(), 'hive-draft-copy-'));
const dataDir = join(fixture, 'desktop');
const piDir = join(fixture, 'pi');
await mkdir(piDir);
const runtimePath = resolve('dist/main.js');
const png = 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+pZLcAAAAASUVORK5CYII=';
let sequence = 0;

function startRuntime() {
  const child = spawn(process.execPath, [runtimePath], {
    cwd: fixture, env: {...process.env, HIVE_DESKTOP_DATA_DIR: dataDir, PI_CODING_AGENT_DIR: piDir},
    stdio: ['pipe', 'pipe', 'pipe'],
  });
  const closed = once(child, 'close');
  const pending = new Map<number, {resolve(value: any): void; reject(error: Error): void}>();
  const lines = createInterface({input: child.stdout});
  let stderr = '';
  child.stderr.on('data', value => { stderr += value; });
  lines.on('line', value => {
    const frame = JSON.parse(value);
    pending.get(frame.id)?.resolve(frame);
  });
  child.on('close', () => {
    for (const request of pending.values()) request.reject(new Error(stderr));
    lines.close();
  });
  const rpc = async (method: string, params: unknown = {}) => {
    const id = ++sequence;
    let timer: NodeJS.Timeout | undefined;
    try {
      return await new Promise<any>((resolve, reject) => {
        pending.set(id, {resolve, reject});
        timer = setTimeout(() => reject(new Error(`Timed out: ${method}\n${stderr}`)), 15_000);
        child.stdin.write(JSON.stringify({id, method, params}) + '\n');
      });
    } finally {
      clearTimeout(timer);
      pending.delete(id);
    }
  };
  const ok = async (method: string, params: unknown = {}) => {
    const response = await rpc(method, params);
    assert.equal(response.error, undefined, `${method}: ${JSON.stringify(response.error)}`);
    return response.result;
  };
  const abort = async () => {
    if (child.exitCode === null && child.signalCode === null) child.kill('SIGKILL');
    await closed;
  };
  return {rpc, ok, abort, closed};
}

let runtime = startRuntime();
let db: DatabaseSync | undefined;
try {
  assert.ok((await runtime.ok('initialize')).hivePort);
  db = new DatabaseSync(join(dataDir, 'hive', 'runtime.sqlite'));
  const sourcePath = join(fixture, 'source');
  const targetPath = join(fixture, 'target');
  await Promise.all([mkdir(sourcePath), mkdir(targetPath)]);
  const [source, target] = await Promise.all([sourcePath, targetPath].map(path =>
    runtime.ok('hive.request', {method: 'POST', path: '/api/workspaces',
      body: {path, name: path.split('/').at(-1), autostart_orchestrator: false}})));
  const sourceSession = await runtime.ok('chat.createRecord', {workspaceId: source.id, backend: 'pi'});
  const existingTarget = await runtime.ok('chat.createRecord', {workspaceId: target.id, backend: 'pi'});
  // Opening an untouched editor may create a native session ID but does not bind it.
  const opened = await runtime.ok('chat.open', {sessionId: sourceSession.id});
  assert.equal(opened.identityState, 'new');
  assert.equal(opened.originAgentId, undefined);

  const regularPath = join(fixture, '草稿-regular.txt');
  const regularBytes = Buffer.from('original staged regular bytes\n');
  const snapshotBytes = Buffer.from('source project snapshot\n');
  await Promise.all([
    writeFile(regularPath, regularBytes),
    writeFile(join(sourcePath, 'snapshot.txt'), snapshotBytes),
    writeFile(join(targetPath, 'snapshot.txt'), 'different destination file\n'),
  ]);
  const workspaceId = source.id;
  const regular = await runtime.ok('chat.attachments.add', {workspaceId, path: regularPath});
  const image = await runtime.ok('chat.attachments.clipboard', {workspaceId,
    image: {type: 'image', mimeType: 'image/png', data: png}});
  const previewParams = {workspaceId, relativePath: 'snapshot.txt', sessionId: sourceSession.id};
  const sourceUploads = db.prepare('SELECT count(*) AS count FROM workspace_uploads WHERE workspace_id = ?');
  const countBeforePreview = sourceUploads.get(workspaceId);
  const wrongDigest = await runtime.rpc('project.files.attach', {...previewParams, expectedSha256: '0'.repeat(64)});
  assert.match(wrongDigest.error?.message ?? '', /文件在预览后已更改/);
  assert.deepEqual(sourceUploads.get(workspaceId), countBeforePreview);
  const preview = await runtime.ok('project.files.preview', previewParams);
  const scopedPreview = await runtime.ok('project.files.preview', {...previewParams, expectedDirectory: null});
  assert.equal(scopedPreview.executionDirectory, source.path);
  const wrongDirectory = await runtime.rpc('project.files.preview', {...previewParams, expectedDirectory: target.path});
  assert.match(wrongDirectory.error?.message ?? '', /execution directory changed/);
  const snapshot = await runtime.ok('project.files.attach', {...previewParams, expectedSha256: preview.sha256});
  assert.equal(snapshot.attachment.projectFile?.sha256, preview.sha256);
  const [regularId, imageId, snapshotId] = [regular, image, snapshot].map(item => item.attachment.id);
  assert.equal(snapshot.attachment.projectFile.sourceSessionId, sourceSession.id);
  await unlink(regularPath);
  await writeFile(join(sourcePath, 'snapshot.txt'), 'source working file changed after attachment\n');

  const base = {workspaceId, targetWorkspaceId: target.id, backend: 'pi', profile: sourceSession.profile};
  const sessionIds = async () => (await runtime.ok('chat.list', {workspaceId: target.id})).map((row: {id: string}) => row.id);
  const countUploads = () => (db!.prepare('SELECT count(*) AS count FROM workspace_uploads WHERE workspace_id = ?')
    .get(target.id) as {count: number}).count;
  const copyRequest = {...base, requestId: randomUUID(), sourceSessionId: sourceSession.id,
    attachmentIds: [regularId, imageId, snapshotId]};
  const results = await Promise.all([
    runtime.ok('chat.copyDraftRecord', copyRequest), runtime.ok('chat.copyDraftRecord', copyRequest),
  ]);
  assert.deepEqual(results[0], results[1]);
  const copied = results[0];
  assert.deepEqual(copied.session.profile, sourceSession.profile);
  assert.equal(copied.session.id, copyRequest.requestId);
  assert.equal(copied.session.executionPath, target.path);
  assert.equal(copied.session.identityState, 'new');
  assert.equal(copied.session.originAgentId, undefined);
  assert.equal(countUploads(), 3);
  assert.ok(copied.attachments.every((attachment: {id: string; workspaceId: string; projectFile: unknown}) =>
    !copyRequest.attachmentIds.includes(attachment.id) && attachment.workspaceId === target.id && !attachment.projectFile));
  assert.deepEqual(copied.attachments.map(({name, mimeType, size}: any) => ({name, mimeType, size})),
    [regular, image, snapshot].map(({attachment: {name, mimeType, size}}) => ({name, mimeType, size})));
  assert.ok((await runtime.rpc('chat.copyDraftRecord', {...copyRequest, attachmentIds: [imageId]})).error);
  assert.ok((await runtime.rpc('chat.copyDraftRecord', {...copyRequest, requestId: randomUUID(), workspaceId: target.id})).error);
  assert.ok((await runtime.rpc('chat.copyDraftRecord', {...copyRequest, requestId: randomUUID(), sourceSessionId: existingTarget.id})).error);

  const copiedUploads = async () => runtime.ok('chat.attachments.get',
    {workspaceId: target.id, ids: copied.attachments.map(({id}: {id: string}) => id)});
  const verifyBytes = async () => {
    const uploaded = await copiedUploads();
    assert.deepEqual(await readFile(uploaded[0].filePath), regularBytes);
    assert.equal(uploaded[1].image.data, png);
    assert.deepEqual(await readFile(uploaded[2].filePath), snapshotBytes);
    assert.equal(await readFile(join(targetPath, 'snapshot.txt'), 'utf8'), 'different destination file\n');
  };
  await verifyBytes();
  assert.ok((await runtime.rpc('chat.attachments.get', {workspaceId, ids: [copied.attachments[0].id]})).error);

  const invalid = {...base, requestId: randomUUID(), attachmentIds: [regularId, randomUUID()]};
  assert.ok((await runtime.rpc('chat.copyDraftRecord', invalid)).error);
  assert.equal(countUploads(), 3, 'A failure on the second attachment cannot publish the first.');
  assert.ok(!(await sessionIds()).includes(invalid.requestId));
  assert.deepEqual(await readFile(regular.filePath), regularBytes);

  // Lose destination ChatIndex publication after the Hive upload transaction.
  const interrupted = {...base, requestId: randomUUID(), attachmentIds: [imageId]};
  const index = join(dataDir, 'chats', 'index.json');
  await rename(index, index + '.good');
  await mkdir(index);
  try {
    assert.ok((await runtime.rpc('chat.copyDraftRecord', interrupted)).error);
    assert.equal(countUploads(), 4);
  } finally {
    await rm(index, {recursive: true});
    await rename(index + '.good', index);
  }

  await runtime.abort();
  runtime = startRuntime();
  assert.ok((await runtime.ok('initialize')).hivePort);
  // Simulate the desktop dying before it could apply the first successful response.
  assert.deepEqual(await runtime.ok('chat.copyDraftRecord', copyRequest), copied);
  await verifyBytes();
  assert.ok((await runtime.ok('workspace.remove', {workspaceId})).ok);
  assert.equal((await runtime.ok('chat.copyDraftRecord', interrupted)).attachments.length, 1,
    'A committed stage remains recoverable after the source project is removed.');
  assert.equal((await runtime.ok('chat.copyDraftRecord', interrupted)).session.id, interrupted.requestId);
  assert.deepEqual(new Set(await sessionIds()), new Set([existingTarget.id, copied.session.id, interrupted.requestId]));
  assert.equal(countUploads(), 4);
  assert.equal((db.prepare('SELECT COUNT(*) AS count FROM agent_runs').get() as {count: number}).count, 0);

  assert.ok((await runtime.ok('workspace.remove', {workspaceId: target.id})).ok);
  assert.equal(countUploads(), 0);
  assert.equal((db.prepare('SELECT COUNT(*) AS count FROM desktop_draft_copies').get() as {count: number}).count, 0);
  assert.ok(await runtime.ok('shutdown'));
  assert.equal((await runtime.closed)[0], 0);
  console.log(`DRAFT_COPY_WIRE_OK: exact cold replay, fresh destination IDs and original bytes, atomic upload publication, and no agent runs. Evidence: ${fixture}`);
} finally {
  db?.close();
  await runtime.abort();
}

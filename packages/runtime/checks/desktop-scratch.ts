import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { existsSync } from 'node:fs';
import { mkdir, mkdtemp, readFile, realpath, rm, symlink, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createDesktopRuntime } from '../src/desktop-runtime.js';
import { scratchWorkspace } from '../src/scratch-workspace.js';

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
    assert.deepEqual(runtime.chats.list(workspaceId).map(session => session.id), [requestId]);
  } finally {
    await runtime.close();
  }

  runtime = await createDesktopRuntime(data, () => {});
  try {
    const retry = await runtime.chats.createScratchRecord('pi', undefined, requestId,
      () => scratchWorkspace(data, runtime.requestHive));
    assert.equal(retry.workspace.id, workspaceId);
    assert.equal(retry.session.id, requestId);
    assert.equal(retry.session.sessionFile, null);
    assert.deepEqual(runtime.chats.list(workspaceId).map(session => session.id), [requestId]);
    const output = join(retry.workspace.path, 'notes.txt');
    await writeFile(output, 'Keep my notes.\n');
    await runtime.removeWorkspace(workspaceId);
    assert.deepEqual(await runtime.requestHive('GET', '/api/workspaces'), []);
    assert.equal(await readFile(output, 'utf8'), 'Keep my notes.\n');
  } finally {
    await runtime.close();
  }
  console.log('DESKTOP_SCRATCH_OK: guards, lost workspace reply, concurrent and cold retry, file retention');
} finally {
  await rm(root, { recursive: true, force: true });
}

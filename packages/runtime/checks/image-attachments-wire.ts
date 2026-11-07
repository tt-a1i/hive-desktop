/** Production JSONL + real Hive uploads/SQLite, no model requests. */
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createInterface } from 'node:readline';
import { createServer } from 'node:http';

const directory = await mkdtemp(join(tmpdir(), 'hive-images-wire-'));
const agentDir = join(directory, 'pi');
await mkdir(agentDir);
const project = join(directory, 'project');
await mkdir(project);
let modelRequests = 0;
const server = createServer((_, response) => {
  modelRequests++;
  response.writeHead(400);
  response.end();
});
server.listen(0, '127.0.0.1');
await once(server, 'listening');
const address = server.address();
assert.ok(address && typeof address !== 'string');
// Explicit text-only fixture prevents SDK fallback model selection or inference.
await writeFile(
  join(agentDir, 'models.json'),
  JSON.stringify({
    providers: {
      'wire-local': {
        api: 'openai-responses',
        baseUrl: `http://127.0.0.1:${address.port}/v1`,
        apiKey: 'fixture-only',
        models: [{ id: 'gpt-6.1-terra', name: 'Text-only capability fixture', input: ['text'] }],
      },
    },
  }),
);
await writeFile(
  join(agentDir, 'settings.json'),
  JSON.stringify({ defaultProvider: 'wire-local', defaultModel: 'gpt-6.1-terra' }),
);
const child = spawn(process.execPath, ['dist/main.js'], {
  cwd: process.cwd(),
  env: {
    ...process.env,
    PI_CODING_AGENT_DIR: agentDir,
    PI_OFFLINE: '1',
    HIVE_DESKTOP_DATA_DIR: join(directory, 'desktop'),
  },
  stdio: ['pipe', 'pipe', 'pipe'],
});
const exited = once(child, 'exit');
let stderr = '';
child.stderr.on('data', (chunk) => {
  stderr += String(chunk);
});
let sequence = 0;
const pending = new Map<number, { resolve(value: any): void; reject(error: Error): void }>();
const lines = createInterface({ input: child.stdout });
lines.on('line', (line) => {
  const response = JSON.parse(line);
  const request = pending.get(response.id);
  if (request) {
    pending.delete(response.id);
    request.resolve(response);
  }
});
child.on('exit', () => {
  for (const request of pending.values()) request.reject(new Error('Runtime exited before reply: ' + stderr));
  pending.clear();
});
function rpc(method: string, params = {}) {
  return new Promise<any>((resolve, reject) => {
    const id = ++sequence;
    const timeout = setTimeout(() => {
      pending.delete(id);
      reject(new Error('Image RPC timed out: ' + method));
    }, 10_000);
    pending.set(id, {
      resolve: (value) => {
        clearTimeout(timeout);
        resolve(value);
      },
      reject: (error) => {
        clearTimeout(timeout);
        reject(error);
      },
    });
    child.stdin.write(JSON.stringify({ id, method, params }) + '\n');
  });
}
try {
  assert.ok((await rpc('initialize')).result.hivePort);
  const create = await rpc('hive.request', {
    method: 'POST',
    path: '/api/workspaces',
    body: {
      path: project,
      name: 'Wire images',
      autostart_orchestrator: false,
    },
  });
  assert.equal(create.error, undefined);
  const workspaceId = create.result.id;
  const data = 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGP4z8AAAAMBAQDJ/pLvAAAAAElFTkSuQmCC';
  const path = join(directory, '图片.png');
  await writeFile(path, Buffer.from(data, 'base64'));
  const added = await rpc('chat.attachments.add', { workspaceId, path });
  assert.equal(added.error, undefined);
  assert.equal(added.result.attachment.name, '图片.png');
  assert.equal(added.result.image.data, data);
  const pasted = await rpc('chat.attachments.clipboard', { workspaceId, image: added.result.image });
  assert.equal(pasted.error, undefined);
  assert.equal(pasted.result.image.data, data);
  assert.equal(pasted.result.attachment.name, '粘贴图片.png');
  assert.notEqual(pasted.result.attachment.id, added.result.attachment.id);
  assert.deepEqual(
    (await rpc('chat.attachments.get', { workspaceId, ids: [pasted.result.attachment.id] })).result,
    [pasted.result],
  );
  assert.ok(
    (await rpc('chat.attachments.clipboard', { workspaceId, image: { ...added.result.image, data: 'bad' } }))
      .error,
  );
  assert.ok(
    (
      await rpc('chat.attachments.clipboard', {
        workspaceId,
        image: { ...added.result.image, mimeType: 'image/jpeg' },
      })
    ).error,
  );
  await writeFile(path, 'changed source');
  const ids = [added.result.attachment.id];
  assert.deepEqual((await rpc('chat.attachments.get', { workspaceId, ids })).result, [added.result]);
  assert.ok((await rpc('chat.attachments.get', { workspaceId, ids: ['../outside'] })).error);
  assert.ok((await rpc('chat.attachments.get', { workspaceId, ids: [...ids, ...ids] })).error);
  assert.ok((await rpc('chat.attachments.add', { workspaceId, path })).error);
  const created = await rpc('chat.create', { workspaceId, backend: 'pi' });
  assert.equal(created.error, undefined);
  const sessionId = created.result.id;
  const before = await rpc('chat.open', { sessionId });
  assert.ok((await rpc('chat.prompt', { sessionId, text: {}, attachmentIds: ids })).error);
  assert.ok((await rpc('chat.prompt', { sessionId, text: '', attachmentIds: [] })).error);
  assert.ok(
    (await rpc('chat.prompt', { sessionId, text: '', attachmentIds: [pasted.result.attachment.id] })).error,
    'Text-only Pi model must reject image-only input',
  );
  const after = await rpc('chat.open', { sessionId });
  assert.deepEqual(after.result.history, before.result.history);
  assert.equal(after.result.pending, 0);
  assert.equal(modelRequests, 0);
  // Upload remains recoverable after the failed send and independent source mutation.
  assert.equal((await rpc('chat.attachments.get', { workspaceId, ids })).result[0].image.data, data);
  assert.equal(await readFile(path, 'utf8'), 'changed source');
  assert.equal((await rpc('shutdown')).result.ok, true);
  await exited;
  assert.equal(child.exitCode, 0);
  console.log(
    'IMAGE_ATTACHMENTS_WIRE_OK: production JSONL upload/read snapshot, strict IDs and prompt inputs, failed-send history isolation, image-only text-model rejection and clean shutdown. No model requests.',
  );
} finally {
  if (child.exitCode === null && child.signalCode === null) {
    child.kill('SIGKILL');
    await exited;
  }
  lines.close();
  server.closeAllConnections();
  await new Promise<void>((resolve) => server.close(() => resolve()));
  await rm(directory, { recursive: true, force: true });
}

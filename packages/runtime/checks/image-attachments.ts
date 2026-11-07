/** Real upload storage, Hive user-input HTTP/SQLite and native Pi queue. No vendor inference. */
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { once } from 'node:events';
import { mkdtemp, mkdir, readFile, writeFile, rm } from 'node:fs/promises';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { DatabaseSync } from 'node:sqlite';
import { encodeImageInput, decodeImageInput } from '../src/image-attachments.js';
const dir = await mkdtemp(join(tmpdir(), 'hive-attachment-input-'));
const agentDir = join(dir, 'pi');
await mkdir(agentDir);
const data = 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGP4z8AAAAMBAQDJ/pLvAAAAAElFTkSuQmCC';
const bodies: any[] = [];
const server = createServer(async (request, response) => {
  const chunks = [];
  for await (const chunk of request) chunks.push(Buffer.from(chunk));
  bodies.push(JSON.parse(Buffer.concat(chunks).toString()));
  response.writeHead(400, { 'content-type': 'application/json' });
  response.end(
    JSON.stringify({
      error: { message: 'Intentional attachment loopback rejection', type: 'invalid_request_error' },
    }),
  );
});
server.listen(0, '127.0.0.1');
await once(server, 'listening');
const address = server.address();
assert.ok(address && typeof address !== 'string');
const prev = process.env.PI_CODING_AGENT_DIR;
process.env.PI_CODING_AGENT_DIR = agentDir;
await writeFile(
  join(agentDir, 'models.json'),
  JSON.stringify({
    providers: {
      'attachment-test': {
        api: 'openai-responses',
        baseUrl: `http://127.0.0.1:${address.port}/v1`,
        apiKey: 'local-only',
        models: [{ id: 'gpt-6.1-terra', name: 'Terra local attachment fixture', input: ['text', 'image'] }],
      },
    },
  }),
);
await writeFile(
  join(agentDir, 'settings.json'),
  JSON.stringify({ defaultProvider: 'attachment-test', defaultModel: 'gpt-6.1-terra' }),
);
const { createDesktopRuntime } = await import('../src/desktop-runtime.js');
let runtime: Awaited<ReturnType<typeof createDesktopRuntime>> | undefined;
async function wait<T>(fn: () => Promise<T>, ready: (value: T) => boolean) {
  const end = Date.now() + 10000;
  for (;;) {
    const value = await fn();
    if (ready(value)) return value;
    if (Date.now() > end) throw Error('Attachment flow timed out');
    await new Promise((resolve) => setTimeout(resolve, 15));
  }
}
try {
  runtime = await createDesktopRuntime(join(dir, 'state'), () => {});
  const project = join(dir, 'project');
  await mkdir(project);
  const other = join(dir, 'other');
  await mkdir(other);
  const ws = (await runtime.hive.request('POST', '/api/workspaces', {
    path: project,
    name: 'Images',
    autostart_orchestrator: false,
  })) as { id: string };
  const ws2 = (await runtime.hive.request('POST', '/api/workspaces', {
    path: other,
    name: 'Other',
    autostart_orchestrator: false,
  })) as { id: string };
  const path = join(dir, '原图.png');
  await writeFile(path, Buffer.from(data, 'base64'));
  const staged = await runtime.chats.attachments.add(ws.id, path);
  assert.equal(staged.image.data, data);
  assert.equal(staged.attachment.name, '原图.png');
  await writeFile(path, 'original path changed');
  assert.deepEqual((await runtime.chats.attachments.get(ws.id, [staged.attachment.id]))[0], staged);
  await assert.rejects(runtime.chats.attachments.get(ws2.id, [staged.attachment.id]));
  await assert.rejects(runtime.chats.attachments.get(ws.id, ['../../outside']));
  await assert.rejects(runtime.chats.attachments.add(ws.id, path));
  await assert.rejects(runtime.chats.attachments.add(ws.id, dir));
  const clipboard = await runtime.chats.attachments.addClipboard(ws.id, staged.image);
  assert.equal(clipboard.image.data, data);
  assert.equal(clipboard.attachment.name, '粘贴图片.png');
  assert.notEqual(clipboard.attachment.id, staged.attachment.id);
  await assert.rejects(runtime.chats.attachments.addClipboard(ws.id, { ...staged.image, data: 'bad' }));
  const encoded = encodeImageInput('原图核验', [staged.attachment.id]);
  assert.deepEqual(decodeImageInput(encoded), { text: '原图核验', ids: [staged.attachment.id] });
  assert.deepEqual(
    decodeImageInput(encodeImageInput(encoded, [])),
    { text: encoded, ids: [] },
    'Literal metadata in a desktop message must remain literal',
  );
  const chat = await runtime.chats.create(ws.id, 'pi');
  await assert.rejects(
    runtime.chats.prompt(chat.id, 'wrong workspace', ['00000000-0000-0000-0000-000000000000']),
  );
  assert.equal(bodies.length, 0);
  await runtime.chats.prompt(chat.id, '原图核验', [staged.attachment.id]);
  const snapshot = await wait(
    () => runtime!.chats.open(chat.id),
    (snapshot) => !snapshot.busy && snapshot.pending === 1,
  );
  assert.equal(bodies.length, 1);
  assert.ok(
    bodies[0].input
      .flatMap((x: any) => x.content ?? [])
      .some((x: any) => x.type === 'input_image' && x.image_url === `data:image/png;base64,${data}`),
  );
  const message = snapshot.history.find((x: any) => x.customType === 'hive-delivery') as any;
  assert.equal(message.presentation.displayText, '原图核验');
  assert.deepEqual(message.presentation.attachments, [staged.attachment]);
  assert.equal(message.presentation.sourceText, encoded);
  assert.ok(message.content.some((x: any) => x.type === 'image' && x.data === data));
  assert.ok(!message.content.some((x: any) => x.type === 'text' && x.text.includes('hive-desktop-images')));
  const database = new DatabaseSync(join(dir, 'state/hive/runtime.sqlite'), { readOnly: true });
  try {
    const row = database
      .prepare(
        "SELECT text FROM messages WHERE type='user_input' AND workspace_id=? ORDER BY sequence DESC LIMIT 1",
      )
      .get(ws.id) as { text: string };
    assert.equal(row.text, encoded);
  } finally {
    database.close();
  }
  await runtime.close();
  runtime = undefined;
  runtime = await createDesktopRuntime(join(dir, 'state'), () => {});
  assert.deepEqual((await runtime.chats.attachments.get(ws.id, [staged.attachment.id]))[0], staged);
  const reopened = await runtime.chats.open(chat.id);
  assert.equal(reopened.pending, 1);
  assert.ok(
    (reopened.history.find((x: any) => x.customType === 'hive-delivery') as any).content.some(
      (x: any) => x.type === 'image' && x.data === data,
    ),
  );
  await writeFile(path, Buffer.from(data, 'base64'));
  const otherImage = await runtime.chats.attachments.add(ws2.id, path);
  const imageOnly = await runtime.chats.create(ws2.id, 'pi');
  await runtime.chats.prompt(imageOnly.id, '', [otherImage.attachment.id]);
  await wait(
    () => runtime!.chats.open(imageOnly.id),
    (snapshot) => !snapshot.busy && snapshot.pending === 1,
  );
  assert.equal(bodies.length, 2);
  assert.ok(runtime.chats.list(ws.id).find((x) => x.id === chat.id)!.title.length > 0);
  console.log(
    'IMAGE_ATTACHMENTS_OK: actual upload and original-file mutation isolation, workspace validation, Hive HTTP+SQLite user-input association, native Pi image bytes, durable queue/history/reopen, image-only message and literal metadata escaping. Loopback only, no vendor inference.',
  );
} finally {
  await runtime?.close();
  server.closeAllConnections();
  await new Promise<void>((resolve) => server.close(() => resolve()));
  if (prev === undefined) delete process.env.PI_CODING_AGENT_DIR;
  else process.env.PI_CODING_AGENT_DIR = prev;
  await rm(dir, { recursive: true, force: true });
}

/** Real upload storage, Hive user-input HTTP/SQLite and native Pi queue. No vendor inference. */
import assert from 'node:assert/strict';
import { createHash, randomUUID } from 'node:crypto';
import { createServer } from 'node:http';
import { once } from 'node:events';
import { appendFile, mkdtemp, mkdir, readFile, writeFile, rm } from 'node:fs/promises';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { DatabaseSync } from 'node:sqlite';
import { ChatIndex } from '../src/chat-index.js';
import { encodeImageInput, decodeImageInput } from '../src/image-attachments.js';
import { desktopHistoryMessage } from '../src/desktop-history.js';
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
  const inputs = snapshot.history.filter((x: any) => x.role === 'user'
    && x.presentation?.attachments?.some((attachment: any) => attachment.id === staged.attachment.id)) as any[];
  assert.equal(inputs.length, 1, 'Explicit user input must remain a single native user message');
  const message = inputs[0];
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
  const restoredInputs = reopened.history.filter((x: any) => x.role === 'user'
    && x.presentation?.attachments?.some((attachment: any) => attachment.id === staged.attachment.id)) as any[];
  assert.equal(restoredInputs.length, 1, 'Cold reopen must not duplicate native user input');
  const restoredInput = restoredInputs[0];
  assert.deepEqual(restoredInput.content, message.content);
  assert.deepEqual(restoredInput.presentation, message.presentation);
  const messageId = reopened.historyMessageIds[reopened.history.indexOf(restoredInput)]!;
  const compact = desktopHistoryMessage(restoredInput, messageId);
  assert.deepEqual((compact as {desktopHistoryImages: unknown}).desktopHistoryImages, {messageId, count: 1});
  assert.equal(JSON.stringify(compact).includes(data), false, 'the native item must not duplicate deferred image bytes');
  assert.ok(JSON.stringify(restoredInput).includes(data), 'history storage and legacy callers preserve original bytes');
  assert.deepEqual(await runtime.chats.historyImage(ws.id, chat.id, messageId, 0), {messageId, imageIndex: 0, data});
  await assert.rejects(runtime.chats.historyImage(ws2.id, chat.id, messageId, 0), /another workspace/);
  await assert.rejects(runtime.chats.historyImage(ws.id, chat.id, messageId, 1), /image is unavailable/);
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

  await runtime.close();
  runtime = undefined;
  assert.ok(reopened.sessionFile && reopened.profile);
  const readOnlyId = randomUUID();
  const chatsDirectory = join(dir, 'state', 'chats');
  // A legacy record without a proven owner can display the transcript but cannot activate Pi.
  new ChatIndex(chatsDirectory).save({id: readOnlyId, workspaceId: ws.id, backend: 'pi', profile: reopened.profile,
    title: 'Legacy image history', identityState: 'unknown', sessionFile: reopened.sessionFile, updatedAt: Date.now()});
  runtime = await createDesktopRuntime(join(dir, 'state'), () => {});
  const cold = await runtime.chats.open(readOnlyId);
  assert.equal(cold.readOnly, true);
  const coldImage = cold.history.find((row: any) => row.presentation?.attachments?.some((attachment: any) => attachment.id === staged.attachment.id));
  assert.ok(coldImage);
  const coldMessageId = cold.historyMessageIds[cold.history.indexOf(coldImage)]!;
  assert.notEqual(coldMessageId, messageId, 'Live Pi uses the delivery ID while read-only history uses the saved entry ID');
  assert.deepEqual(await runtime.chats.historyImage(ws.id, readOnlyId, coldMessageId, 0), {messageId: coldMessageId, imageIndex: 0, data});

  const spool = join(chatsDirectory, 'deliveries', `${createHash('sha256').update(readOnlyId).digest('hex')}.json`);
  const invalidSpool = '{"pending":"invalid fixture"}\n';
  await writeFile(spool, invalidSpool);
  await assert.rejects(runtime.chats.open(readOnlyId), /Invalid delivery spool/);
  assert.deepEqual(await runtime.chats.historyImage(ws.id, readOnlyId, coldMessageId, 0), {messageId: coldMessageId, imageIndex: 0, data});
  assert.deepEqual(await runtime.chats.historyImage(ws.id, chat.id, messageId, 0), {messageId, imageIndex: 0, data});
  await assert.rejects(runtime.chats.historyImage(ws2.id, readOnlyId, coldMessageId, 0), /another workspace/);
  await assert.rejects(runtime.chats.historyImage(ws.id, readOnlyId, coldMessageId, 1), /image is unavailable/);

  const lastEntry = JSON.parse((await readFile(reopened.sessionFile, 'utf8')).trimEnd().split('\n').at(-1)!);
  // Pi displays a tool result under its tool call ID, so it can collide with a message ID.
  let parentId = lastEntry.id;
  for (const duplicateId of [coldMessageId, messageId]) {
    const entry = {type: 'message', id: randomUUID(), parentId,
      timestamp: new Date().toISOString(), message: {role: 'toolResult', toolCallId: duplicateId, toolName: 'fixture',
        content: [{type: 'text', text: 'A second row with the same display ID'}], timestamp: Date.now()}};
    await appendFile(reopened.sessionFile, JSON.stringify(entry) + '\n');
    parentId = entry.id;
    await assert.rejects(runtime.chats.historyImage(ws.id, readOnlyId, duplicateId, 0), /image message is unavailable/);
  }
  assert.equal(await readFile(spool, 'utf8'), invalidSpool);
  assert.equal(bodies.length, 2, 'Reading saved images must not make a model request');
  console.log(
    'IMAGE_ATTACHMENTS_OK: upload isolation, workspace validation, Hive HTTP+SQLite user-input association, native Pi image bytes, cold reads with corrupt spool or a prior live ID, ambiguous ID rejection, image-only message and literal metadata escaping. Loopback only, no vendor inference.',
  );
} finally {
  await runtime?.close();
  server.closeAllConnections();
  await new Promise<void>((resolve) => server.close(() => resolve()));
  if (prev === undefined) delete process.env.PI_CODING_AGENT_DIR;
  else process.env.PI_CODING_AGENT_DIR = prev;
  await rm(dir, { recursive: true, force: true });
}

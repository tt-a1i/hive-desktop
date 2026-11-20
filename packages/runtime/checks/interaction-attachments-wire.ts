/** Production JSONL, real Hive storage, and a controlled local Codex peer. */
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {once} from 'node:events';
import {mkdtemp, mkdir, readFile, rm, unlink, writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {createInterface} from 'node:readline';
import {fileURLToPath} from 'node:url';

const root = await mkdtemp(join(tmpdir(), 'hive-question-recovery-'));
const project = join(root, 'project');
const data = join(root, 'desktop');
const pi = join(root, 'pi');
await Promise.all([mkdir(project), mkdir(pi), mkdir(join(root, 'codex'))]);
const peer = fileURLToPath(new URL('./fixtures/codex-question-attachment-peer.cjs', import.meta.url));
const command = join(root, process.platform === 'win32' ? 'codex.cmd' : 'codex-fixture');
const shellQuote = (value: string) => "'" + value.replaceAll("'", "'\\''") + "'";
await writeFile(command, process.platform === 'win32'
  ? `@"${process.execPath}" "${peer}" %*\r\n`
  : `#!/bin/sh\nexec ${shellQuote(process.execPath)} ${shellQuote(peer)} "$@"\n`, {mode: 0o700});
const entry = process.env.HIVE_DESKTOP_TEST_RUNTIME ?? fileURLToPath(new URL('../dist/main.js', import.meta.url));
const sidecar = spawn(process.execPath, [entry], {cwd: root, stdio: ['pipe', 'pipe', 'pipe'], env: {
  ...process.env, HIVE_DESKTOP_DATA_DIR: data, HIVE_CODEX_COMMAND: command, CODEX_HOME: join(root, 'codex'),
  PI_CODING_AGENT_DIR: pi, PI_OFFLINE: '1',
}});
const exited = once(sidecar, 'exit');
let stderr = '';
sidecar.stderr.on('data', chunk => {stderr += String(chunk);});
const lines = createInterface({input: sidecar.stdout});
let sequence = 0;
const pending = new Map<number, (frame: any) => void>();
let publicEvents = '';
lines.on('line', line => {
  const frame = JSON.parse(line);
  if (frame.event) publicEvents += line + '\n';
  pending.get(frame.id)?.(frame);
});
async function frame(method: string, params: Record<string, unknown> = {}) {
  const id = ++sequence;
  let timer: ReturnType<typeof setTimeout> | undefined;
  try {
    return await new Promise<any>((resolve, reject) => {
      timer = setTimeout(() => reject(new Error(`${method} did not reply\n${stderr}`)), 10_000);
      pending.set(id, resolve);
      sidecar.stdin.write(JSON.stringify({id, method, params}) + '\n');
    });
  } finally {
    clearTimeout(timer);
    pending.delete(id);
  }
}
async function rpc(method: string, params: Record<string, unknown> = {}) {
  const reply = await frame(method, params);
  assert.equal(reply.error, undefined, JSON.stringify(reply.error));
  return reply.result;
}
async function wait<T>(read: () => Promise<T>, ready: (value: T) => boolean): Promise<T> {
  const deadline = Date.now() + 10_000;
  for (;;) {
    const value = await read();
    if (ready(value)) return value;
    assert.ok(Date.now() < deadline, 'Controlled peer did not reach the expected state');
    await new Promise(resolve => setTimeout(resolve, 20));
  }
}
async function responses() {
  try {
    return (await readFile(join(project, 'responses.jsonl'), 'utf8')).trim().split('\n').map(line => JSON.parse(line));
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === 'ENOENT') return [];
    throw error;
  }
}

try {
  await rpc('initialize');
  const workspace = await rpc('hive.request', {method: 'POST', path: '/api/workspaces',
    body: {path: project, name: 'Question recovery', autostart_orchestrator: false}});
  const {id: sessionId} = await rpc('chat.createRecord', {workspaceId: workspace.id, backend: 'codex'});
  await rpc('chat.prompt', {sessionId, text: 'Open a controlled question'});
  const chat = await wait(() => rpc('chat.open', {sessionId}), value => value.interactions.length === 1);
  const identity = {sessionId, interactionId: chat.interactions[0].id};
  const source = join(project, 'reselect.txt');
  await writeFile(source, 'original');
  const add = () => rpc('chat.attachments.add', {workspaceId: workspace.id, path: source});
  const first = await add();
  const second = await add(); // Same displayed filename, healthy copy, distinct upload ID.
  const mapping = {public: [first.attachment.id], extra: [second.attachment.id]};
  const draftParams = {...identity, attachmentsByQuestionId: mapping};
  const {receipt: draft} = await rpc('chat.interactionAttachments.save', draftParams);
  const copy = draft.attachments.find((item: any) => item.attachment.id === first.attachment.id).filePath;
  const secret = 'SECRET-ANSWER-FOR-CONTROLLED-PEER';
  const response = {answers: {public: {answers: ['keep answer']}, extra: {answers: ['healthy']}, secret: {answers: [secret]}}};
  const submitParams = {...draftParams, response};
  const expected = {
    code: 'QUESTION_ATTACHMENT_COPY_UNAVAILABLE',
    message: 'Question attachment copy is unavailable or changed. Remove it and select the file again.',
    attachmentId: first.attachment.id,
  };

  await writeFile(copy, 'replaced'); // Same-length corruption.
  assert.deepEqual((await frame('chat.interactionAttachments.save', draftParams)).error, expected);
  await unlink(copy);
  assert.deepEqual((await frame('chat.respondInteraction', submitParams)).error, expected);
  assert.deepEqual(await rpc('chat.interactionAttachments.status', identity), {safeToRetry: true});
  assert.deepEqual(await responses(), []);
  assert.deepEqual((await rpc('chat.interactionAttachments.list', {sessionId})).receipts, [draft]);

  // An already-prepared handoff takes precedence even while the copy is missing.
  const journal = join(data, 'chats', 'interaction-attachments', `${draft.id}.json`);
  await writeFile(journal, JSON.stringify({...draft, status: 'prepared'}));
  for (const [method, params] of [['chat.interactionAttachments.save', draftParams], ['chat.respondInteraction', submitParams]] as const) {
    assert.equal((await frame(method, params)).error.code, 'INTERACTION_STALE');
  }
  assert.deepEqual(await rpc('chat.interactionAttachments.status', identity), {safeToRetry: false});
  assert.deepEqual(await responses(), []);
  // This was a controlled journal injection; the peer received no reply.
  await writeFile(journal, JSON.stringify(draft));

  const removed = {extra: [second.attachment.id]};
  await rpc('chat.interactionAttachments.save', {...identity, attachmentsByQuestionId: removed});
  const replacement = await add();
  assert.notEqual(replacement.attachment.id, first.attachment.id);
  const recovered = {...removed, public: [replacement.attachment.id]};
  await rpc('chat.interactionAttachments.save', {...identity, attachmentsByQuestionId: recovered});
  const sent = await rpc('chat.respondInteraction', {...identity, response, attachmentsByQuestionId: recovered});
  const [reply] = await wait(responses, rows => rows.length === 1);
  assert.equal(sent.receipt.status, 'callback-handed');
  assert.equal(reply.id, 7);
  assert.deepEqual(reply.result.answers.secret, response.answers.secret);
  for (const [questionId, expectedAnswer] of [['public', 'keep answer'], ['extra', 'healthy']]) {
    const attachment = sent.receipt.attachments.find((item: any) => item.questionId === questionId);
    const [firstLine, ...lines] = reply.result.answers[questionId].answers[0].split('\n');
    assert.equal(firstLine, expectedAnswer);
    const references = lines.filter((line: string) => line.startsWith('{')).map((line: string) => JSON.parse(line));
    assert.deepEqual(references.map((ref: any) => ref.path), [attachment.filePath]);
    assert.equal(await readFile(attachment.filePath, 'utf8'), 'original');
  }
  assert.ok(!JSON.stringify(reply).includes(copy));
  assert.ok(!JSON.stringify(sent.receipt).includes(secret));
  assert.ok(!publicEvents.includes(secret));
  assert.ok(!stderr.includes(secret));
  assert.ok((await frame('chat.respondInteraction', {...identity, response})).error);
  await writeFile(join(project, 'finish'), '');
  await wait(() => rpc('chat.open', {sessionId}), value => value.interactions.length === 0);
  await rpc('shutdown');
  assert.equal((await exited)[0], 0, stderr);
  assert.equal((await responses()).length, 1);
  console.log('QUESTION_ATTACHMENT_RECOVERY_WIRE_OK: exact upload, sanitized save/submit errors, guarded receipt, remove/reselect, one native reply.');
} finally {
  if (sidecar.exitCode === null && sidecar.signalCode === null) {sidecar.kill(); await exited;}
  lines.close();
  await rm(root, {recursive: true, force: true});
}

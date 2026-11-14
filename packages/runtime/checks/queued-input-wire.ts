/** Production JSONL edit/cancel contract with real Hive/SQLite and a controlled Codex peer. */
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { mkdtemp, mkdir, writeFile, chmod, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createInterface } from 'node:readline';

const directory = await mkdtemp(join(tmpdir(), 'hive-queue-wire-'));
const project = join(directory, 'project');
const agent = join(directory, 'agent');
await mkdir(project); await mkdir(agent);
const peer = fileURLToPath(new URL('./fixtures/codex-queue-peer.cjs', import.meta.url));
const command = join(directory, process.platform === 'win32' ? 'peer.cmd' : 'peer');
const quote = (value: string) => `'${value.replaceAll("'", "'\\''")}'`;
await writeFile(command, process.platform === 'win32'
  ? `@"${process.execPath}" "${peer}" %*\r\n`
  : `#!/bin/sh\nexec ${quote(process.execPath)} ${quote(peer)} "$@"\n`);
await chmod(command, 0o700);
const entry = process.env.HIVE_DESKTOP_TEST_RUNTIME ?? fileURLToPath(new URL('../dist/main.js', import.meta.url));
const child = spawn(process.execPath, [entry], {cwd: directory, stdio: ['pipe', 'pipe', 'pipe'],
  env: {...process.env, HIVE_DESKTOP_DATA_DIR: join(directory, 'state'), HIVE_CODEX_COMMAND: command,
    PI_CODING_AGENT_DIR: agent, PI_OFFLINE: '1'}});
const exited = once(child, 'exit');
let diagnostics = '';
child.stderr.on('data', chunk => {diagnostics += String(chunk);});
let sequence = 0;
const pending = new Map<number, {resolve(value: any): void; reject(error: Error): void}>();
const lines = createInterface({input: child.stdout});
lines.on('line', line => {
  const response = JSON.parse(line);
  const request = pending.get(response.id);
  if (request) {pending.delete(response.id); request.resolve(response);}
});
child.on('exit', () => {
  for (const request of pending.values()) request.reject(new Error('Runtime exited before reply: ' + diagnostics));
  pending.clear();
});
async function rpc(method: string, params: Record<string, unknown> = {}) {
  const response = await new Promise<any>((resolve, reject) => {
    const id = ++sequence;
    const timer = setTimeout(() => {pending.delete(id); reject(new Error(`Timed out: ${method}`));}, 15_000);
    pending.set(id, {resolve: value => {clearTimeout(timer); resolve(value);}, reject: error => {clearTimeout(timer); reject(error);}});
    child.stdin.write(JSON.stringify({id, method, params}) + '\n');
  });
  if (response.error) throw new Error(response.error.message);
  return response.result;
}
try {
  await rpc('initialize');
  const workspace = await rpc('hive.request', {method: 'POST', path: '/api/workspaces',
    body: {path: project, name: 'Queue wire fixture', autostart_orchestrator: false}});
  const session = await rpc('chat.createRecord', {workspaceId: workspace.id, backend: 'codex'});
  const sessionId = session.id;
  await rpc('chat.prompt', {sessionId, text: 'Hold the controlled turn.'});
  const source = join(project, 'details.txt'); await writeFile(source, 'preserve attachment bytes\n');
  const upload = await rpc('chat.attachments.add', {workspaceId: workspace.id, path: source});
  await rpc('chat.prompt', {sessionId, text: 'Queued with attachment', attachmentIds: [upload.attachment.id]});
  await rpc('chat.prompt', {sessionId, text: 'Queued without attachment'});
  const opened = await rpc('chat.open', {sessionId});
  const attached = opened.pendingUserInputs.find((item: any) => item.text === 'Queued with attachment');
  const plain = opened.pendingUserInputs.find((item: any) => item.text === 'Queued without attachment');
  assert.ok(attached.editable && plain.editable);
  await assert.rejects(rpc('chat.queue.edit', {sessionId, deliveryId: attached.deliveryId, expectedText: attached.text}), /text.*(required|string)/i);
  assert.equal((await rpc('chat.queue.edit', {sessionId, deliveryId: attached.deliveryId,
    expectedText: attached.text, text: ''})).ok, true, 'The wire must permit attachment-only edits');
  const changed = (await rpc('chat.open', {sessionId})).pendingUserInputs.find((item: any) => item.deliveryId === attached.deliveryId);
  assert.equal(changed.text, '');
  assert.equal(changed.attachments[0].id, upload.attachment.id);
  await assert.rejects(rpc('chat.queue.edit', {sessionId, deliveryId: plain.deliveryId, expectedText: plain.text, text: ''}), /Message cannot be empty/);
  assert.equal((await rpc('chat.open', {sessionId})).pendingUserInputs.find((item: any) => item.deliveryId === plain.deliveryId).text, plain.text);
  await rpc('chat.queue.cancel', {sessionId, deliveryId: attached.deliveryId, expectedText: ''});
  await rpc('chat.queue.cancel', {sessionId, deliveryId: plain.deliveryId, expectedText: plain.text});
  const remaining = (await rpc('chat.open', {sessionId})).pendingUserInputs;
  assert.ok(!remaining.some((item: any) => item.deliveryId === attached.deliveryId || item.deliveryId === plain.deliveryId));
  await rpc('chat.abort', {sessionId});
  await rpc('shutdown');
  assert.equal((await exited)[0], 0, diagnostics);
  console.log('QUEUED_INPUT_WIRE_OK: empty text keeps original attachments, empty plain input rejected, missing edit text cannot cancel; real JSONL/Hive/SQLite, controlled peer only.');
} finally {
  if (child.exitCode === null && child.signalCode === null) {child.kill(); await exited;}
  lines.close();
  await rm(directory, {recursive: true, force: true});
}

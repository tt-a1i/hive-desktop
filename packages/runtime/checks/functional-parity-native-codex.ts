/** Opt-in actual Codex/Terra: edit/cancel, copied project reference, cold restore and body search. */
import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { mkdtemp, mkdir, writeFile, readFile, unlink, access } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { ChatIndex } from '../src/chat-index.js';
import { createDesktopRuntime } from '../src/desktop-runtime.js';

if (!process.argv.includes('--live')) throw new Error('Pass --live for an actual isolated Codex GPT-6.1 Terra check');
const command = process.env.HIVE_CODEX_COMMAND;
assert.ok(command, 'Explicit native Codex executable required');
const directory = await mkdtemp(join(tmpdir(), 'hive-functional-native-'));
const data = join(directory, 'state');
const project = join(directory, 'project');
await mkdir(join(project, 'src'), {recursive: true});
const marker = `proof-${randomUUID()}`;
const source = join(project, 'src', '引用 details.txt');
await writeFile(source, `marker=${marker}\n`);
const events: any[] = [];
const receipt: Record<string, unknown> = {pass: false, directory, model: 'openai/gpt-6.1-terra',
  actualCodex: true, vendorInference: true, originalSourceRemoved: false};
let runtime: Awaited<ReturnType<typeof createDesktopRuntime>> | undefined;
let id: string | undefined;
const boot = () => createDesktopRuntime(data, event => events.push(event), {
  executableEnvironment: {HIVE_CODEX_COMMAND: command},
});
async function wait(predicate: () => Promise<boolean>, label: string) {
  const end = Date.now() + 180_000;
  while (!await predicate()) {
    const state = await runtime!.chats.open(id!);
    assert.ok(!state.interactions?.some(interaction => interaction.state === 'pending'),
      'Native interaction needs user input; this check never grants permissions automatically');
    if (!state.busy && events.some(event => event.type === 'error')) {
      throw new Error(`Native run failed: ${events.filter(event => event.type === 'error').map(event => event.message).join('; ')}`);
    }
    assert.ok(Date.now() < end, label);
    await delay(100);
  }
}
try {
  runtime = await boot();
  const workspace = await runtime.hive.request('POST', '/api/workspaces', {
    path: project, name: 'Functional parity native proof', autostart_orchestrator: false,
  }) as {id: string};
  const record = await runtime.chats.createRecord(workspace.id, 'codex'); id = record.id;
  await runtime.close(); runtime = undefined;
  const index = new ChatIndex(join(data, 'chats'));
  index.save({...index.get(id), model: {provider: 'openai', id: 'gpt-6.1-terra', name: 'Terra functional proof'}});
  runtime = await boot();
  assert.equal((await runtime.chats.open(id)).model?.id, 'gpt-6.1-terra');
  await runtime.chats.prompt(id, 'For an interruption check, run exactly the read-only shell command sleep 45. Do not create files or any team. After the command finishes reply FIRST_TURN_FINISHED.');
  await wait(async () => events.some(event => event.type === 'tool_start' && JSON.stringify(event.args).includes('sleep 45')),
    'Native sleep tool did not start');
  const found = await runtime.chats.projectFilesSearch(workspace.id, '引用 details');
  assert.ok(found.paths.includes('src/引用 details.txt'));
  const uploaded = await runtime.chats.projectFilesAttach(workspace.id, 'src/引用 details.txt');
  assert.equal(uploaded.attachment.projectFile?.relativePath, 'src/引用 details.txt');
  assert.equal(await readFile(uploaded.filePath!, 'utf8'), `marker=${marker}\n`);
  await unlink(source); receipt.originalSourceRemoved = true;
  assert.equal((await runtime.chats.projectFileStatus(workspace.id, uploaded.attachment.id)).state, 'missing');
  const original = 'Original pending wording: do not process this old request before it is edited.';
  const edited = 'Read the attached saved text file using a real local tool. The original source was deleted; use the provided saved copy. Read the value after marker= and write only that exact value followed by one newline to result.txt in the project. Reply with that value. Do not create a team or repeat sleep.';
  const withdrawn = 'Canceled pending request: create the file canceled.txt with the text THIS_MUST_NOT_RUN.';
  await runtime.chats.prompt(id, original, [uploaded.attachment.id]);
  await runtime.chats.prompt(id, withdrawn);
  let state = await runtime.chats.open(id);
  const queued = state.pendingUserInputs.find(input => input.text === original)!;
  const canceled = state.pendingUserInputs.find(input => input.text === withdrawn)!;
  assert.ok(queued?.editable && canceled?.editable);
  await runtime.chats.mutateQueuedInput(id, queued.deliveryId, original, edited);
  await assert.rejects(runtime.chats.mutateQueuedInput(id, queued.deliveryId, original, 'stale overwrite'));
  await runtime.chats.mutateQueuedInput(id, canceled.deliveryId, withdrawn);
  state = await runtime.chats.open(id);
  assert.equal(state.pendingUserInputs.find(input => input.deliveryId === queued.deliveryId)?.text, edited);
  assert.ok(!state.pendingUserInputs.some(input => input.deliveryId === canceled.deliveryId));
  await runtime.chats.abort(id);
  await runtime.close(); runtime = await boot();
  const cold = await runtime.chats.open(id);
  assert.equal(cold.busy, false);
  assert.equal(cold.pendingUserInputs.length, 1);
  assert.equal(cold.pendingUserInputs[0]?.deliveryId, queued.deliveryId);
  assert.equal(cold.pendingUserInputs[0]?.text, edited);
  assert.deepEqual(cold.pendingUserInputs[0]?.attachments[0]?.projectFile, uploaded.attachment.projectFile);
  assert.equal((await runtime.chats.attachments.get(workspace.id, [uploaded.attachment.id]))[0]?.attachment.projectFile?.sha256,
    uploaded.attachment.projectFile?.sha256);
  await runtime.chats.resume(id);
  await wait(async () => {const snapshot = await runtime!.chats.open(id!); return !snapshot.busy && snapshot.pending === 0;},
    'Edited pending message did not complete');
  assert.equal(await readFile(join(project, 'result.txt'), 'utf8'), `${marker}\n`);
  await assert.rejects(access(join(project, 'canceled.txt')));
  const done = await runtime.chats.open(id);
  assert.equal(done.model?.id, 'gpt-6.1-terra');
  assert.equal(done.history.filter((message: any) => message.presentation?.deliveryId === queued.deliveryId).length, 1);
  assert.equal(done.history.filter((message: any) => message.presentation?.deliveryId === canceled.deliveryId).length, 0);
  const results = await runtime.chats.search(marker, false, 0, workspace.id);
  const hit = results.matches.find(hit => hit.session.id === id);
  assert.ok(hit?.bodyMatch, 'Actual model/tool output must be searchable by persisted body');
  receipt.pass = true;
  Object.assign(receipt, {chatId: id, threadId: done.codexThreadId, queuedDeliveryId: queued.deliveryId,
    canceledDeliveryId: canceled.deliveryId, sourceReference: uploaded.attachment.projectFile,
    markerReturned: true, independentFileBytesVerified: true, coldPaused: true, noCanceledDelivery: true,
    bodyMatchMessageId: hit.bodyMatch.messageId, searchIncompleteHistoryCount: results.incompleteHistoryCount});
} catch (error) {
  receipt.error = error instanceof Error ? error.message : String(error);
  process.exitCode = 1;
} finally {
  if (runtime) {
    if (id) await runtime.chats.abort(id).catch(error => {receipt.abortError = String(error);});
    await runtime.close();
  }
  const evidence = resolve('../../.artifacts/functional-parity/native-codex');
  await mkdir(evidence, {recursive: true});
  await writeFile(join(evidence, 'receipt.json'), JSON.stringify(receipt, null, 2));
  console.log(JSON.stringify(receipt));
}

/** Synthetic ACP peers, real MCP bridge, Hive HTTP/SQLite and native team CLI. No model requests. */
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createServer } from 'node:http';
import { once } from 'node:events';
import { createDesktopRuntime } from '../src/desktop-runtime.js';
import { createAcpSession } from '../src/agents/acp.js';
import { encodeImageInput } from '../src/image-attachments.js';
import type { Chats } from '../src/chats.js';
import { DeliveryQueue } from '../src/delivery-queue.js';

const dir = await mkdtemp(join(tmpdir(), 'hive-acp-team-'));
const agentDir = join(dir, 'agent'); await mkdir(agentDir);
execFileSync('git', ['init', '-q', dir]);
const oldAgent = process.env.PI_CODING_AGENT_DIR, oldOffline = process.env.PI_OFFLINE;
process.env.PI_CODING_AGENT_DIR = agentDir; process.env.PI_OFFLINE = '1';
let modelRequests = 0;
const modelServer = createServer((_req, res) => { modelRequests++; res.writeHead(500).end(); });
modelServer.listen(0, '127.0.0.1'); await once(modelServer, 'listening');
const address = modelServer.address(); assert(address && typeof address !== 'string');
await writeFile(join(agentDir, 'models.json'), JSON.stringify({ providers: { openai: { api: 'openai-responses', baseUrl: `http://127.0.0.1:${address.port}/v1`, apiKey: 'fixture', models: [{ id: 'gpt-6.1-terra', name: 'Fixture', input: ['text'] }] } } }));
await writeFile(join(agentDir, 'settings.json'), JSON.stringify({ defaultProvider: 'openai', defaultModel: 'gpt-6.1-terra' }));
const runtime = await createDesktopRuntime(join(dir, 'state'), () => {});
const privateTokens:string[]=[];const eventReceipts:string[]=[];
const sessions: Awaited<ReturnType<typeof createAcpSession>>[] = [];
const queues = (chats: Chats) => (chats as unknown as { live: Map<string, { queue: DeliveryQueue }> }).live;

async function checkColdAcpQueue() {
  const project = join(dir, 'queue-project'), data = join(dir, 'queue-state'), nativeRoot = join(dir, 'queue-native');
  await mkdir(project);
  const nativePeer = fileURLToPath(new URL('./fixtures/acp-runtime-peer.cjs', import.meta.url));
  const protocolPath = join(nativeRoot, 'protocol.jsonl');
  const events: unknown[] = [];
  let owned = await createDesktopRuntime(data, event => events.push(event));
  const text = 'Queued ACP text with an attachment';
  const editedText = 'Edited ACP text with the original attachment';
  const withdrawnText = 'Queued ACP text to withdraw';
  const resumeFixture = 'Initial ACP turn';
  try {
    const workspace = await owned.hive.request('POST', '/api/workspaces', {path: project, name: 'Cold ACP queue', autostart_orchestrator: false}) as {id: string};
    const profile = (await owned.profiles.create({driver: 'acp', label: 'Cold ACP fixture',
      launch: {command: process.execPath, args: [nativePeer, nativeRoot]}}, (await owned.profiles.list()).storeVersion)).profile;
    const {id} = await owned.chats.createRecord(workspace.id, 'acp', {profile: profile.ref});
    await owned.chats.prompt(id, resumeFixture);
    const queue = queues(owned.chats).get(id)!.queue;
    await queue.idle();
    assert.equal(queue.pendingCount, 0);
    await queue.pause();
    const path = join(project, 'original-notes.txt');
    await writeFile(path, 'Attachment bytes for the ordinary ACP cold queue');
    const {attachment} = await owned.chats.attachments.add(workspace.id, path);
    const endpoint = `/api/workspaces/${workspace.id}/user-input`;
    await owned.hive.request('POST', endpoint, {text: encodeImageInput(text, [attachment.id])});
    await owned.hive.request('POST', endpoint, {text: withdrawnText});
    assert.equal(queue.pendingCount, 2);
    const before = await owned.chats.open(id);
    assert.ok(before.sessionFile && before.acpSessionId);
    assert.equal(before.pendingUserInputs[0]?.attachments[0]?.id, attachment.id);
    const [edit, withdraw] = queue.pending;
    assert.ok(edit?.presentation?.messageSequence && withdraw?.presentation?.messageSequence);
    await owned.close();
    const protocolBefore = await readFile(protocolPath, 'utf8');
    const transcriptBefore = await readFile(before.sessionFile, 'utf8');
    const queueDir = join(data, 'chats', 'deliveries');
    const spoolPath = join(queueDir, createHash('sha256').update(id).digest('hex') + '.json');
    // Simulate receipts left in the spool by an interrupted cleanup. Deliberately
    // leave the sent and completed copies pending to exercise the transcript guard.
    const sent = {...withdraw, id: randomUUID()}, completed = {...withdraw, id: randomUUID()};
    const started = {...withdraw, id: randomUUID(), queueState: 'started'};
    const transcript = JSON.parse(transcriptBefore);
    await writeFile(before.sessionFile, JSON.stringify({...transcript, deliveries: {...transcript.deliveries,
      [sent.id]: 'sent', [completed.id]: 'completed'}}));
    const simulatedTranscript = await readFile(before.sessionFile, 'utf8');
    await writeFile(spoolPath, JSON.stringify([edit, withdraw, sent, completed, started]));
    owned = await createDesktopRuntime(data, event => events.push(event));
    events.length = 0;
    const cold = await owned.chats.open(id);
    assert.equal(cold.passive, true);
    assert.equal(cold.readOnly, false);
    assert.equal(cold.originAgentId, `${workspace.id}:orchestrator`);
    assert.equal(cold.acpSessionId, before.acpSessionId);
    assert.deepEqual(cold.profile, profile.ref);
    assert.deepEqual(cold.pendingUserInputs.map(input => [input.deliveryId, input.editable, input.reason]),
      [[edit.id, true, undefined], [withdraw.id, true, undefined],
        [sent.id, false, 'attempted'], [completed.id, false, 'completed'], [started.id, false, 'attempted']]);
    for (const item of [sent, completed, started]) {
      await assert.rejects(owned.chats.mutateQueuedInput(id, item.id, withdrawnText, 'forbidden edit'), /Only unstarted/);
      await assert.rejects(owned.chats.mutateQueuedInput(id, item.id, withdrawnText), /Only unstarted/);
    }
    await writeFile(before.sessionFile, JSON.stringify({...transcript, sessionId: 'another-session'}));
    const wrongIdentity = await owned.chats.open(id);
    assert.ok(wrongIdentity.historyWarning?.includes('does not match'));
    assert.ok(wrongIdentity.pendingUserInputs.every(item => !item.editable));
    await assert.rejects(owned.chats.mutateQueuedInput(id, edit.id, text, editedText), /saved ACP session/);
    await writeFile(before.sessionFile, simulatedTranscript);
    const replacement = await owned.chats.createRecord(workspace.id, 'acp', {profile: profile.ref});
    const index = (owned.chats as unknown as {index: import('../src/chat-index.js').ChatIndex}).index;
    index.bind(replacement.id, cold.originAgentId!);
    const unbound = await owned.chats.open(id);
    assert.equal(unbound.originAgentId, cold.originAgentId);
    assert.ok(unbound.pendingUserInputs.every(item => !item.editable));
    await assert.rejects(owned.chats.mutateQueuedInput(id, edit.id, text, editedText), /saved ACP session/);
    index.bind(id, cold.originAgentId!);
    const correctedText = 'Text already corrected in the authoritative source';
    owned.hive.mutateUserInput(workspace.id, cold.originAgentId!, edit.presentation.messageSequence!,
      encodeImageInput(text, [attachment.id]), encodeImageInput(correctedText, [attachment.id]), () => {});
    await assert.rejects(owned.chats.mutateQueuedInput(id, edit.id, text, editedText), /changed; reopen/);
    await owned.chats.mutateQueuedInput(id, edit.id, correctedText, editedText);
    await owned.chats.mutateQueuedInput(id, withdraw.id, withdrawnText);
    const persisted = DeliveryQueue.persistedItems(queueDir, id);
    assert.deepEqual(persisted.slice(1), [sent, completed, started]);
    assert.equal(persisted[0]?.id, edit.id);
    assert.deepEqual(persisted[0]?.presentation?.attachments, edit.presentation.attachments);
    assert.equal(persisted[0]?.presentation?.messageSequence, edit.presentation.messageSequence);
    assert.deepEqual({...owned.hive.getUserInput(workspace.id, cold.originAgentId!, edit.presentation.messageSequence!)},
      {type: 'user_input', text: encodeImageInput(editedText, [attachment.id])});
    assert.deepEqual({...owned.hive.getUserInput(workspace.id, cold.originAgentId!, withdraw.presentation.messageSequence!)},
      {type: 'user_input_cancelled', text: withdrawnText});
    const changes = events.filter((event): event is {type: string; sessionId: string; pendingUserInputs: {deliveryId: string; text: string}[]} =>
      !!event && typeof event === 'object' && 'type' in event && event.type === 'state'
      && 'sessionId' in event && event.sessionId === id);
    assert.equal(changes.length, 3);
    assert.equal(changes[0]?.pendingUserInputs.find(item => item.deliveryId === edit.id)?.text, correctedText);
    assert.equal(changes[1]?.pendingUserInputs.find(item => item.deliveryId === edit.id)?.text, editedText);
    assert.ok(!changes[2]?.pendingUserInputs.some(item => item.deliveryId === withdraw.id));
    assert.equal(await readFile(protocolPath, 'utf8'), protocolBefore);
    assert.equal(await readFile(before.sessionFile, 'utf8'), simulatedTranscript);

    // Retire only the synthetic protection cases before the normal resume.
    await owned.close();
    await writeFile(spoolPath, JSON.stringify([persisted[0]]));
    await writeFile(before.sessionFile, transcriptBefore);
    owned = await createDesktopRuntime(data, event => events.push(event));
    const reopened = await owned.chats.open(id);
    assert.deepEqual(reopened.pendingUserInputs.map(input => [input.deliveryId, input.text, input.attachments[0]?.id]),
      [[edit.id, editedText, attachment.id]]);
    assert.equal(await readFile(protocolPath, 'utf8'), protocolBefore);
    await owned.chats.resume(id);
    await queues(owned.chats).get(id)!.queue.idle();
    const resumed = await owned.chats.open(id);
    assert.equal(resumed.acpSessionId, before.acpSessionId);
    assert.equal(resumed.pending, 0);
    const nativeHistory = JSON.parse(await readFile(join(nativeRoot, before.acpSessionId + '.json'), 'utf8'));
    const inputs: string[] = nativeHistory.history.filter((message: {kind: string}) => message.kind === 'user_message_chunk')
      .map((message: {text: string}) => message.text);
    assert.equal(inputs.length, 2);
    assert.ok(inputs[0]?.includes(resumeFixture));
    assert.ok(inputs[1]?.includes(editedText) && inputs[1]?.includes(attachment.name));
    assert.ok(inputs.every(input => !input.includes(withdrawnText) && !input.includes(text)));
    const afterProtocol = await readFile(protocolPath, 'utf8');
    const newCalls = afterProtocol.slice(protocolBefore.length).trim().split('\n').map(line => JSON.parse(line));
    assert.equal(newCalls.filter(call => call.method === 'session/prompt').length, 1);
    assert.ok(newCalls.some(call => call.method === 'session/load' && call.sessionId === before.acpSessionId));
    const evidence = await mkdtemp(join(tmpdir(), 'hive-acp-cold-queue-'));
    await writeFile(join(evidence, 'protocol-before.jsonl'), protocolBefore);
    await writeFile(join(evidence, 'protocol-after-resume.jsonl'), afterProtocol);
    await writeFile(join(evidence, 'receipt.json'), JSON.stringify({pass: true, sessionId: id,
      nativeSessionId: before.acpSessionId, deliveryId: edit.id, attachmentId: attachment.id,
      protected: ['sent', 'completed', 'started', 'transcript identity', 'stale editor', 'unbound original session'],
      nativeLogUnchangedWhileCold: true, sourceAndSpoolAgree: true, stateEvents: changes, newNativePrompts: 1}, null, 2));
    console.log('Ordinary cold ACP queue edit and withdrawal passed; evidence:', evidence);
  } finally {
    await owned.close();
  }
}

try {
  const workspace = await runtime.hive.request('POST', '/api/workspaces', { path: dir, name: 'ACP team', autostart_orchestrator: false }) as { id: string };
  const chat = await runtime.chats.create(workspace.id); await runtime.chats.resume(chat.id);
  const orchestrator = `${workspace.id}:orchestrator`;
  const peer = fileURLToPath(new URL('./fixtures/acp-team-peer.cjs', import.meta.url));
  async function session(agentId: string) {
    const token = runtime.agents.peekAgentToken(agentId); assert(token);privateTokens.push(token);
    const handle = await createAcpSession({ command: process.execPath, args: [peer], env:{HIVE_ACP_TEAM_BINDING:'stale-binding',HIVE_ACP_TEAM_PROOF:'stale-proof'}, cwd: dir, sessionDir: dir, onEvent: event=>eventReceipts.push(JSON.stringify(event)), teamBinding: () => ({ workspaceId: workspace.id, agentId, token, hivePort: String(runtime.hive.port), cwd: dir }) });
    sessions.push(handle);
    for (let i = 0; i < 100 && !handle.teamConnected(); i++) await new Promise(r => setTimeout(r, 10));
    assert(handle.teamConnected()); return handle;
  }
  async function call(handle: Awaited<ReturnType<typeof session>>, args: string[] | null) {
    await handle.prompt(JSON.stringify(args));
    const last = handle.history().at(-1) as { content: Array<{ text: string }> };
    const result = JSON.parse(last.content[0]!.text);
    assert(!result.isError, JSON.stringify(result));
    return result.content[0].text;
  }
  const lead = await session(orchestrator);
  // A peer that reflects the temporary descriptor must not leak its token to history/events.
  const reflected=await call(lead,['__fixture_reflect_private_descriptor']);
  assert(!reflected.includes('socketPath')&&!reflected.includes('hivePort'),'both complete private descriptor values must be redacted');
  await call(lead, ['spawn', 'coder', '--name', 'acp-worker']);
  const members = await runtime.hive.request('GET', `/api/ui/workspaces/${workspace.id}/team`) as Array<{ id: string; name: string }>;
  const worker = members.find(m => m.name === 'acp-worker'); assert(worker);
  await runtime.hive.request('POST', `/api/workspaces/${workspace.id}/agents/${encodeURIComponent(worker.id)}/start`);
  const workerChat = runtime.chats.list(workspace.id).find(c => c.agentId === worker.id); assert(workerChat);
  await queues(runtime.chats).get(workerChat.id)!.queue.pause();
  await queues(runtime.chats).get(chat.id)!.queue.pause();
  const dispatchId = JSON.parse(await call(lead, ['send', 'acp-worker', 'fixture task'])).dispatch_id as string;
  const detail = () => runtime.requestHive('GET', `/api/ui/workspaces/${workspace.id}/dispatches/${dispatchId}`) as Promise<{ state: string; report_text: string | null }>;
  assert.equal((await detail()).state, 'submitted');
  const workerSession = await session(worker.id);
  await call(workerSession, null);
  assert.equal((await detail()).state, 'submitted', 'ACP end_turn must not report dispatch');
  const inbox = JSON.parse(await call(workerSession, ['messages', '--dispatch', dispatchId]));
  await call(workerSession, ['report', 'fixture report', '--dispatch', dispatchId, '--seen', String(inbox.required_seen_seq)]);
  assert.equal((await detail()).state, 'reported');
  assert.equal((await detail()).report_text, 'fixture report');
  assert(queues(runtime.chats).get(chat.id)!.queue.pending.some(item => item.presentation?.dispatchId === dispatchId && item.presentation.displayText === 'fixture report'));
  // Changing the current Worker conversation must not retarget the original dispatch source.
  const workerRun = runtime.agents.getActiveRunByAgentId(workspace.id, worker.id)!;
  runtime.agents.stopAgentRun(workerRun.runId);
  assert.equal(await runtime.agents.waitForRunExit(workerRun.runId, 5000), true);
  const replacement = await runtime.chats.createRecord(workspace.id, 'pi');
  const index = (runtime.chats as unknown as { index: import('../src/chat-index.js').ChatIndex }).index;
  index.bind(replacement.id, worker.id);
  await runtime.chats.open(replacement.id);
  await queues(runtime.chats).get(replacement.id)!.queue.pause();
  const source = await runtime.chats.dispatchSession(workspace.id, dispatchId, worker.id);
  assert.equal(source.sourceSessionId, workerChat.id);
  assert.equal(source.session.originAgentId, worker.id);
  assert.equal(index.forAgent(workspace.id, worker.id)!.id, replacement.id);
  for (const session of sessions) {
    const transcript = await readFile(session.sessionFile!, 'utf8');
    for (const token of privateTokens) assert(!transcript.includes(token), 'team tokens must never be persisted');
    assert(!transcript.includes('socketPath'),'private proof descriptor must never be persisted');
  }
  assert(!eventReceipts.some(event=>event.includes('socketPath')),'private proof descriptor must not leak in events');
  for(const token of privateTokens)assert(!eventReceipts.some(event=>event.includes(token)),'reflected descriptors must not leak tokens in events');
  for (const session of sessions.splice(0)) await session.dispose();
  await runtime.close();
  await checkColdAcpQueue();
  assert.equal(modelRequests, 0);
  console.log('ACP team: real Hive spawn, dispatch, end_turn, seen, report and original orchestrator delivery passed; no model inference');
} finally {
  for (const session of sessions) await session.dispose();
  await runtime.close(); await new Promise<void>(resolve => modelServer.close(() => resolve()));
  if (oldAgent === undefined) delete process.env.PI_CODING_AGENT_DIR; else process.env.PI_CODING_AGENT_DIR = oldAgent;
  if (oldOffline === undefined) delete process.env.PI_OFFLINE; else process.env.PI_OFFLINE = oldOffline;
  await rm(dir, { recursive: true, force: true });
}

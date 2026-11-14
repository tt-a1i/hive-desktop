// Real Hive HTTP/SQLite and paused Pi SDK sessions; no vendor/model calls.
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { DatabaseSync } from 'node:sqlite';
import { createDesktopRuntime } from '../src/desktop-runtime.js';
import type { Chats } from '../src/chats.js';
import type { DeliveryQueue } from '../src/delivery-queue.js';
const directory = await mkdtemp(join(tmpdir(), 'hive-queue-mutation-'));
const agent = join(directory, 'agent'); await mkdir(agent);
await writeFile(join(agent, 'settings.json'), JSON.stringify({ defaultProvider: 'openai', defaultModel: 'gpt-6.1-terra' }));
const previous = process.env.PI_CODING_AGENT_DIR, offline = process.env.PI_OFFLINE;
process.env.PI_CODING_AGENT_DIR = agent; process.env.PI_OFFLINE = '1';
const queueOf = (chats: Chats, id: string) => (chats as unknown as { live: Map<string, { queue: DeliveryQueue }> }).live.get(id)!.queue;
let runtime: Awaited<ReturnType<typeof createDesktopRuntime>> | undefined;
try {
 const events: any[] = [];
 const state = join(directory, 'state'); runtime = await createDesktopRuntime(state, event => events.push(event));
 const workspace = await runtime.hive.request('POST', '/api/workspaces', { path: directory, name: 'Queue mutation', autostart_orchestrator: false }) as { id: string };
 const chat = await runtime.chats.create(workspace.id); let queue = queueOf(runtime.chats, chat.id); queue.resume = () => {};
 const file = join(directory, 'evidence.txt'); await writeFile(file, 'fixed attachment');
 const attachment = await runtime.chats.attachments.add(workspace.id, file);
 await runtime.chats.prompt(chat.id, 'duplicate', [attachment.attachment.id]);
 await runtime.chats.prompt(chat.id, 'duplicate');
 const first = queue.pending[0]!, second = queue.pending[1]!;
 assert.ok(first.presentation?.messageSequence); assert.ok(second.presentation?.messageSequence);
 assert.notEqual(first.presentation?.messageSequence, second.presentation?.messageSequence);
 await runtime.chats.mutateQueuedInput(chat.id, first.id, 'duplicate', 'edited');
 assert.equal(queue.pending[0]!.presentation!.displayText, 'edited');
 assert.deepEqual(queue.pending[0]!.presentation!.attachments, first.presentation!.attachments);
 assert.ok(queue.pending[0]!.text.includes('evidence.txt'));
 assert.ok(queue.pending[0]!.presentation!.sourceText?.startsWith('edited'));
 await assert.rejects(runtime.chats.mutateQueuedInput(chat.id, first.id, 'duplicate', 'stale'), /changed/);
 // Protected team delivery and a legacy orchestrator input cannot be mutated.
 await queue.enqueue('protected dispatch', true, { version: 1, origin: 'dispatch', displayText: 'protected dispatch', dispatchId: 'protected' });
 const protectedItem = queue.pending.at(-1)!;
 await assert.rejects(runtime.chats.mutateQueuedInput(chat.id, protectedItem.id, 'protected dispatch', 'unsafe'), /unstarted/);
 await queue.enqueue('legacy', true, { version: 1, origin: 'user', displayText: 'legacy' });
 const legacy = queue.pending.at(-1)!;
 const legacyVisible = (await runtime.chats.open(chat.id)).pendingUserInputs.find(input => input.deliveryId === legacy.id)!;
 assert.equal(legacyVisible.editable, false); assert.equal(legacyVisible.reason, 'legacy');
 const legacyState = events.findLast(event => event.type === 'state' && event.pendingUserInputs?.some((input: any) => input.deliveryId === legacy.id));
 assert.ok(legacyState); assert.equal(legacyState.pendingUserInputs.find((input: any) => input.deliveryId === legacy.id).editable, false);
 assert.equal(legacyState.pendingUserInputs.find((input: any) => input.deliveryId === legacy.id).reason, 'legacy');

 await assert.rejects(runtime.chats.mutateQueuedInput(chat.id, legacy.id, 'legacy', 'unsafe'), /unstarted/);
 await queue.pause(); queue.discardPending(new Set([protectedItem.id, legacy.id]));
 const db = new DatabaseSync(join(state, 'hive/runtime.sqlite'));
 const row = (sequence: number) => db.prepare('SELECT type,text FROM messages WHERE sequence=?').get(sequence) as { type: string; text: string };
 assert.ok(row(first.presentation!.messageSequence!).text.startsWith('edited'));
 assert.equal(row(second.presentation!.messageSequence!).text, 'duplicate');
 // An error after the spool rename but before SQLite COMMIT must restore old text.
 const mutate = runtime.hive.mutateUserInput.bind(runtime.hive);
 runtime.hive.mutateUserInput = (w,a,s,e,r,persist) => mutate(w,a,s,e,r,() => { persist(); throw new Error('injected precommit failure'); });
 await assert.rejects(runtime.chats.mutateQueuedInput(chat.id, first.id, 'edited', 'not committed'), /precommit/);
 assert.equal(queue.pending[0]!.presentation!.displayText, 'edited');
 await assert.rejects(runtime.chats.mutateQueuedInput(chat.id, second.id, 'duplicate'), /precommit/);
 assert.equal(queue.pendingUserInputs.find(input => input.deliveryId === second.id)!.editable, true);
 runtime.hive.mutateUserInput = mutate;
 await runtime.chats.mutateQueuedInput(chat.id, second.id, 'duplicate');
 assert.equal(row(second.presentation!.messageSequence!).type, 'user_input_cancelled');
 assert.equal(queue.pending.some(item => item.id === second.id), false);
 assert.ok(Number((db.prepare("SELECT COUNT(*) AS n FROM messages WHERE type IN ('user_input_edit_audit','user_input_cancel_audit')").get() as { n: number }).n) >= 2);
 // Simulate process death after spool edit, before SQL commit.
 const { replaceQueuedInput } = await import('../src/queued-input-mutation.js');
 queue.editPendingUserInput(first.id, replaceQueuedInput(queue.pending[0]!, 'crash-only edit'));
 db.close(); await runtime.close(); runtime = await createDesktopRuntime(state, () => {});
 await runtime.chats.open(chat.id); queue = queueOf(runtime.chats, chat.id);
 assert.equal(queue.pending[0]!.presentation!.displayText, 'edited');
 // Simulate death before cancellation commit: retained tombstone restores original pending input.
 queue.cancelPendingUserInput(first.id, { retain: true });
 await runtime.close(); runtime = await createDesktopRuntime(state, () => {}); await runtime.chats.open(chat.id); queue = queueOf(runtime.chats, chat.id);
 assert.equal(queue.pendingUserInputs[0]!.editable, true);
 const current = queue.pending[0]!;
 // Simulate death after SQL commit but before final tombstone cleanup.
 runtime.hive.mutateUserInput(workspace.id, `${workspace.id}:orchestrator`, current.presentation!.messageSequence!, current.presentation!.sourceText!, null,
  () => queue.cancelPendingUserInput(first.id, { retain: true }));
 await runtime.close(); runtime = await createDesktopRuntime(state, () => {}); await runtime.chats.open(chat.id); queue = queueOf(runtime.chats, chat.id);
 assert.equal(queue.pendingCount, 0);
 // Direct worker input deliberately has no orchestrator user_input ledger row.
 const member = runtime.createMember(workspace.id, { name: 'Queue worker', role: 'coder', backend: 'pi' });
 await runtime.agents.startAgent({ id: workspace.id, path: directory, name: 'Queue mutation' }, member.id, { hivePort: String(runtime.hive.port) });
 const workerChat = runtime.chats.list(workspace.id).find(record => record.agentId === member.id)!;
 const workerQueue = queueOf(runtime.chats, workerChat.id); workerQueue.resume = () => {}; await workerQueue.pause();
 await runtime.chats.prompt(workerChat.id, 'worker original');
 const workerInput = workerQueue.pending.find(item => item.presentation?.origin === 'user')!;
 assert.ok(workerInput); assert.equal(workerInput.presentation!.messageSequence, undefined);
 assert.equal((await runtime.chats.open(workerChat.id)).pendingUserInputs.find(input => input.deliveryId === workerInput.id)!.editable, true);
 await runtime.chats.mutateQueuedInput(workerChat.id, workerInput.id, 'worker original', 'worker edited');
 assert.equal(workerQueue.pending.find(item => item.id === workerInput.id)!.presentation!.displayText, 'worker edited');
 await runtime.chats.mutateQueuedInput(workerChat.id, workerInput.id, 'worker edited');
 assert.equal(workerQueue.pending.some(item => item.id === workerInput.id), false);

 console.log('QUEUED_INPUT_MUTATION_OK: exact duplicate provenance, immutable attachment/suffix, CAS, SQL audit/cancel, precommit rollback, stale-edit and before/after cancel crash recovery. Real Hive HTTP/SQLite, no model calls.');
} finally {
 await runtime?.close();
 if (previous === undefined) delete process.env.PI_CODING_AGENT_DIR; else process.env.PI_CODING_AGENT_DIR = previous;
 if (offline === undefined) delete process.env.PI_OFFLINE; else process.env.PI_OFFLINE = offline;
 await rm(directory, { recursive: true, force: true });
}

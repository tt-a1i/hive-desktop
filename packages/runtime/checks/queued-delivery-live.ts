import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdir, mkdtemp, readFile, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { createDesktopRuntime } from '../src/desktop-runtime.js';
import type { ChatEvent } from '../src/chats.js';
import type { DeliveryItem } from '../src/delivery-queue.js';

if (!process.argv.includes('--live')) throw new Error('Pass --live to run real Pi requests with openai/gpt-6.1-terra');
const artifacts = resolve('../../.artifacts');
await mkdir(artifacts, { recursive: true });
const evidence = await mkdtemp(join(artifacts, 'queued-delivery-live-'));
const directory = await mkdtemp(join(tmpdir(), 'hive-queued-live-'));
const project = join(directory, 'project');
await mkdir(project);
const events: (ChatEvent & { phase: string })[] = [];
const receipt: Record<string, unknown> = { evidence, directory, scope: 'Pi builtin; direct Hive runtime delivery API; no two-agent loop', model: 'openai/gpt-6.1-terra' };
let phase = 'initial';
let runtime: Awaited<ReturnType<typeof createDesktopRuntime>> | undefined;
const emit = (event: ChatEvent) => { events.push({ ...event, phase }); if (['tool_start', 'settled', 'error'].includes(event.type)) console.log(JSON.stringify({ phase, ...event })); };
const wait = async (fn: () => Promise<boolean>, label: string) => {
 const deadline = Date.now() + 180_000;
 while (Date.now() < deadline) { if (await fn()) return; await new Promise(r => setTimeout(r, 100)); }
 throw new Error(`Timed out: ${label}`);
};
const hasText = (history: readonly unknown[], value: string) => history.some(m => {
 const item = m as { role?: string; content?: { type?: string; text?: string }[] };
 return item.role === 'assistant' && Array.isArray(item.content) && item.content.some(p => p.type === 'text' && p.text?.includes(value));
});
try {
 runtime = await createDesktopRuntime(join(directory, 'state'), emit);
 const workspace = await runtime.hive.request('POST', '/api/workspaces', { path: project, name: 'Queued delivery live', autostart_orchestrator: false }) as { id: string };
 const chat = await runtime.chats.create(workspace.id, 'pi');
 await runtime.chats.setModel(chat.id, 'openai', 'gpt-6.1-terra');
 await runtime.chats.prompt(chat.id, 'Run the bash command sleep 30. After it completes reply exactly INTERRUPTED_SHOULD_NOT_COMPLETE.');
 await wait(async () => events.some(e => e.type === 'tool_start' && e.toolName === 'bash' && JSON.stringify(e.args).includes('sleep 30')), 'sleep 30 bash tool start');
 const active = await runtime.chats.open(chat.id);
 assert.equal(active.model?.provider, 'openai'); assert.equal(active.model?.id, 'gpt-6.1-terra');
 assert.ok(active.agentId); assert.ok(active.piSessionId);
 const text = 'Hive team notification: the previous task was interrupted. After this message is explicitly resumed, reply exactly QUEUED_RECOVERY_OK.';
 const presentation = { version: 1 as const, origin: 'system' as const, displayText: 'Team notification: verify queued recovery' };
 const spoolPath = join(directory, 'state/chats/deliveries', `${createHash('sha256').update(chat.id).digest('hex')}.json`);
 const spool = async () => JSON.parse(await readFile(spoolPath, 'utf8')) as DeliveryItem[];
 await runtime.agents.deliverSystemMessageToAgent(workspace.id, active.agentId, text, { requireActiveRun: true, presentation });
 const queued = await spool();
 const item = queued.find(i => i.text === text);
 assert.ok(item); assert.equal(queued.length, 2); assert.equal(item.triggerTurn, true); assert.deepEqual(item.presentation, presentation);
 receipt.queued = { spool: queued, piSessionId: active.piSessionId, chatId: chat.id, workspaceId: workspace.id, agentId: active.agentId };
 phase = 'abort';
 await runtime.chats.abort(chat.id);
 const aborted = await runtime.chats.open(chat.id);
 assert.equal(aborted.busy, false); assert.equal(aborted.pending, 1); assert.deepEqual(await spool(), [item]);
 assert.ok(!hasText(aborted.history, 'QUEUED_RECOVERY_OK'));
 await new Promise(r => setTimeout(r, 1500));
 assert.deepEqual(await spool(), [item]); assert.equal((await runtime.chats.open(chat.id)).busy, false);
 receipt.aborted = { pending: aborted.pending, spool: await spool() };
 await runtime.close(); runtime = undefined;
 phase = 'reopen';
 runtime = await createDesktopRuntime(join(directory, 'state'), emit);
 const reopened = await runtime.chats.open(chat.id);
 assert.equal(reopened.piSessionId, active.piSessionId); assert.equal(reopened.busy, false); assert.equal(reopened.pending, 1);
 assert.deepEqual(await spool(), [item]);
 await new Promise(r => setTimeout(r, 1500));
 assert.equal((await runtime.chats.open(chat.id)).busy, false); assert.deepEqual(await spool(), [item]);
 assert.ok(!events.some(e => e.phase === 'reopen' && (e.type === 'assistant_delta' || e.type === 'tool_start')));
 receipt.reopened = { piSessionId: reopened.piSessionId, pending: reopened.pending, spool: await spool() };
 phase = 'resume';
 await runtime.chats.resume(chat.id);
 await wait(async () => { const s = await runtime!.chats.open(chat.id); return !s.busy && s.pending === 0 && hasText(s.history, 'QUEUED_RECOVERY_OK'); }, 'resumed assistant response');
 const done = await runtime.chats.open(chat.id);
 const finalAssistant = [...done.history].reverse().find(m => (m as { role?: string }).role === 'assistant') as { content?: { type?: string; text?: string }[]; stopReason?: string } | undefined;
 assert.ok(finalAssistant?.content?.some(part => part.type === 'text' && part.text === 'QUEUED_RECOVERY_OK'), 'final assistant text matches exactly');
 const deliveries = done.history.flatMap((message, index) => {
  const p = (message as { presentation?: { deliveryId?: string } }).presentation;
  return p?.deliveryId === item.id ? [{ presentation: p, messageId: done.historyMessageIds[index], message }] : [];
 });
 assert.equal(deliveries.length, 1); assert.equal(deliveries[0]!.messageId, item.id);
 assert.deepEqual(deliveries[0]!.presentation, { ...presentation, deliveryId: item.id });
 assert.deepEqual(await spool(), []);
 receipt.completed = { piSessionId: done.piSessionId, pending: done.pending, deliveries, history: done.history, historyMessageIds: done.historyMessageIds, spool: await spool() };
 receipt.status = 'passed';
 console.log(`QUEUED_DELIVERY_LIVE_OK ${evidence}`);
} catch (error) {
 receipt.status = 'failed'; receipt.error = error instanceof Error ? { message: error.message, stack: error.stack } : String(error);
 throw error;
} finally {
 if (runtime) await runtime.close();
 await writeFile(join(evidence, 'receipt.json'), JSON.stringify(receipt, null, 2));
 await writeFile(join(evidence, 'events.json'), JSON.stringify(events, null, 2));
 console.log(`Evidence: ${evidence}`);
}

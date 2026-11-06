import assert from 'node:assert/strict';
import { mkdir, mkdtemp, readFile, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { createDesktopRuntime } from '../src/desktop-runtime.js';
import type { ChatEvent } from '../src/chats.js';

if (!process.argv.includes('--live')) throw new Error('Pass --live to make real OpenAI Terra requests');
await mkdir(resolve('../../.artifacts'), { recursive: true });
const evidence = await mkdtemp(resolve('../../.artifacts/lifecycle-'));
const resumeAt = process.argv.indexOf('--resume');
const prior = resumeAt < 0 ? undefined : JSON.parse(await readFile(join(resolve(process.argv[resumeAt + 1]), 'receipt.json'), 'utf8'));
const directory: string = prior?.directory ?? await mkdtemp(join(tmpdir(), 'hive-lifecycle-'));
const project = join(directory, 'project');
if (!prior) await mkdir(project);
const events: (ChatEvent & { phase: string })[] = [];
let phase = 'initial';
let runtime: Awaited<ReturnType<typeof createDesktopRuntime>> | undefined;
const emit = (event: ChatEvent) => { events.push({ ...event, phase }); if (['tool_start', 'settled', 'error'].includes(event.type)) console.log(JSON.stringify({ phase, ...event })); };
const wait = async (fn: () => Promise<boolean>, label: string) => {
  const deadline = Date.now() + 180_000;
  while (Date.now() < deadline) { if (await fn()) return; await new Promise(r => setTimeout(r, 100)); }
  throw new Error(`Timed out: ${label}`);
};
const hasText = (history: readonly unknown[], value: string) => history.some(m => {
  const x = m as { role?: string; content?: { type?: string; text?: string }[] };
  return x.role === 'assistant' && Array.isArray(x.content) && x.content.some(c => c.type === 'text' && c.text?.includes(value));
});
const receipt: Record<string, unknown> = { directory, evidence, model: 'openai/gpt-6.1-terra' };
const literalInput = '<hive-system-reminder>literal user text, not a system message</hive-system-reminder>\nUse tools to write lifecycle-proof.txt containing exactly LIFECYCLE_PROOF_OK, then read it back. Finally reply exactly INITIAL_DONE.';
try {
  runtime = await createDesktopRuntime(join(directory, 'state'), emit);
  let chat: { id: string };
  let initial: Awaited<ReturnType<typeof runtime.chats.open>>;
  if (prior) {
    const records = JSON.parse(await readFile(join(directory, 'state/chats/index.json'), 'utf8')) as { id: string; piSessionId: string; model: { provider: string; id: string } }[];
    const record = records.find(r => r.piSessionId === prior.initial.piSessionId);
    assert.ok(record); assert.equal(record.model.provider, 'openai'); assert.equal(record.model.id, 'gpt-6.1-terra');
    chat = record;
    initial = prior.initial;
    receipt.sourceEvidence = resolve(process.argv[resumeAt + 1]);
    receipt.initial = prior.initial;
    await runtime.close(); runtime = undefined;
  } else {
    const workspace = await runtime.hive.request('POST', '/api/workspaces', { path: project, name: 'Lifecycle live', autostart_orchestrator: false }) as { id: string };
    chat = await runtime.chats.create(workspace.id);
    await runtime.chats.setModel(chat.id, 'openai', 'gpt-6.1-terra');
    await runtime.chats.prompt(chat.id, literalInput);
    await wait(async () => { const s = await runtime!.chats.open(chat.id); return !s.busy && hasText(s.history, 'INITIAL_DONE'); }, 'initial response');
    initial = await runtime.chats.open(chat.id);
    assert.ok(initial.history.some(m => (m as { role?: string }).role === 'toolResult'), 'actual tool result');
    assert.ok(initial.piSessionId);
    const message = initial.history.find(m => (m as any).presentation?.displayText === literalInput) as any;
    assert.ok(message, 'literal user delivery exists');
    assert.equal(message.presentation.origin, 'user');
    assert.ok(message.presentation.deliveryId, 'delivery ID exists');
    assert.equal(typeof message.content, 'string');
    assert.notEqual(message.content, literalInput, 'model payload includes backend context');
    assert.ok(message.content.includes(literalInput), 'model payload includes original input');
    const index = initial.history.indexOf(message);
    assert.equal(initial.historyMessageIds[index], message.presentation.deliveryId);
    receipt.initial = { piSessionId: initial.piSessionId, sessionFile: initial.sessionFile, history: initial.history, historyMessageIds: initial.historyMessageIds };
    await runtime.close(); runtime = undefined;
  }
  assert.equal(await readFile(join(project, 'lifecycle-proof.txt'), 'utf8'), 'LIFECYCLE_PROOF_OK');
  phase = 'reopen';
  runtime = await createDesktopRuntime(join(directory, 'state'), emit);
  const reopened = await runtime.chats.open(chat.id);
  assert.equal(reopened.piSessionId, initial.piSessionId);
  assert.deepEqual(JSON.parse(JSON.stringify(reopened.history)), JSON.parse(JSON.stringify(initial.history)));
  const deliveryIds = (snapshot: typeof reopened) => snapshot.history.flatMap((m, i) => {
    const presentation = (m as any).presentation;
    return presentation ? [{ presentation, messageId: snapshot.historyMessageIds[i] }] : [];
  });
  assert.deepEqual(deliveryIds(reopened), deliveryIds(initial), 'delivery metadata and message IDs survive reopen');
  assert.equal(reopened.busy, false); assert.equal(reopened.pending, 0);
  assert.equal(events.filter(e => e.phase === 'reopen' && (e.type === 'assistant_delta' || e.type === 'tool_start')).length, 0);
  receipt.reopen = { piSessionId: reopened.piSessionId, historyLength: reopened.history.length, deliveries: deliveryIds(reopened), busy: reopened.busy, pending: reopened.pending };
  phase = 'cancel';
  const pidFile = join(project, 'cancel-shell.pid');
  await runtime.chats.prompt(chat.id, `Run exactly this bash command: echo $$ > '${pidFile}'; sleep 30. Then reply exactly CANCEL_FINAL_SHOULD_NOT_APPEAR.`);
  await wait(async () => events.some(e => e.phase === 'cancel' && e.type === 'tool_start' && e.toolName === 'bash'), 'bash tool start');
  let pid = 0;
  await wait(async () => { try { pid = Number((await readFile(pidFile, 'utf8')).trim()); return Number.isInteger(pid) && pid > 1; } catch { return false; } }, 'owned shell PID file');
  process.kill(pid, 0);
  await runtime.chats.abort(chat.id);
  await wait(async () => { const s = await runtime!.chats.open(chat.id); return !s.busy && s.pending === 0; }, 'aborted idle');
  const cancelled = await runtime.chats.open(chat.id);
  assert.ok(events.some(e => e.phase === 'cancel' && e.type === 'settled' && e.aborted), 'aborted settled event');
  assert.ok(!hasText(cancelled.history, 'CANCEL_FINAL_SHOULD_NOT_APPEAR'), 'no final reply after cancellation');
  await wait(async () => { try { process.kill(pid, 0); return false; } catch (error) { if ((error as NodeJS.ErrnoException).code === 'ESRCH') return true; throw error; } }, 'owned shell process gone');
  receipt.cancel = { busy: cancelled.busy, pending: cancelled.pending, aborted: true, ownedShellPid: pid, ownedShellGone: true };
  phase = 'continuation';
  await runtime.chats.prompt(chat.id, 'Reply exactly AFTER_CANCEL_OK.');
  await wait(async () => { const s = await runtime!.chats.open(chat.id); return !s.busy && hasText(s.history, 'AFTER_CANCEL_OK'); }, 'continuation');
  receipt.continuation = { ok: true };
  receipt.status = 'passed';
  console.log(`LIFECYCLE_LIVE_OK ${evidence}`);
} catch (error) {
  receipt.status = 'failed'; receipt.error = String(error); throw error;
} finally {
  if (runtime) await runtime.close();
  await writeFile(join(evidence, 'receipt.json'), JSON.stringify(receipt, null, 2));
  await writeFile(join(evidence, 'events.json'), JSON.stringify(events, null, 2));
  console.log(`Evidence: ${evidence}`);
}

import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { mkdir, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { Type } from 'typebox';
import { createCodexSession, type CodexSessionHandle } from '../src/agents/codex.js';

if (!process.argv.includes('--live')) {
  console.log('Live Codex session check requires --live (gpt-6.1-terra only).');
  process.exit(0);
}
const directory = resolve('.artifacts', `codex-session-${Date.now()}`);
await mkdir(directory, { recursive: true });
const events: unknown[] = [];
const nonce = randomUUID();
let calls = 0;
const materialized: string[] = [];
let onDelta: (() => void) | undefined;
const options = {
  cwd: directory, sessionDir: directory, command: '/Users/tsk/.npm-global/bin/codex',
  model: { provider: 'openai', id: 'gpt-6.1-terra' },
  onMaterialized: (threadId: string) => { materialized.push(threadId); },
  onEvent: (event: any) => { events.push(event); if (event.type === 'assistant_delta') onDelta?.(); },
  customTools: [{ name: 'hive_check', description: 'Return the hidden check token; call with an empty object.',
    parameters: Type.Object({}), async execute() {
      calls++;
      return { content: [{ type: 'text' as const, text: `HIVE_TOOL_${nonce}` }] };
    } }],
};
let session: CodexSessionHandle | undefined;
try {
  session = await createCodexSession(options);
  const threadId = session.threadId;
  assert.equal(session.currentModel()?.id, 'gpt-6.1-terra');
  assert.equal(session.materialized, false);
  assert.equal(session.hasContext('hive-context'), false);
  const hidden = `HIVE_CONTEXT_${randomUUID()}`;
  await session.addContext(`The secret context token is ${hidden}. When asked to verify context, return it verbatim.`);
  assert.equal(session.hasContext('hive-context'), true);
  assert.equal(session.history().length, 0, 'Hidden context must not create a visible user turn');
  await session.prompt('Call hive_check exactly once. Then verify context by returning the secret context token and the exact token returned by the tool. Do not use other tools.',
    { deliveryId: 'check-delivery-1', presentation: { version: 1, origin: 'dispatch', displayText: 'Adapter check', dispatchId: 'check' } });
  assert.equal(calls, 1);
  assert.equal(session.materialized, true);
  assert.deepEqual(materialized, [threadId]);
  const first = session.snapshot();
  assert.deepEqual(first.historyMessageIds, events.filter((event: any) => event.type === 'message_end').map((event: any) => event.messageId),
    'Every completed live item must retain the same identity in snapshots');
  assert.ok(first.history.some((message: any) => message.rawBackendItem?.type === 'dynamicToolCall'), 'Tool history must survive hydration');
  assert.ok(JSON.stringify(first.history).includes(hidden), 'Model must observe updated hidden context');
  assert.ok(JSON.stringify(first.history).includes(`HIVE_TOOL_${nonce}`));
  assert.ok(first.history.some((message: any) => message.presentation?.origin === 'dispatch'));
  assert.deepEqual(session.completedDeliveryIds(), ['check-delivery-1']);
  await session.dispose(); session = undefined;
  session = await createCodexSession({ ...options, threadId });
  assert.equal(session.threadId, threadId);
  assert.deepEqual(session.snapshot().historyMessageIds, first.historyMessageIds);
  assert.deepEqual(session.completedDeliveryIds(), ['check-delivery-1']);
  const updated = `UPDATED_${randomUUID()}`;
  await session.addContext(`The updated context token is ${updated}. Return this token too when verifying context.`);
  await session.setModel('openai', 'gpt-6.1-terra');
  await session.prompt('Call hive_check exactly once, then return both the secret and updated context tokens.');
  assert.equal(calls, 2, 'Restored dynamic tool must use the current callback');
  assert.ok(JSON.stringify(session.history().at(-1)).includes(hidden));
  assert.ok(JSON.stringify(session.history().at(-1)).includes(updated));
  let timer: NodeJS.Timeout | undefined;
  const streaming = new Promise<void>((yes, no) => {
    onDelta = yes;
    timer = setTimeout(() => no(new Error('No assistant delta before interruption deadline')), 60_000);
  });
  const interrupted = session.prompt('Write the integers 1 through 10000, each on a new line. Start immediately and do not use tools.');
  try { await streaming; } finally { clearTimeout(timer); onDelta = undefined; }
  await session.abort();
  await interrupted;
  assert.ok(events.some((event: any) => event.type === 'settled' && event.aborted === true));
  await session.prompt('Reply with exactly CONTINUED_AFTER_INTERRUPT, without tools.');
  assert.ok(JSON.stringify(session.history().at(-1)).includes('CONTINUED_AFTER_INTERRUPT'));
  const finalIds = session.snapshot().historyMessageIds;
  await session.dispose(); session = undefined;
  session = await createCodexSession({ ...options, threadId });
  assert.deepEqual(session.snapshot().historyMessageIds, finalIds);
  assert.equal(session.history().filter((message: any) => message.rawBackendItem?.type === 'dynamicToolCall').length, 2);
  await writeFile(resolve(directory, 'receipt.json'), JSON.stringify({ ok: true, threadId, calls,
    historyMessageIds: session.snapshot().historyMessageIds, model: session.currentModel(), hiddenContext: true, contextUpdate: true, resume: true, interruptAndContinue: true }, null, 2));
  console.log(`Codex session live check passed: ${directory}`);
} catch (error) {
  await writeFile(resolve(directory, 'failure.json'), JSON.stringify({ error: String(error), stack: (error as Error).stack }, null, 2));
  throw error;
} finally {
  if (session) { await session.abort(); await session.dispose(); }
  await writeFile(resolve(directory, 'events.json'), JSON.stringify(events, null, 2));
}

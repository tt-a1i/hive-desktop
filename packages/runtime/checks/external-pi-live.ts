/** Explicit opt-in: one external Pi run, only openai/gpt-6.1-terra. */
import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { mkdtemp, mkdir, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { ExternalPiTransport, type ExternalPiRecord } from '../src/agents/external-pi-transport.js';
if (!process.argv.includes('--live')) throw new Error('Pass --live to request openai/gpt-6.1-terra');
const directory = await mkdtemp(join(tmpdir(), 'hive-external-pi-live-'));
const evidence = resolve('../../.artifacts/external-pi/live.json');
const events: ExternalPiRecord[] = [];
const transport = await ExternalPiTransport.start({ cwd: directory, sessionDir: directory,
  command: process.env.HIVE_PI_COMMAND ?? '/Users/tsk/Library/pnpm/pi', extensionPath: resolve('dist/agents/external-pi-extension.js'),
  onEvent: event => { events.push(event); } });
const deliveryId = randomUUID();
const word = `hive-${randomUUID()}`;
const details = { version: 1, origin: 'user', displayText: 'Return the context code', deliveryId };
try {
  const model = await transport.request<{ provider: string; id: string }>('set_model', { provider: 'openai', modelId: 'gpt-6.1-terra' });
  assert.equal(model.provider, 'openai'); assert.equal(model.id, 'gpt-6.1-terra');
  await transport.bridge({ action: 'context', text: `The hidden Hive verification code is ${word}.` });
  let unsubscribe = () => {};
  let timer: ReturnType<typeof setTimeout>;
  const settled = new Promise<void>((resolve, reject) => {
    timer = setTimeout(() => reject(new Error('External Pi live turn timed out')), 120000);
    unsubscribe = transport.onEvent(event => {
      if (event.type === 'agent_settled') {
        if (event.aborted) reject(new Error('External Pi live turn aborted')); else resolve();
      }
      if (event.type === 'extension_error') reject(new Error(String(event.error)));
    });
  });
  // Observe failure immediately while the prompt command response is in flight.
  void settled.catch(() => {});
  try {
    await transport.bridge({ action: 'delivery', text: 'Return only the hidden Hive verification code. Do not use tools.', details });
    await settled;
  } finally { clearTimeout(timer!); unsubscribe(); }
  const messages = await transport.request<{ messages: { role: string; model?: string; provider?: string; content: { type: string; text?: string }[]; details?: unknown }[] }>('get_messages');
  const assistant = messages.messages.findLast(message => message.role === 'assistant');
  assert.ok(assistant); assert.equal(assistant.model, 'gpt-6.1-terra'); assert.equal(assistant.provider, 'openai');
  assert.equal(assistant.content.filter(block => block.type === 'text').map(block => block.text).join('').trim(), word);
  assert.deepEqual(messages.messages.find(message => message.role === 'custom' && message.details)?.details, details);
  const runEntries = await transport.request<{ entries: { id: string; type: string; message?: { role: string } }[] }>('get_entries');
  const assistantEntry = runEntries.entries.findLast(entry => entry.type === 'message' && entry.message?.role === 'assistant');
  assert.ok(assistantEntry);
  await transport.bridge({ action: 'complete', deliveryId, assistantEntryId: assistantEntry.id });
  const entries = await transport.request<{ entries: { customType?: string; data?: { id?: string } }[] }>('get_entries');
  assert.ok(entries.entries.some(entry => entry.customType === 'hive-delivery-completed' && entry.data?.id === deliveryId));
  await mkdir(resolve('../../.artifacts/external-pi'), { recursive: true });
  const receipt = { pass: true, model: 'openai/gpt-6.1-terra', directory, details, messages: messages.messages,
    eventTypes: [...new Set(events.map(event => event.type))], state: await transport.request('get_state') };
  await writeFile(evidence, JSON.stringify(receipt, null, 2) + '\n');
  console.log(JSON.stringify({ pass: true, evidence, directory }));
} finally { await transport.close(); }

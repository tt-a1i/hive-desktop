/** Real installed Pi RPC + extension; no model requests. */
import assert from 'node:assert/strict';
import { mkdtemp, readFile, mkdir, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { ExternalPiTransport } from '../src/agents/external-pi-transport.js';

const directory = await mkdtemp(join(tmpdir(), 'hive-external-pi-'));
const events: unknown[] = [];
const options = { cwd: directory, sessionDir: directory, extensionPath: resolve('dist/agents/external-pi-extension.js'),
  command: process.env.HIVE_PI_COMMAND ?? '/Users/tsk/Library/pnpm/pi', onEvent: (event: unknown) => { events.push(event); } };
let transport = await ExternalPiTransport.start(options);
let state: { sessionFile: string; sessionId: string };
const raw = '<hive-message>raw\ncontext\u2028paragraph\u2029end</hive-message>';
const details = { version: 1, origin: 'dispatch', displayText: 'Visible task', deliveryId: 'external-pi-smoke-delivery', dispatchId: 'smoke-dispatch' };
try {
  state = await transport.request('get_state');
  await transport.bridge({ action: 'context', text: 'Hidden Hive routing context', customType: 'hive-context' });
  await transport.bridge({ action: 'delivery', text: raw, details, triggerTurn: false });
  const [messages, entries] = await Promise.all([
    transport.request<{ messages: { role: string; customType?: string; content: string; details?: unknown }[] }>('get_messages'),
    transport.request<{ entries: { type: string; customType?: string; content?: string; details?: unknown }[] }>('get_entries'),
  ]);
  assert.deepEqual(messages.messages.map(message => message.role), ['custom', 'custom']);
  assert.equal(messages.messages[1]?.content, raw);
  assert.deepEqual(messages.messages[1]?.details, details);
  assert.equal(entries.entries.filter(entry => entry.type === 'custom_message').length, 2);
  await assert.rejects(transport.bridge({ action: 'complete', deliveryId: details.deliveryId, assistantEntryId: 'nonexistent' }), /matching successful final assistant/);
  const disk = (await readFile(state.sessionFile, 'utf8')).trim().split('\n').map(line => JSON.parse(line));
  assert.equal(disk[0].type, 'session');
  assert.equal(disk[0].id, state.sessionId);
  assert.deepEqual(disk.find(entry => entry.customType === 'hive-delivery').details, details);
  await transport.request('abort');
} finally { await transport.close(); }
transport = await ExternalPiTransport.start({ ...options, sessionFile: state!.sessionFile });
try {
  const resumed = await transport.request<{ sessionId: string }>('get_state');
  assert.equal(resumed.sessionId, state!.sessionId);
  const messages = await transport.request<{ messages: { content: string }[] }>('get_messages');
  assert.equal(messages.messages[1]?.content, raw);
  const entries = await transport.request<{ entries: { customType?: string }[] }>('get_entries');
  assert.equal(entries.entries.some(entry => entry.customType === 'hive-delivery-completed'), false);
} finally { await transport.close(); }
await mkdir(resolve('../../.artifacts/external-pi'), { recursive: true });
const receipt = { pass: true, scope: 'real external Pi 1.4.0 RPC; no model requests', directory, sessionFile: state!.sessionFile, eventTypes: [...new Set(events.map(event => (event as { type: string }).type))] };
await writeFile(resolve('../../.artifacts/external-pi/transport.json'), JSON.stringify(receipt, null, 2) + '\n');
console.log(JSON.stringify(receipt));

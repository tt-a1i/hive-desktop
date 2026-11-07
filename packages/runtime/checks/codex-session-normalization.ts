import assert from 'node:assert/strict';
import { CodexEvents } from '../src/agents/codex-events.js';
import { normalizeCodexItem, type CodexDelivery } from '../src/agents/codex-history.js';
import type { PiEvent } from '../src/agents/types.js';

const raw = { type: 'userMessage', id: 'native-input', content: [{ type: 'text', text: '<hive-dispatch>Full raw task envelope</hive-dispatch>', text_elements: [] }] };
const original = structuredClone(raw);
for (const origin of ['user', 'dispatch', 'report', 'recovery', 'cancel', 'system', 'unknown'] as const) {
  const delivery: CodexDelivery = { turnId: 'turn', deliveryId: `delivery-${origin}`,
    presentation: { version: 1, origin, displayText: 'Concise visible text', dispatchId: 'dispatch', workerName: 'worker' } };
  const message = normalizeCodexItem(raw, delivery);
  assert.equal(message.role, 'custom');
  assert.equal(message.customType, 'hive-delivery');
  assert.deepEqual(message.presentation, { ...delivery.presentation, deliveryId: delivery.deliveryId });
  assert.deepEqual(message.details, message.presentation);
  assert.deepEqual(message.content, [{ type: 'text', text: raw.content[0]!.text }]);
  assert.deepEqual(message.rawBackendItem, original);
  const events: PiEvent[] = [];
  const projector = new CodexEvents('thread', event => events.push(event), turnId => turnId === 'turn' ? delivery : undefined);
  projector.item(raw, 'turn', true);
  const ended = events.find(event => event.type === 'message_end');
  assert.ok(ended?.type === 'message_end');
  assert.deepEqual(ended.message, message);
  assert.deepEqual(projector.snapshot().history[0], message, 'Live and restored history must use the same native provenance contract');
}
const legacy = normalizeCodexItem(raw);
assert.equal(legacy.role, 'user', 'User-pasted envelopes without metadata remain ordinary input');
assert.equal(legacy.customType, undefined);
assert.equal(legacy.presentation, undefined);
assert.deepEqual(raw, original, 'Presentation must not modify the backend payload');
console.log('Codex normalization passed: native hive-delivery contract for all origins, live/snapshot parity, raw preservation, ordinary legacy input. No model calls.');


const events: PiEvent[] = [];
const tools = new CodexEvents('thread', event => events.push(event), () => undefined);
const command = { type: 'commandExecution', id: 'shell', command: 'git status', cwd: '/project' };
tools.item({ ...command, status: 'inProgress' }, 'turn', false);
tools.item({ ...command, status: 'completed', aggregatedOutput: 'clean' }, 'turn', true);
const start = events.find(event => event.type === 'tool_start');
assert.deepEqual(start?.args, { command: 'git status', cwd: '/project' });
const finished = tools.snapshot().history[0] as ReturnType<typeof normalizeCodexItem>;
assert.deepEqual(finished.toolInput, start?.args);
assert.deepEqual(finished.content, [{ type: 'text', text: 'clean' }]);
assert.equal(normalizeCodexItem({ ...command, status: 'declined' }).toolStatus, 'declined');
const result = normalizeCodexItem({ type: 'dynamicToolCall', tool: 'team', id: 'send',
  arguments: { args: ['list'] }, contentItems: [{ type: 'inputText', text: 'worker ready' }] });
assert.deepEqual(result.toolInput, { args: ['list'] });
assert.deepEqual(result.content, [{ type: 'text', text: 'worker ready' }]);

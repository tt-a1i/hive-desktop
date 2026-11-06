import assert from 'node:assert/strict';
import { fileURLToPath } from 'node:url';
import { CodexTransport } from '../src/agents/codex-transport.js';

// A real subprocess with a synthetic JSONL peer, not evidence about Codex behavior.
const command = fileURLToPath(new URL('./fixtures/codex-jsonl.mjs', import.meta.url));
const base = { command, cwd: process.cwd(), requestTimeoutMs: 1000 };
for (const field of ['requestTimeoutMs', 'shutdownGraceMs'] as const) {
  await assert.rejects(CodexTransport.start({ ...base, [field]: 2_147_483_648 }), RangeError);
}
const notifications: unknown[] = [];
const transport = await CodexTransport.start({
  ...base,
  onNotification: (notification) => { notifications.push(notification); },
  onRequest: async (request) => ({ echoed: request.params }),
});
try {
  const resolutionOrder: number[] = [];
  const first = transport.sendRequest('fixture/overlap', { sequence: 1 }).then((result) => {
    resolutionOrder.push(1);
    return result;
  });
  const second = transport.sendRequest('fixture/overlap', { sequence: 2 }).then((result) => {
    resolutionOrder.push(2);
    return result;
  });
  assert.deepEqual(await Promise.all([first, second]), [{ sequence: 1 }, { sequence: 2 }]);
  assert.deepEqual(resolutionOrder, [2, 1]);
  assert.deepEqual(await transport.sendRequest('fixture/serverRequests', {}), [
    { id: 41, result: { echoed: { kind: 'numeric' } } },
    { id: '41', result: { echoed: { kind: 'string' } } },
  ]);
  assert.deepEqual(notifications, [{ method: 'fixture/ready', params: { ready: true } }]);
  await assert.rejects(transport.sendRequest('fixture/hold', {}), /timed out/);
  // The fixture writes the expired response before this one; correlation must survive it.
  assert.equal(await transport.sendRequest('fixture/release', {}), 'released');
} finally { await transport.close(); }

const unsupported = await CodexTransport.start(base);
try {
  const replies = await unsupported.sendRequest<{ id: string | number; error: { code: number } }[]>('fixture/serverRequests', {});
  assert.deepEqual(replies.map(({ id, error }) => ({ id, code: error.code })), [
    { id: 41, code: -32601 }, { id: '41', code: -32601 },
  ]);
} finally { await unsupported.close(); }
console.log('Synthetic JSONL subprocess: timer bounds, reversed responses, inbound numeric/string IDs, async callback, unsupported requests, timeout + late response passed.');

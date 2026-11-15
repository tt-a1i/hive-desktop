import assert from 'node:assert/strict';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import type { ChatRecord } from '../src/chat-index.js';
import { handoffNativeCompletedIds } from '../src/handoff-native-receipts.js';

const root = mkdtempSync(join(tmpdir(), 'hive-handoff-receipts-'));
try {
 const file = join(root, 'target.jsonl');
 const context = '[historical context exact frozen hash]';
 const delivery = { deliveryId: 'new-target-delivery', nativeSessionId: 'target-native' };
 const handle = { sessionId: 'target-native', sessionFile: file, completedDeliveryIds: () => ['ordinary', 'new-target-delivery'] };
 const base: ChatRecord = { id: 'target', workspaceId: 'workspace', title: 'Target', backend: 'pi', sessionFile: file, piSessionId: 'target-native', updatedAt: 1 };
 const state = { type: 'custom', customType: 'hive-user-input', data: { version: 1, origin: 'user', displayText: 'Continue', rawText: `Continue\n${context}`, deliveryId: delivery.deliveryId } };
 const ctx = { type: 'custom_message', customType: 'hive-user-input-context', content: `\n${context}`, display: false, details: { deliveryId: delivery.deliveryId } };
 const user = { type: 'message', message: { role: 'user', content: [{ type: 'text', text: 'Continue' }], timestamp: 1 } };
 const result = (disposition: string) => ({ type: 'custom', customType: 'hive-user-input-result', data: { deliveryId: delivery.deliveryId, disposition } });
 const save = (rows: unknown[], nativeId = 'target-native') => writeFileSync(file, [{ type: 'session', version: 3, id: nativeId, timestamp: '2026-11-15T00:00:00Z', cwd: root }, ...rows.map((row, index) => ({ id: `item-${index}`, parentId: index ? `item-${index - 1}` : null, timestamp: '2026-11-15T00:00:00Z', ...(row as object) }))].map(row => JSON.stringify(row)).join('\n') + '\n');
 const ids = (record = base, selected = delivery, expected = context) => handoffNativeCompletedIds(record, handle, selected, expected);
 const rejected = () => assert.deepEqual(ids(), ['ordinary']);
 for (const backend of ['pi', 'pi-rpc'] as const) {
  save([state, ctx, user, result('started')]);
  const record = { ...base, backend, externalPiSessionId: 'target-native' };
  assert.deepEqual(ids(record), ['ordinary', delivery.deliveryId]);
  assert.deepEqual(handoffNativeCompletedIds(record, { ...handle }, { ...delivery }, context), ['ordinary', delivery.deliveryId], 'cold file proof needs no live model or memory');
  for (const disposition of ['handled', 'queued', 'failed']) {
   save([state, ctx, user, result(disposition)]);
   assert.deepEqual(ids(record), ['ordinary']);
  }
 }
 save([state, user, result('started')]); rejected();
 save([state, { ...ctx, content: 'different-context' }, user, result('started')]); rejected();
 save([state, { ...ctx, details: { deliveryId: 'source-delivery' } }, user, result('started')]); rejected();
 save([state, ctx, result('started')]); rejected();
 save([state, ctx, user, result('started')], 'source-native'); rejected();
 save([state, ctx, user, result('started')]);
 assert.deepEqual(ids(base, { ...delivery, nativeSessionId: 'source-native' }), ['ordinary']);
 assert.deepEqual(ids(base, { ...delivery, nativeSessionId: '' }), ['ordinary']);
 assert.deepEqual(ids({ ...base, piSessionId: 'source-native' }), ['ordinary']);
 assert.deepEqual(ids(base, delivery, ''), ['ordinary']);
 assert.deepEqual(ids(base, delivery, 'wrong expected hash'), ['ordinary']);
 // Persisted final leaf selects a different branch; the abandoned proof cannot consume.
 save([state, ctx, user, result('started'), { type: 'custom', customType: 'other-branch', parentId: null }]); rejected();
 save([state, ctx, user, result('started'), result('handled')]); rejected();
 writeFileSync(file, '{malformed'); rejected();
 writeFileSync(file, ' '.repeat(16 * 1024 * 1024 + 1)); rejected();
 rmSync(file); rejected();
 for (const backend of ['codex', 'claude', 'acp'] as const) {
  assert.deepEqual(ids({ ...base, backend }), ['ordinary', delivery.deliveryId]);
  assert.deepEqual(ids({ ...base, backend }, { ...delivery, nativeSessionId: 'source-native' }), ['ordinary']);
 }
 console.log('handoff native receipts: Pi started/user/context/current-branch proof, cold recovery, identity and malformed evidence checks passed');
} finally { rmSync(root, { recursive: true, force: true }); }

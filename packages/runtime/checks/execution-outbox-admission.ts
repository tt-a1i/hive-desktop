import assert from 'node:assert/strict';
import {createDispatchMessageOutbox} from '@tt-a1i/hive/dist/src/server/dispatch-message-outbox.js';
import {ExecutionMutations} from '../src/execution-mutations.js';
const gate = new ExecutionMutations();
let state = 'queued', writes = 0, claims = 0;
const message = {id:'m', workspaceId:'w', recipientAgentId:'worker', dispatchId:'d', kind:'update', fromAgentId:'sender', sequence:1, text:'hello'};
const runtime = {
  executionAdmissionBlocked: () => gate.isLeased('w'),
  beginExecutionAdmission: () => gate.begin('w'),
  getActiveRunByAgentId: () => ({}),
  deliverSystemMessageToAgent: async () => { writes++; },
};
const messages = {
  controllerBinding: () => ({external:false}),
  listQueued: () => state === 'queued' ? [message] : [],
  claim: () => { claims++; state = 'delivering'; return true; },
  getMessage: () => ({...message, deliveryState:state}),
  isEligible: () => true,
  delivered: () => {state = 'delivered';},
  failed: () => {state = 'failed';},
  cancel: () => {state = 'cancelled';},
};
const drain = createDispatchMessageOutbox({runtime, messages, isClosing: () => false} as never);
const release = gate.acquire('w');
drain('w', 'worker');
await new Promise(resolve => setImmediate(resolve));
assert.equal(claims, 0); assert.equal(state, 'queued'); assert.equal(writes, 0);
release(); drain('w', 'worker');
await new Promise(resolve => setImmediate(resolve));
assert.equal(claims, 1); assert.equal(state, 'delivered'); assert.equal(writes, 1);
console.log('EXECUTION_OUTBOX_ADMISSION_OK: queue unclaimed during lease and delivered after release');

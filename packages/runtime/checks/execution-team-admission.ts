import assert from 'node:assert/strict';
import {createTeamOperations} from '@tt-a1i/hive/dist/src/server/team-operations.js';
import {ExecutionMutations} from '../src/execution-mutations.js';
const gate = new ExecutionMutations();
let claims = 0, delivered = 0, writes = 0;
const entry = {id:'report', payload:'report text', createdAt:Date.now()};
const agentRuntime = {
  executionAdmissionBlocked: () => gate.isLeased('w'),
  getActiveRunByAgentId: () => ({}),
  deliverSystemMessageToAgent: async () => {writes++;},
};
const operations = createTeamOperations({agentRuntime,
  reportOutbox: {listPending: () => [entry], markDelivered: () => {delivered++;}},
  listOpenWorkspaceDispatches: () => {claims++; return [];},
  isRuntimeClosing: () => false,
} as never);
const release = gate.acquire('w');
operations.drainReportOutbox('w', 'worker');
operations.replayQueuedDispatches('w', 'worker');
await new Promise(resolve => setImmediate(resolve));
assert.equal(writes, 0); assert.equal(delivered, 0); assert.equal(claims, 0);
release();
operations.drainReportOutbox('w', 'worker');
operations.replayQueuedDispatches('w', 'worker');
await new Promise(resolve => setImmediate(resolve));
assert.equal(writes, 1); assert.equal(delivered, 1); assert.equal(claims, 1);
console.log('EXECUTION_TEAM_ADMISSION_OK: report and dispatch drains pause and resume');

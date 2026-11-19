/** Cross-platform controls for the Windows observer preflight; no official CLI, account or model. */
import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { prepareProcessObserver, type ProcessObserverAttempt } from './windows-process-observer.js';

const ownPid = process.pid;
const ready = [{ ProcessId: ownPid }];
let calls = 0;
const first: ProcessObserverAttempt[] = [];
await prepareProcessObserver(async () => { calls++; return ready; }, ownPid, attempt => first.push(attempt));
assert.equal(calls, 1); assert.deepEqual(first.map(x => x.status), ['ready']);

// Capture a real Node execFile timeout rather than inventing its error shape.
const timeout = await new Promise<Error & { code?: unknown; killed?: unknown; signal?: unknown }>(resolve => {
  execFile(process.execPath, ['-e', 'setInterval(()=>{},1000)'], { timeout: 30 }, error => {
    assert.ok(error); resolve(error);
  });
});
assert.equal(timeout.code, null); assert.equal(timeout.killed, true); assert.equal(timeout.signal, 'SIGTERM');
calls = 0;
const recovered: ProcessObserverAttempt[] = [];
await prepareProcessObserver(async () => { if (++calls === 1) throw timeout; return ready; }, ownPid, attempt => recovered.push(attempt));
assert.equal(calls, 2); assert.deepEqual(recovered.map(x => x.status), ['timeout', 'ready']);
assert.deepEqual(recovered[0].failure, { code: null, killed: true, signal: 'SIGTERM' });

calls = 0;
const exhausted: ProcessObserverAttempt[] = [];
await assert.rejects(prepareProcessObserver(async () => { calls++; throw timeout; }, ownPid, attempt => exhausted.push(attempt)), error => error === timeout);
assert.equal(calls, 2); assert.deepEqual(exhausted.map(x => x.status), ['timeout', 'timeout']);

for (const failure of [
  Object.assign(new Error('CIM query rejected'), { code: 1, killed: false, signal: null }),
  Object.assign(new Error('PowerShell missing'), { code: 'ENOENT', killed: false, signal: null }),
  Object.assign(new Error('Output exceeded maxBuffer'), { code: 'ERR_CHILD_PROCESS_STDIO_MAXBUFFER', killed: true, signal: 'SIGTERM' }),
  new SyntaxError('Malformed process JSON'),
]) {
  calls = 0;
  const observations: ProcessObserverAttempt[] = [];
  await assert.rejects(prepareProcessObserver(async () => { calls++; throw failure; }, ownPid, attempt => observations.push(attempt)), error => error === failure);
  assert.equal(calls, 1); assert.deepEqual(observations.map(x => x.status), ['error']);
}
calls = 0;
await assert.rejects(prepareProcessObserver(async () => { calls++; return []; }, ownPid, () => {}), /must see the check process/);
assert.equal(calls, 1, 'Missing observer identity must fail rather than appear ready');
console.log('Windows process observer preflight: ready, one real execFile timeout then recovery, persistent timeout, CIM/spawn/JSON/maxBuffer failures and missing identity checked. No CLI/model calls.');

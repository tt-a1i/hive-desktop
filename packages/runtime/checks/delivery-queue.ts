import assert from "node:assert/strict";
import { mkdtempSync, rmSync, mkdirSync, renameSync } from "node:fs";
import { createHash } from "node:crypto";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { DeliveryQueue, type DeliveryItem } from "../src/delivery-queue.js";
import type { DeliveryPresentation } from "../src/delivery-presentation.js";

const directory = mkdtempSync(join(tmpdir(), "hive-delivery-"));
const deferred = () => {
 let resolve!: () => void;
 const promise = new Promise<void>(done => { resolve = done; });
 return { promise, resolve };
};
const unexpected = (error: Error) => { throw error; };
try {
 const started = deferred(), blocked = deferred();
 const executed: string[] = [];
 const first = new DeliveryQueue({ directory, sessionId: "fifo", completedIds: () => [], onError: unexpected,
  execute: async item => { executed.push(item.text); if (item.text === "one") { started.resolve(); await blocked.promise; } },
 });
 await first.enqueue("one");
 await started.promise;
 const presentation: DeliveryPresentation = { version: 1, origin: "user", displayText: "<hive-system-reminder>literal user text</hive-system-reminder>" };
 await first.enqueue("two", false, presentation); // Acceptance cannot wait for the blocked model.
 const originalDisplay = presentation.displayText;
 presentation.displayText = "caller mutation";
 const exposed = first.pending[1]!.presentation!;
 exposed.displayText = "snapshot mutation";
 assert.equal(first.pending[1]?.presentation?.displayText, originalDisplay);
 assert.equal(first.busy, true);
 assert.deepEqual(first.pending.map(item => item.text), ["one", "two"]);
 const stopped = first.pause();
 blocked.resolve();
 await stopped;
 assert.deepEqual(executed, ["one"]);
 assert.equal(first.pending[0]?.triggerTurn, false);
 const reopened: DeliveryItem[] = [];
 const second = new DeliveryQueue({ directory, sessionId: "fifo", completedIds: () => [], onError: unexpected,
  execute: async item => { reopened.push(item); },
 });
 second.resume();
 await second.idle();
 assert.deepEqual(reopened.map(item => item.text), ["two"]);
 assert.equal(reopened[0]?.presentation?.origin, "user");
 assert.equal(reopened[0]?.presentation?.displayText, originalDisplay);
 assert.deepEqual(second.pending, []);

 const failure = new Error("Execution failed");
 const errors: Error[] = [];
 let attempts = 0;
 const failed = new DeliveryQueue({ directory, sessionId: "failed", completedIds: () => [], onError: error => errors.push(error),
  execute: async () => { attempts++; throw failure; },
 });
 await failed.enqueue("retry me");
 await failed.idle();
 await failed.enqueue("later");
 assert.equal(attempts, 1);
 assert.deepEqual(errors, [failure]);
 assert.deepEqual(failed.pending.map(item => item.text), ["retry me", "later"]);
 await failed.pause();
 failed.discardCurrent();
 assert.deepEqual(failed.pending.map(item => item.text), ["later"]);

 const completedId = failed.pending[0]!.id;
 let executedCompleted = false;
 const completed = new DeliveryQueue({ directory, sessionId: "failed", completedIds: () => [completedId], onError: unexpected,
  execute: async () => { executedCompleted = true; },
 });
 completed.resume();
 await completed.idle();
 assert.equal(executedCompleted, false);
 assert.deepEqual(completed.pending, []);

 // The native turn can finish just before Stop, with the rejected steer
 // still settling. The abort then has no active turn to interrupt.
 const primaryStarted = deferred(), finishPrimary = deferred(), primaryRemoved = deferred(), finishSteer = deferred();
 const stopExecutions: string[] = [];
 const stopOptions = { directory, sessionId: "stop-during-steer", completedIds: () => [], onError: unexpected,
  execute: async (item: DeliveryItem) => {
   stopExecutions.push(item.text);
   if (item.text === "primary") { primaryStarted.resolve(); await finishPrimary.promise; }
  },
 };
 const stopping = new DeliveryQueue({ ...stopOptions,
  steer: async () => { await finishSteer.promise; return false; },
  onState: busy => { if (busy && stopping.pendingCount === 1 && stopExecutions.length === 1) primaryRemoved.resolve(); },
 });
 await stopping.enqueue("primary");
 await primaryStarted.promise;
 await stopping.enqueue("append or queue", true, {version: 1, origin: "user", displayText: "append or queue"}, undefined, {steerUser: true});
 const queuedId = stopping.pending[1]!.id;
 finishPrimary.resolve();
 await primaryRemoved.promise;
 const pausing = stopping.pause();
 finishSteer.resolve();
 await pausing;
 stopping.discardCurrent();
 assert.deepEqual(stopExecutions, ["primary"], "Stop cannot start a fallback turn after the native steer declines");
 const coldPaused = new DeliveryQueue({ ...stopOptions, startPaused: true });
 assert.deepEqual(coldPaused.pendingUserInputs.map(item => [item.deliveryId, item.text, item.editable]), [[queuedId, "append or queue", true]]);
 coldPaused.resume();
 await coldPaused.idle();
 assert.deepEqual(stopExecutions, ["primary", "append or queue"]);
 assert.equal(coldPaused.pendingCount, 0);

 // A definite negative receipt must refresh the paused UI even when neither
 // busy nor the pending count changes. Unknown new attempts stay protected.
 for (const outcome of ['declined', 'declined-cancel', 'pending', 'ack-without-completion', 'completed'] as const) {
  const entered = deferred(), finish = deferred(), decline = deferred();
  const sent: string[] = [], complete: string[] = [], faults: Error[] = [];
  const capabilities: {editable:boolean}[][] = [];
  const options = {directory, sessionId: `receipt-${outcome}`, completedIds: () => complete,
   onError: (error: Error) => faults.push(error), execute: async (item: DeliveryItem) => {
    sent.push(item.text); if (item.text === 'primary') { entered.resolve(); await finish.promise; }
   }};
  const q = new DeliveryQueue({...options, steer: async item => {
   if (outcome === 'pending') return new Promise<boolean>(() => {});
   if (outcome === 'declined' || outcome === 'declined-cancel') { await decline.promise; return false; }
   if (outcome === 'completed') complete.push(item.id);
   return true;
  }, onState: () => capabilities.push(q.pendingUserInputs)});
  await q.enqueue('primary'); await entered.promise;
  await q.enqueue('original', true, {version:1, origin:'user', displayText:'original'}, undefined, {steerUser:true});
  const target = q.pending[1]!;
  const pause = q.pause(); finish.resolve(); await pause; q.discardCurrent();
  assert.equal(q.pendingUserInputs[0]!.editable, false);
  const before = capabilities.length;
  if (outcome === 'declined' || outcome === 'declined-cancel') {
   decline.resolve(); await new Promise(resolve => setImmediate(resolve));
   assert.equal(q.pendingUserInputs[0]!.editable, true);
   assert.ok(capabilities.length > before, 'A capability-only change needs an event');
   assert.equal(capabilities.at(-1)![0]!.editable, true);
   if(outcome==='declined-cancel')q.cancelPendingUserInput(target.id);
   else q.editPendingUserInput(target.id, {...target, text:'edited', presentation:{...target.presentation!,displayText:'edited'}});
  }
  const cold = new DeliveryQueue({...options, startPaused:true});
  await new Promise(resolve=>setImmediate(resolve));
  assert.equal(q.hasLiveSteering,outcome==='pending','Only an actually unsettled native request blocks local review');
  if(outcome==='declined-cancel'){
   assert.equal(cold.pendingCount,0);cold.resume();await cold.idle();assert.deepEqual(sent,['primary']);continue;
  }
  if(outcome==='completed')assert.equal(cold.pendingCount,0,'A trusted completion resolves a new attempted append before resume');
  else assert.equal(cold.pendingUserInputs[0]!.editable, outcome === 'declined');
  cold.resume(); await cold.idle();
  if (outcome === 'declined') assert.deepEqual(sent, ['primary','edited']);
  else {
   assert.deepEqual(sent, ['primary'], 'An uncertain/consumed original cannot be resent');
   assert.equal(cold.pendingCount, outcome === 'completed' ? 0 : 1);
   if (outcome !== 'completed') assert.match(faults.at(-1)!.message, /outcome is uncertain/);
  }
 }

 // A negative native response cannot grant editing if its durable update fails.
 const failureStart=deferred(),failureFinish=deferred(),failureDecline=deferred();
 const failureSession='decline-disk-failure';
 const retained=new DeliveryQueue({directory,sessionId:failureSession,completedIds:()=>[],onError:unexpected,
  execute:async()=>{failureStart.resolve();await failureFinish.promise;},steer:async()=>{await failureDecline.promise;return false;}});
 await retained.enqueue('primary');await failureStart.promise;
 await retained.enqueue('original',true,{version:1,origin:'user',displayText:'original'},undefined,{steerUser:true});
 const retainingPause=retained.pause();failureFinish.resolve();await retainingPause;retained.discardCurrent();
 const retainedPath=join(directory,createHash('sha256').update(failureSession).digest('hex')+'.json');
 renameSync(retainedPath,retainedPath+'.held');mkdirSync(retainedPath);
 failureDecline.resolve();await new Promise(resolve=>setImmediate(resolve));
 assert.equal(retained.pendingUserInputs[0]!.editable,false);
 rmSync(retainedPath,{recursive:true});renameSync(retainedPath+'.held',retainedPath);
 const retainedCold=new DeliveryQueue({directory,sessionId:failureSession,startPaused:true,completedIds:()=>[],onError:unexpected,execute:async()=>{throw Error('No replay');}});
 assert.equal(retainedCold.pendingUserInputs[0]!.editable,false);

 // Legacy pending/steerUser is not proof of a native attempt: an earlier
 // ordinary input can prevent steering. Do not migrate or freeze that input.
 const legacyStart = deferred(), legacyFinish = deferred(); let legacySteers = 0;
 const legacySent: string[] = [];
 const legacyOptions = {directory,sessionId:'legacy-unattempted',completedIds:()=>[],onError:unexpected,
  execute:async(item:DeliveryItem)=>{legacySent.push(item.text);if(item.text==='primary'){legacyStart.resolve();await legacyFinish.promise;}},
  steer:async()=>{legacySteers++;return false;}};
 const legacy = new DeliveryQueue(legacyOptions);
 await legacy.enqueue('primary');await legacyStart.promise;
 await legacy.enqueue('earlier ordinary',true,{version:1,origin:'user',displayText:'earlier ordinary'});
 await legacy.enqueue('never steered',true,{version:1,origin:'user',displayText:'never steered'},undefined,{steerUser:true});
 const legacyPause=legacy.pause();legacyFinish.resolve();await legacyPause;legacy.discardCurrent();
 assert.equal(legacySteers,0);
 const legacyCold=new DeliveryQueue({...legacyOptions,startPaused:true});
 assert.ok(legacyCold.pendingUserInputs.every(item=>item.editable));
 legacyCold.resume();await legacyCold.idle();assert.deepEqual(legacySent,['primary','earlier ordinary','never steered']);

 const states: { busy: boolean; pending: number }[] = [];
 const observed = new DeliveryQueue({ directory, sessionId: "observed", startPaused: true,
  completedIds: () => [], onError: () => {},
  execute: async () => { throw failure; },
  onState: busy => states.push({ busy, pending: observed.pending.length }),
 });
 assert.deepEqual(states, []); // Constructor must not access the unassigned variable.
 await observed.enqueue("paused acceptance");
 assert.deepEqual(states.at(-1), { busy: false, pending: 1 });
 observed.resume();
 await observed.idle();
 assert.deepEqual(states.at(-1), { busy: false, pending: 1 });
 await observed.pause();
 observed.discardCurrent();
 assert.deepEqual(states.at(-1), { busy: false, pending: 0 });
 const beforeEmptyResume = states.length;
 observed.resume();
 await observed.idle();
 assert.equal(states.length, beforeEmptyResume);
 assert.deepEqual(states.at(-1), { busy: false, pending: 0 });
 const emptyStates: boolean[] = [];
 const empty = new DeliveryQueue({ directory, sessionId: "empty", startPaused: true,
  completedIds: () => [], onError: unexpected, execute: async () => {}, onState: busy => emptyStates.push(busy),
 });
 empty.resume();
 assert.deepEqual(emptyStates, [false]);
 console.log("Delivery queue passed: durable acceptance, FIFO, reload, failure retention, cancellation, completion deduplication.");
} finally { rmSync(directory, { recursive: true, force: true }); }

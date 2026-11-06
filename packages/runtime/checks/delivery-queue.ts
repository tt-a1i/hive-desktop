import assert from "node:assert/strict";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { DeliveryQueue, type DeliveryItem } from "../src/delivery-queue.js";

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
 await first.enqueue("two", false); // Acceptance cannot wait for the blocked model.
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

import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import fs, { mkdtempSync, readdirSync, readFileSync, realpathSync, rmSync, statSync, symlinkSync, writeFileSync } from "node:fs";
import { syncBuiltinESMExports } from "node:module";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { assertHandoffFirstInput, HandoffDeliveries } from "../src/handoff-delivery.js";

const directory = mkdtempSync(join(tmpdir(), "hive-handoff-delivery-"));
const snapshotHash = "a".repeat(64);
const pathFor = (id: string) => join(directory, "session-handoffs", "deliveries", `${createHash("sha256").update(id).digest("hex")}.json`);
try {
 const journal = new HandoffDeliveries(directory);
 assert.equal(journal.get("target"), undefined);
 assert.equal(journal.initialize("target", "operation", snapshotHash).status, "pending");
 assert.throws(() => journal.initialize("target", "different-operation", snapshotHash), /context changed/);
 assert.throws(() => journal.reserve("target", "operation", "b".repeat(64), "new-input"), /context changed/);
 assert.equal(journal.reserve("target", "operation", snapshotHash, "new-input").status, "reserved");
 assert.equal(journal.reserve("target", "operation", snapshotHash, "new-input").status, "reserved");
 assert.throws(() => journal.reserve("target", "operation", snapshotHash, "second-input"), /cannot be reserved/);
 assert.equal(journal.reconcileQueue("target", ["new-input"])?.status, "reserved");
 assert.equal(journal.reconcileQueue("target", [])?.status, "pending");
 journal.reserve("target", "operation", snapshotHash, "replacement");
 assert.throws(() => journal.attempted("target", "wrong", "target-native"), /identity mismatch/);
 assert.equal(journal.attempted("target", "replacement", "target-native").status, "attempted");
 assert.throws(() => journal.attempted("target", "replacement", "target-native"), /must not be retried/);
 assert.throws(() => journal.releaseUnattempted("target", "replacement"), /cannot be released/);
 assert.equal(journal.reconcileQueue("target", [])?.status, "attempted");
 assert.throws(() => journal.consumed("target", "replacement", "source-native", ["replacement"]), /same target/);
 assert.throws(() => journal.consumed("target", "replacement", "target-native", ["source-completed"]), /same target/);
 const cold = new HandoffDeliveries(directory);
 assert.equal(cold.reconcile("target", "source-native", ["replacement"])?.status, "uncertain");
 assert.equal(cold.reconcileQueue("target", [])?.status, "uncertain");
 assert.throws(() => cold.reserve("target", "operation", snapshotHash, "again"), /uncertain/);
 assert.throws(() => cold.attempted("target", "replacement", "new-native"), /must not be retried/);
 assert.equal(cold.reconcile("target", "target-native", ["replacement"])?.status, "consumed");
 assert.equal(cold.consumed("target", "replacement", "target-native", ["replacement"]).status, "consumed");
 assert.equal(cold.reconcile("target", "other-native", [])?.status, "consumed");
 assert.throws(() => cold.releaseUnattempted("target", "replacement"), /cannot be released/);

 // A native completed receipt can repair a failed consumed write, before queue dedup.
 journal.reserve("write-failure", "operation-2", snapshotHash, "target-delivery");
 journal.attempted("write-failure", "target-delivery", "actual-native");
 const path = pathFor("write-failure"), original = readFileSync(path, "utf8");
 const realPath = realpathSync(path);
 const actualRename = fs.renameSync;
 fs.renameSync = (source, destination) => {
  if (destination === realPath) throw new Error("Injected consumed marker publication failure");
  actualRename(source, destination);
 };
 syncBuiltinESMExports();
 try { assert.throws(() => journal.consumed("write-failure", "target-delivery", "actual-native", ["target-delivery"]), /Injected consumed marker/); }
 finally { fs.renameSync = actualRename; syncBuiltinESMExports(); }
 assert.equal(readFileSync(path, "utf8"), original);
 assert.equal(new HandoffDeliveries(directory).reconcile("write-failure", "actual-native", ["target-delivery"])?.status, "consumed");
 assert.equal(readdirSync(join(directory, "session-handoffs", "deliveries")).some(name => name.endsWith(".tmp")), false);

 // Cancelled unattempted first input allows the next explicit input to reserve it.
 journal.reserve("cancel", "operation-3", snapshotHash, "cancelled");
 journal.releaseUnattempted("cancel", "cancelled");
 assert.equal(journal.reserve("cancel", "operation-3", snapshotHash, "next").deliveryId, "next");
 assert.equal(journal.reconcile("cancel", "source-native", ["next"])?.status, "reserved");
 assert.throws(() => journal.consumed("cancel", "next", "source-native", ["next"]), /same target/);

 for (const text of ["/skill", "  /extension args", "\n\t/command", "", " \n"]) assert.throws(() => assertHandoffFirstInput(text), /ordinary nonempty text/);
 for (const text of ["Continue please", "  inspect /src/file", "A / B"]) assert.doesNotThrow(() => assertHandoffFirstInput(text));
 if (process.platform !== "win32") {
  assert.equal(statSync(pathFor("target")).mode & 0o777, 0o600);
  assert.equal(statSync(join(directory, "session-handoffs")).mode & 0o777, 0o700);
  assert.equal(statSync(join(directory, "session-handoffs", "deliveries")).mode & 0o777, 0o700);
  symlinkSync(pathFor("target"), pathFor("linked"));
  assert.throws(() => journal.get("linked"), /Invalid handoff delivery file/);
 }
 writeFileSync(pathFor("bad"), JSON.stringify({ version: 1, targetId: "bad", operationId: "op", snapshotHash, status: "attempted", deliveryId: "delivery" }));
 assert.throws(() => journal.get("bad"), /identity/);
 writeFileSync(pathFor("bad"), JSON.stringify({ ...journal.get("target"), targetId: "bad", sourceCompletedIds: ["replacement"] }));
 assert.throws(() => journal.get("bad"), /Invalid handoff delivery journal/);
 console.log("handoff delivery: reservation, cancel, cold uncertain/receipt recovery, private storage and first-input checks passed");
} finally { rmSync(directory, { recursive: true, force: true }); }

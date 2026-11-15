import { createHash, randomUUID } from "node:crypto";
import fs, { chmodSync, closeSync, fsyncSync, lstatSync, mkdirSync, openSync, readFileSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";

export type HandoffDeliveryStatus = "pending" | "reserved" | "attempted" | "consumed" | "uncertain";
export interface HandoffDelivery {
 readonly version: 1;
 readonly targetId: string;
 readonly operationId: string;
 readonly snapshotHash: string;
 readonly status: HandoffDeliveryStatus;
 readonly deliveryId?: string;
 readonly nativeSessionId?: string;
}

function identifier(value: string): void {
 if (typeof value !== "string" || !value.trim() || value.length > 4096) throw new Error("Invalid handoff delivery identity");
}
function hash(value: string): void {
 if (typeof value !== "string" || !/^[a-f0-9]{64}$/.test(value)) throw new Error("Invalid handoff snapshot hash");
}
function syncDirectory(path: string): void {
 let fd: number | undefined;
 try { fd = openSync(path, "r"); fsyncSync(fd); }
 catch (error) {
  if (process.platform !== "win32" || !["EINVAL", "EPERM", "EISDIR", "EACCES"].includes((error as NodeJS.ErrnoException).code ?? "")) throw error;
 } finally { if (fd !== undefined) closeSync(fd); }
}

/** First-slice commands can complete locally without consuming context. Call before launch AND enqueue. */
export function assertHandoffFirstInput(text: string): void {
 if (typeof text !== "string" || !text.trim() || text.trimStart().startsWith("/")) {
  throw new Error("Handoff first input must be ordinary nonempty text; slash commands are not supported");
 }
}

/**
 * Synchronous private journal, with one runtime owner per data directory.
 * reserve precedes the first spool write; attempted follows the durable spool write
 * and precedes the native prompt. Never retry an attempted/uncertain delivery.
 * Reconcile against the target's durable native receipt BEFORE queue dedup starts.
 * Release only after proving the unattempted spool entry absent (or cancel durable).
 * A native identity passed here must come from this target's actual native handle,
 * never from the source or inherited receipts. This module does not read source receipts.
 */
export class HandoffDeliveries {
 private readonly directory: string;
 constructor(dataDir: string) {
  mkdirSync(dataDir, { recursive: true, mode: 0o700 });
  this.directory = join(realpathSync(resolve(dataDir)), "session-handoffs", "deliveries");
  mkdirSync(this.directory, { recursive: true, mode: 0o700 });
  this.validateDirectory();
  chmodSync(join(this.directory, ".."), 0o700);
  chmodSync(this.directory, 0o700);
  syncDirectory(this.directory);
  syncDirectory(join(this.directory, ".."));
  syncDirectory(join(this.directory, "..", ".."));
 }

 private validateDirectory(): void {
  for (const path of [join(this.directory, ".."), this.directory]) {
   const info = lstatSync(path);
   if (!info.isDirectory() || info.isSymbolicLink() || realpathSync(path) !== resolve(path)) throw new Error("Unsafe handoff delivery directory");
  }
 }

 private path(targetId: string): string {
  identifier(targetId);
  this.validateDirectory();
  return join(this.directory, `${createHash("sha256").update(targetId).digest("hex")}.json`);
 }

 get(targetId: string): HandoffDelivery | undefined {
  let contents: string;
  try {
   const path = this.path(targetId), info = lstatSync(path);
   if (!info.isFile() || info.isSymbolicLink() || info.size > 32768) throw new Error("Invalid handoff delivery file");
   contents = readFileSync(path, "utf8");
  }
  catch (error) { if ((error as NodeJS.ErrnoException).code === "ENOENT") return undefined; throw error; }
  const value: HandoffDelivery = JSON.parse(contents);
  if (!value || typeof value !== "object" || Array.isArray(value)
   || Object.keys(value).some(key => !["version", "targetId", "operationId", "snapshotHash", "status", "deliveryId", "nativeSessionId"].includes(key))
   || value.version !== 1 || value.targetId !== targetId
   || !["pending", "reserved", "attempted", "consumed", "uncertain"].includes(value.status)) throw new Error("Invalid handoff delivery journal");
  identifier(value.operationId); hash(value.snapshotHash);
  if (value.status === "pending") {
   if (value.deliveryId !== undefined || value.nativeSessionId !== undefined) throw new Error("Invalid pending handoff delivery");
  } else {
   identifier(value.deliveryId!);
   if (value.status === "reserved") {
    if (value.nativeSessionId !== undefined) throw new Error("Invalid reserved handoff delivery");
   } else identifier(value.nativeSessionId!);
  }
  return value;
 }

 private write(value: HandoffDelivery): HandoffDelivery {
  const path = this.path(value.targetId), temporary = `${path}.${randomUUID()}.tmp`;
  let fd: number | undefined;
  try {
   fd = openSync(temporary, "wx", 0o600);
   writeFileSync(fd, `${JSON.stringify(value)}\n`, "utf8");
   fsyncSync(fd); closeSync(fd); fd = undefined;
   fs.renameSync(temporary, path);
   syncDirectory(this.directory);
  } finally {
   if (fd !== undefined) closeSync(fd);
   rmSync(temporary, { force: true });
  }
  return structuredClone(value);
 }

 initialize(targetId: string, operationId: string, snapshotHash: string): HandoffDelivery {
  identifier(operationId); hash(snapshotHash);
  const current = this.get(targetId);
  if (current) {
   if (current.operationId !== operationId || current.snapshotHash !== snapshotHash) throw new Error("Handoff delivery context changed");
   return current;
  }
  return this.write({ version: 1, targetId, operationId, snapshotHash, status: "pending" });
 }

 reserve(targetId: string, operationId: string, snapshotHash: string, deliveryId: string): HandoffDelivery {
  identifier(deliveryId);
  const current = this.initialize(targetId, operationId, snapshotHash);
  if (current.status === "reserved" && current.deliveryId === deliveryId) return current;
  if (current.status !== "pending") throw new Error(`Handoff delivery cannot be reserved: ${current.status}`);
  return this.write({ ...current, status: "reserved", deliveryId });
 }

 private requireDelivery(targetId: string, deliveryId: string): HandoffDelivery {
  identifier(deliveryId);
  const current = this.get(targetId);
  if (!current || current.deliveryId !== deliveryId) throw new Error("Handoff delivery identity mismatch");
  return current;
 }

 attempted(targetId: string, deliveryId: string, nativeSessionId: string): HandoffDelivery {
  identifier(nativeSessionId);
  const current = this.requireDelivery(targetId, deliveryId);
  if (current.status !== "reserved") throw new Error(`Handoff delivery must not be retried: ${current.status}`);
  return this.write({ ...current, status: "attempted", nativeSessionId });
 }

 consumed(targetId: string, deliveryId: string, nativeSessionId: string, completedDeliveryIds: readonly string[]): HandoffDelivery {
  const current = this.requireDelivery(targetId, deliveryId);
  if (!["attempted", "uncertain", "consumed"].includes(current.status)
   || current.nativeSessionId !== nativeSessionId || !completedDeliveryIds.includes(deliveryId)) {
   throw new Error("Handoff consumption requires the same target native session and completed delivery receipt");
  }
  return current.status === "consumed" ? current : this.write({ ...current, status: "consumed" });
 }

 reconcile(targetId: string, nativeSessionId: string, completedDeliveryIds: readonly string[]): HandoffDelivery | undefined {
  identifier(nativeSessionId);
  const current = this.get(targetId);
  if (!current || current.status === "pending" || current.status === "reserved" || current.status === "consumed") return current;
  if (current.nativeSessionId === nativeSessionId && completedDeliveryIds.includes(current.deliveryId!)) {
   return this.consumed(targetId, current.deliveryId!, nativeSessionId, completedDeliveryIds);
  }
  return current.status === "uncertain" ? current : this.write({ ...current, status: "uncertain" });
 }

 releaseUnattempted(targetId: string, deliveryId: string): HandoffDelivery {
  const current = this.requireDelivery(targetId, deliveryId);
  if (current.status !== "reserved") throw new Error(`Attempted handoff delivery cannot be released: ${current.status}`);
  const { deliveryId: _delivery, nativeSessionId: _native, ...base } = current;
  return this.write({ ...base, status: "pending" });
 }

 /** Call only with a successfully read, complete spool for this target, before it may run. */
 reconcileQueue(targetId: string, persistedDeliveryIds: readonly string[]): HandoffDelivery | undefined {
  const current = this.get(targetId);
  return current?.status === "reserved" && !persistedDeliveryIds.includes(current.deliveryId!)
   ? this.releaseUnattempted(targetId, current.deliveryId!) : current;
 }
}

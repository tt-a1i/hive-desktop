import { createHash, randomUUID } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { isDeliveryPresentation, type DeliveryPresentation } from "./delivery-presentation.js";

export interface DeliveryItem {
 readonly id: string;
 readonly text: string;
 readonly triggerTurn: boolean;
 readonly presentation?: DeliveryPresentation;
}

interface DeliveryQueueOptions {
 directory: string;
 sessionId: string;
 startPaused?: boolean;
 execute(item: DeliveryItem): Promise<void>;
 completedIds(): readonly string[];
 onError(error: Error): void;
 onState?(busy: boolean, completedReportId?: string): void;
}

/** A transport spool, not a task ledger. One owner per session is required. */
export class DeliveryQueue {
 private readonly path: string;
 private items: DeliveryItem[];
 private running: Promise<void> | undefined;
 private paused = false;
 private attemptedId: string | undefined;
 private lastState: string | undefined;

 constructor(private readonly options: DeliveryQueueOptions) {
  this.paused = options.startPaused ?? false;
  mkdirSync(options.directory, { recursive: true });
  this.path = join(options.directory, `${createHash("sha256").update(options.sessionId).digest("hex")}.json`);
  const stored: unknown = existsSync(this.path) ? JSON.parse(readFileSync(this.path, "utf8")) : [];
  if (!Array.isArray(stored) || !stored.every(item => item && typeof item.id === "string" && typeof item.text === "string" && typeof item.triggerTurn === "boolean"
   && (item.presentation === undefined || isDeliveryPresentation(item.presentation)))) {
   throw new Error(`Invalid delivery spool: ${this.path}`);
  }
  this.items = stored;
  queueMicrotask(() => this.kick());
 }

 get pending(): readonly DeliveryItem[] { return this.items.map(item => ({ ...item, ...(item.presentation && { presentation: structuredClone(item.presentation) }) })); }
 get pendingCount(): number { return this.items.length; }
 get pendingReports() {
  return this.items.flatMap((item, index) => {
   const report = item.presentation;
   return report?.origin === 'report' ? [{ deliveryId: item.id, text: report.displayText,
    workerName: report.workerName, ...(report.workerId && { workerId: report.workerId }), artifacts: [...(report.artifacts ?? [])], processing: index === 0 && this.busy && !this.paused }] : [];
  });
 }
 get busy(): boolean { return this.running !== undefined; }

 /** Resolves after atomic persistence, without waiting for model execution. */
 async enqueue(text: string, triggerTurn = true, presentation?: DeliveryPresentation): Promise<void> {
  if (presentation !== undefined && !isDeliveryPresentation(presentation)) throw new Error("Invalid delivery presentation");
  this.replace([...this.items, { id: randomUUID(), text, triggerTurn, ...(presentation && { presentation: structuredClone(presentation) }) }]);
  this.kick();
  this.emitState();
 }

 resume(): void { this.paused = false; this.kick(); this.emitState(); }

 async idle(): Promise<void> {
  while (this.running) await this.running;
 }

 /** Prevents the next delivery; the caller must abort any running SDK call. */
 async pause(): Promise<void> { this.paused = true; await this.idle(); }

 /** After pause + SDK abort + idle, discard only the interrupted delivery. */
 discardCurrent(): void {
  if (!this.paused || this.busy) throw new Error("Pause and await idle before discarding a delivery");
  if (this.attemptedId && this.items[0]?.id === this.attemptedId) this.replace(this.items.slice(1));
  this.attemptedId = undefined;
  this.emitState();
 }

 private emitState(completedReportId?: string): void {
  const state = `${this.busy}:${this.items.length}`;
  if (state === this.lastState && !completedReportId) return;
  this.lastState = state;
  this.options.onState?.(this.busy, completedReportId);
 }

 private replace(items: DeliveryItem[]): void {
  const temporary = `${this.path}.${randomUUID()}.tmp`;
  try {
   writeFileSync(temporary, JSON.stringify(items), { encoding: "utf8", mode: 0o600, flush: true });
   renameSync(temporary, this.path);
  } finally { rmSync(temporary, { force: true }); }
  // A failed disk write must never change the accepted in-memory queue.
  this.items = items;
 }

 private kick(): void {
  if (this.paused || this.running || !this.items.length) return;
  // Set running before execute: an SDK tool can synchronously enqueue a reply.
  this.running = Promise.resolve().then(() => this.drain()).catch(error => {
   this.paused = true;
   this.options.onError(error instanceof Error ? error : new Error(String(error)));
  }).finally(() => {
   this.running = undefined;
   this.emitState();
   this.kick();
  });
  this.emitState();
 }

 private async drain(): Promise<void> {
  while (!this.paused && this.items.length) {
   const item = this.items[0]!;
   if (!this.options.completedIds().includes(item.id)) {
    this.attemptedId = item.id;
    await this.options.execute(item);
   }
   this.replace(this.items.slice(1));
   this.attemptedId = undefined;
   const completed = item.presentation?.origin === "report" && this.options.completedIds().includes(item.id);
   this.emitState(completed ? item.id : undefined);
  }
 }
}

import WebSocket, { type RawData } from 'ws';
import type { HiveHost } from './hive-host.js';

type TerminalPayload =
  | { type: 'restore'; sequence: number; bytes: number[]; cols: number; rows: number }
  | { type: 'output'; sequence: number; bytes: number[] }
  | { type: 'exit'; code: number | null }
  | { type: 'error'; message: string }
  | { type: 'closed'; reason: string };
export type TerminalEvent = { attachmentId: string; workspaceId: string; runId: string } & TerminalPayload;
export interface TerminalAttachment { attachmentId: string; workspaceId: string; runId: string; cols: number; rows: number }
const MAX_PENDING_BYTES = 1024 * 1024;
const MAX_SYSTEM_INPUT = 4096;
const buffer = (data: RawData): Buffer => Array.isArray(data) ? Buffer.concat(data) : Buffer.isBuffer(data) ? data : Buffer.from(data);

/** One Hive viewer: control/IO share identity; only parser-consumed output earns native credit. */
export class TerminalConnection {
  private io: WebSocket | undefined;
  private control: WebSocket | undefined;
  private closing: Promise<void> | undefined;
  private closed = false;
  private restored = false;
  private restoreSeen = false;
  private completing = false;
  private exited = false;
  private deferredExit: number | null | undefined;
  private sequence = 1;
  private pending: Buffer[] = [];
  private pendingBytes = 0;
  private readonly outstanding = new Map<number, number>();
  private outstandingBytes = 0;
  private systemInput: Buffer[] = [];
  private systemBytes = 0;
  private nextSize: { cols: number; rows: number } | undefined;
  constructor(readonly attachment: TerminalAttachment, private readonly hive: HiveHost,
    private readonly emit: (event: TerminalEvent) => void, private readonly onClosed: () => void) {}
  private event(event: TerminalPayload): void {
    this.emit({ attachmentId: this.attachment.attachmentId, workspaceId: this.attachment.workspaceId, runId: this.attachment.runId, ...event } as TerminalEvent);
  }
  async connect(): Promise<void> {
    // IO captures the opt-in native snapshot boundary before any live chunk; control consumes that same generation's snapshot.
    this.io = this.socket('io');
    this.io.on('message', data => {
      if (this.closed) return;
      const bytes = buffer(data);
      if (!bytes.length) return;
      if (this.pendingBytes + this.outstandingBytes + bytes.length > MAX_PENDING_BYTES) { this.fail(new Error('Terminal viewer exceeded its unconsumed output limit')); return; }
      if (!this.restored) { this.pending.push(bytes); this.pendingBytes += bytes.length; }
      else this.output(bytes);
    });
    await this.opened(this.io);
    if (this.closed) throw new Error('Terminal attachment was closed');
    this.control = this.socket('control');
    this.control.on('message', data => {
      if (this.closed) return;
      try {
        const message: unknown = JSON.parse(buffer(data).toString('utf8'));
        if (!message || typeof message !== 'object') throw new Error('Invalid terminal control message');
        const record = message as Record<string, unknown>;
        if (record.type === 'restore' && typeof record.snapshot === 'string' && !this.restoreSeen) {
          this.restoreSeen = true;
          this.event({ type: 'restore', sequence: 1, bytes: [...Buffer.from(record.snapshot, 'utf8')], cols: this.attachment.cols, rows: this.attachment.rows });
        } else if (record.type === 'exit' && (record.code === null || (typeof record.code === 'number' && Number.isInteger(record.code)))) {
          this.exited = true;
          if (this.restored) this.event({ type: 'exit', code: record.code });
          else this.deferredExit = record.code;
        } else if (record.type === 'error' && typeof record.message === 'string') this.event({ type: 'error', message: record.message });
        else throw new Error('Invalid terminal control message');
      } catch (error) { this.fail(error instanceof Error ? error : new Error(String(error))); }
    });
    await this.opened(this.control);
  }
  private socket(channel: 'io' | 'control'): WebSocket {
    const socket = this.hive.openTerminalSocket(this.attachment.runId, channel, this.attachment.attachmentId, this.attachment.cols, this.attachment.rows);
    socket.on('error', error => { if (!this.closed) this.fail(error); });
    socket.on('close', () => { if (!this.closed) void this.close('connection-closed'); });
    return socket;
  }
  private opened(socket: WebSocket): Promise<void> {
    if (socket.readyState === WebSocket.OPEN) return Promise.resolve();
    return new Promise((resolve, reject) => {
      const cleanup = () => { socket.off('open', success); socket.off('error', failure); socket.off('close', ended); };
      const success = () => { cleanup(); resolve(); };
      const failure = (error: Error) => { cleanup(); reject(error); };
      const ended = () => failure(new Error('Terminal WebSocket closed before opening'));
      socket.once('open', success); socket.once('error', failure); socket.once('close', ended);
    });
  }
  private send(socket: WebSocket | undefined, data: string | Buffer): Promise<void> {
    if (this.closed || socket?.readyState !== WebSocket.OPEN) return Promise.reject(new Error('Terminal attachment is disconnected'));
    return new Promise((resolve, reject) => socket.send(data, error => error ? reject(error) : resolve()));
  }
  async restoreComplete(sequence: number): Promise<{ ok: true; ready: true }> {
    if (this.closed || !this.restoreSeen || this.restored || this.completing || sequence !== 1) throw new Error('Terminal restore acknowledgement is stale');
    this.completing = true;
    await this.send(this.control, JSON.stringify({ type: 'restore_complete' }));
    while (this.nextSize || this.systemInput.length) {
      if (this.nextSize) {
        const next = this.nextSize; this.nextSize = undefined;
        await this.send(this.control, JSON.stringify({ type: 'resize', ...next }));
      }
      const pending = this.systemInput; this.systemInput = [];
      this.systemBytes -= pending.reduce((total, bytes) => total + bytes.length, 0);
      for (const bytes of pending) await this.send(this.io, bytes);
    }
    if (this.closed) throw new Error('Terminal attachment closed during restore');
    this.restored = true;
    const pending = this.pending; this.pending = []; this.pendingBytes = 0;
    for (const bytes of pending) this.output(bytes);
    this.finishDeferredExit();
    return { ok: true, ready: true };
  }
  private finishDeferredExit(): void {
    if (this.deferredExit === undefined || this.outstanding.size || this.closed) return;
    const code = this.deferredExit;
    this.deferredExit = undefined;
    this.event({ type: 'exit', code });
  }
  private output(bytes: Buffer): void {
    const sequence = ++this.sequence;
    this.outstanding.set(sequence, bytes.length); this.outstandingBytes += bytes.length;
    this.event({ type: 'output', sequence, bytes: [...bytes] });
  }
  async ack(sequence: number, bytes: number): Promise<{ ok: true }> {
    const first = this.outstanding.entries().next().value as [number, number] | undefined;
    if (!this.restored || !first || first[0] !== sequence || first[1] !== bytes) throw new Error('Terminal output acknowledgement does not match the next consumed frame');
    // Reserve synchronously so duplicate concurrent RPCs cannot grant credit twice.
    this.outstanding.delete(sequence); this.outstandingBytes -= bytes;
    await this.send(this.control, JSON.stringify({ type: 'output_ack', bytes }));
    this.finishDeferredExit();
    return { ok: true };
  }
  async input(bytes: Buffer, userInput: boolean): Promise<{ ok: true }> {
    // A restored screen can query the cursor even after its process has exited.
    if (!this.closed && this.exited && !userInput) return { ok: true };
    if (this.closed || this.exited) throw new Error('Terminal shell is no longer accepting input');
    if (!this.restored) {
      if (userInput) throw new Error('Terminal restore has not been consumed');
      if (this.systemBytes + bytes.length > MAX_SYSTEM_INPUT) throw new Error('Terminal restore response limit exceeded');
      this.systemInput.push(bytes); this.systemBytes += bytes.length;
    } else await this.send(this.io, bytes);
    return { ok: true };
  }
  async resize(cols: number, rows: number): Promise<{ ok: true }> {
    if (this.closed) throw new Error('Terminal attachment is disconnected');
    if (!this.restored) this.nextSize = { cols, rows };
    else await this.send(this.control, JSON.stringify({ type: 'resize', cols, rows }));
    return { ok: true };
  }
  private fail(error: Error): void { this.event({ type: 'error', message: error.message }); void this.close('connection-error'); }
  close(reason: string): Promise<void> {
    if (this.closing) return this.closing;
    this.closed = true;
    this.pending = []; this.pendingBytes = 0; this.outstanding.clear(); this.outstandingBytes = 0; this.systemInput = []; this.systemBytes = 0;
    this.closing = Promise.all([this.io, this.control].map(socket => {
      if (!socket || socket.readyState === WebSocket.CLOSED) return Promise.resolve();
      return new Promise<void>(resolve => { socket.once('close', () => resolve()); socket.terminate(); });
    })).then(() => { this.event({ type: 'closed', reason }); this.onClosed(); });
    return this.closing;
  }
}

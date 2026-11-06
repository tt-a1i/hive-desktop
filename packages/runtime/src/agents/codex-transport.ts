import type { ChildProcessWithoutNullStreams } from 'node:child_process';
import { OwnedCliProcess } from './owned-cli-process.js';

export type CodexRequestId = number | string;
export type CodexServerRequest = { method: string; params: unknown; id: CodexRequestId };
export type CodexNotification = { method: string; params: unknown };
export type CodexCloseInfo = { code: number | null; signal: NodeJS.Signals | null; error: Error };
export interface CodexTransportOptions {
  cwd: string;
  command?: string;
  requestTimeoutMs?: number;
  shutdownGraceMs?: number;
  onRequest?: (request: CodexServerRequest) => unknown | Promise<unknown>;
  onNotification?: (notification: CodexNotification) => void;
  onClose?: (info: CodexCloseInfo) => void;
}

export class CodexRpcError extends Error {
  constructor(readonly code: number, message: string, readonly data?: unknown) {
    super(message);
    this.name = 'CodexRpcError';
  }
}

type Pending = { resolve: (result: unknown) => void; reject: (error: Error) => void; timer: NodeJS.Timeout };
const MAX_LINE_BYTES = 8 * 1024 * 1024;
const MAX_STDERR_BYTES = 16 * 1024;

/** Local Node backend only. Windows CLI launch has not been verified. */
export class CodexTransport {
  private readonly child: ChildProcessWithoutNullStreams;
  private readonly pending = new Map<number, Pending>();
  private readonly notifications = new Set<(notification: CodexNotification) => void>();
  private readonly process: OwnedCliProcess;
  private nextId = 1;
  private closing = false;
  private closeTask: Promise<void> | undefined;
  private ended = false;
  private diagnosticBuffer = Buffer.alloc(0);

  private constructor(private readonly options: CodexTransportOptions) {
    this.process = new OwnedCliProcess(options.command ?? 'codex', ['app-server', '--listen', 'stdio://'], {
      cwd: options.cwd,
    });
    this.child = this.process.child;
    this.child.once('close', (code, signal) => {
      this.finish(new Error(`Codex app-server closed (${code ?? signal ?? 'unknown'})`), code, signal);
    });
    this.child.once('error', (error) => this.fail(error));
    this.child.once('exit', (code, signal) => {
      this.finish(new Error(`Codex app-server exited (${code ?? signal ?? 'unknown'})`), code, signal);
    });
    this.child.stdin.on('error', (error) => this.fail(error));
    this.child.stdout.on('error', (error) => this.fail(error));
    this.child.stderr.on('error', (error) => this.fail(error));
    this.child.stderr.on('data', (chunk: Buffer) => {
      this.diagnosticBuffer = Buffer.concat([this.diagnosticBuffer, chunk]).subarray(-MAX_STDERR_BYTES);
    });
    let buffered = Buffer.alloc(0);
    this.child.stdout.on('data', (chunk: Buffer) => {
      if (this.ended) return;
      buffered = Buffer.concat([buffered, chunk]);
      let newline: number;
      while ((newline = buffered.indexOf(10)) !== -1) {
        if (newline > MAX_LINE_BYTES) return this.fail(new Error('Codex JSONL line exceeds limit'));
        const line = buffered.subarray(0, newline).toString('utf8').trim();
        buffered = buffered.subarray(newline + 1);
        if (line) {
          try { this.receive(JSON.parse(line)); }
          catch (error) { return this.fail(asError(error)); }
        }
      }
      if (buffered.length > MAX_LINE_BYTES) this.fail(new Error('Codex JSONL line exceeds limit'));
    });
    this.child.stdout.once('end', () => {
      if (!this.closing) this.fail(new Error('Unexpected Codex app-server stdout EOF'));
    });
    if (options.onNotification) this.notifications.add(options.onNotification);
  }

  static async start(options: CodexTransportOptions): Promise<CodexTransport> {
    for (const value of [options.requestTimeoutMs, options.shutdownGraceMs]) {
      if (value !== undefined && (!Number.isFinite(value) || value <= 0 || value > 2_147_483_647)) {
        throw new RangeError('Codex timeouts must be positive finite milliseconds, at most 2147483647');
      }
    }
    const transport = new CodexTransport(options);
    try {
      await transport.sendRequest('initialize', {
        clientInfo: { name: 'hive_desktop', title: 'Hive Desktop', version: '0.1.0' },
        capabilities: { experimentalApi: true, requestAttestation: false },
      });
      await transport.write({ method: 'initialized' });
      return transport;
    } catch (error) {
      await transport.close();
      throw error;
    }
  }

  get pid(): number | undefined { return this.child.pid; }
  get diagnostics(): string { return this.diagnosticBuffer.toString('utf8'); }

  onNotification(listener: (notification: CodexNotification) => void): () => void {
    this.notifications.add(listener);
    return () => { this.notifications.delete(listener); };
  }

  sendRequest<T = unknown>(method: string, params: unknown): Promise<T> {
    if (this.closing || this.ended) return Promise.reject(new Error('Codex transport is closed'));
    const id = this.nextId++;
    return new Promise<T>((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(`Codex request timed out: ${method}`));
      }, this.options.requestTimeoutMs ?? 30_000);
      this.pending.set(id, { resolve: (value) => resolve(value as T), reject, timer });
      void this.write({ method, params, id }).catch((error: unknown) => this.fail(asError(error)));
    });
  }

  close(): Promise<void> {
    if (this.closeTask) return this.closeTask;
    this.closing = true;
    this.rejectPending(new Error('Codex transport is closing'));
    this.closeTask = this.process.close(this.options.shutdownGraceMs ?? 1500);
    return this.closeTask;
  }

  private async write(message: unknown): Promise<void> {
    if (this.closing || this.ended) return Promise.reject(new Error('Codex transport is closed'));
    const line = `${JSON.stringify(message)}\n`;
    return new Promise((resolve, reject) => {
      this.child.stdin.write(line, (error) => error ? reject(error) : resolve());
    });
  }

  private receive(message: unknown): void {
    if (!isObject(message)) throw new Error('Invalid Codex JSONL message');
    if (typeof message.method === 'string') {
      if ('id' in message) {
        if (typeof message.id !== 'string' && typeof message.id !== 'number') throw new Error('Invalid Codex request id');
        void this.answer({ method: message.method, params: message.params, id: message.id });
      } else {
        const notification = { method: message.method, params: message.params };
        for (const listener of this.notifications) {
          // Consumer failures do not change transport or approval semantics.
          try { listener(notification); } catch { /* isolate consumer callback */ }
        }
      }
      return;
    }
    if (typeof message.id !== 'number') throw new Error('Invalid Codex response id');
    const pending = this.pending.get(message.id);
    if (!pending) return; // A timed-out request may respond later.
    this.pending.delete(message.id);
    clearTimeout(pending.timer);
    if ('error' in message) {
      const error = message.error;
      if (!isObject(error) || typeof error.code !== 'number' || typeof error.message !== 'string') {
        pending.reject(new Error('Invalid Codex RPC error'));
      } else pending.reject(new CodexRpcError(error.code, error.message, error.data));
    } else if ('result' in message) pending.resolve(message.result);
    else pending.reject(new Error('Invalid Codex RPC response'));
  }

  private async answer(request: CodexServerRequest): Promise<void> {
    let response: unknown;
    try {
      if (!this.options.onRequest) throw new CodexRpcError(-32601, `Unsupported server request: ${request.method}`);
      const result = await this.options.onRequest(request);
      response = { id: request.id, result: result ?? null };
    } catch (error) {
      const rpc = error instanceof CodexRpcError ? error : new CodexRpcError(-32603, asError(error).message);
      response = { id: request.id, error: { code: rpc.code, message: rpc.message, ...(rpc.data === undefined ? {} : { data: rpc.data }) } };
    }
    if (this.closing || this.ended) return;
    try { await this.write(response); } catch (error) { this.fail(asError(error)); }
  }

  private rejectPending(error: Error): void {
    for (const pending of this.pending.values()) {
      clearTimeout(pending.timer);
      pending.reject(error);
    }
    this.pending.clear();
  }

  private finish(error: Error, code: number | null, signal: NodeJS.Signals | null): void {
    if (this.ended) return;
    this.ended = true;
    this.rejectPending(error);
    try { this.options.onClose?.({ code, signal, error }); } catch { /* isolate consumer callback */ }
  }

  private fail(error: Error): void {
    this.finish(error, this.child.exitCode, this.child.signalCode);
    void this.close().catch(() => { /* Awaited close() retains cleanup failure for the owner. */ });
  }
}

function isObject(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}
function asError(value: unknown): Error { return value instanceof Error ? value : new Error(String(value)); }

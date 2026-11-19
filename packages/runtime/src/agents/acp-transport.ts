import type { ChildProcessWithoutNullStreams } from 'node:child_process';
import { OwnedCliProcess } from './owned-cli-process.js';

export type AcpRequestId = number | string;
export type AcpServerRequest = { method: string; params: unknown; id: AcpRequestId };
export type AcpNotification = { method: string; params: unknown };
export type AcpCloseInfo = { code: number | null; signal: NodeJS.Signals | null; error: Error };
export interface AcpTransportOptions {
  cwd: string;
  command: string;
  args?: string[];
  env?: NodeJS.ProcessEnv;
  requestTimeoutMs?: number;
  shutdownGraceMs?: number;
  signal?: AbortSignal;
  onRequest?: (request: AcpServerRequest) => unknown | Promise<unknown>;
  onNotification?: (notification: AcpNotification) => void;
  onClose?: (info: AcpCloseInfo) => void;
}

export class AcpRequestCancelled extends Error { constructor() { super("Native request is no longer pending"); this.name = "AcpRequestCancelled"; } }

export class AcpRpcError extends Error {
  constructor(readonly code: number, message: string, readonly data?: unknown) {
    super(message);
    this.name = 'AcpRpcError';
  }
}

type Pending = { resolve: (result: unknown) => void; reject: (error: Error) => void; timer: NodeJS.Timeout };
// Bound memory used by one JSONL frame, including native tool results.
const MAX_LINE_BYTES = 32 * 1024 * 1024;
const MAX_STDERR_BYTES = 16 * 1024;

/** JSONL transport for the configured local ACP CLI. */
export class AcpTransport {
  private readonly child: ChildProcessWithoutNullStreams;
  private readonly pending = new Map<number, Pending>();
  private readonly incoming = new Map<AcpRequestId, { cancelled: boolean }>();
  private readonly notifications = new Set<(notification: AcpNotification) => void>();
  private readonly process: OwnedCliProcess;
  initializeResult: unknown;
  private nextId = 1;
  private closing = false;
  private closeTask: Promise<void> | undefined;
  private ended = false;
  private diagnosticBuffer = Buffer.alloc(0);

  private constructor(private readonly options: AcpTransportOptions) {
    this.process = new OwnedCliProcess(options.command, options.args ?? [], {
      cwd: options.cwd,
      env: { ...process.env, ...options.env },
    });
    this.child = this.process.child;
    this.child.once('close', (code, signal) => {
      this.finish(new Error(`ACP process closed (${code ?? signal ?? 'unknown'})`), code, signal);
    });
    this.child.once('error', (error) => this.fail(error));
    this.child.once('exit', (code, signal) => {
      this.finish(new Error(`ACP process exited (${code ?? signal ?? 'unknown'})`), code, signal);
    });
    this.child.stdin.on('error', (error) => this.fail(error));
    this.child.stdout.on('error', (error) => this.fail(error));
    this.child.stderr.on('error', (error) => this.fail(error));
    this.child.stderr.on('data', (chunk: Buffer) => {
      this.diagnosticBuffer = Buffer.concat([this.diagnosticBuffer, chunk]).subarray(-MAX_STDERR_BYTES);
    });
    let chunks: Buffer[] = [];
    let bufferedBytes = 0;
    this.child.stdout.on('data', (chunk: Buffer) => {
      if (this.ended) return;
      let start = 0;
      for (;;) {
        const newline = chunk.indexOf(10, start);
        if (newline === -1) {
          const tail = start === 0 ? chunk : chunk.subarray(start);
          if (tail.length > 0) {
            if (bufferedBytes + tail.length > MAX_LINE_BYTES) {
              chunks = [];
              bufferedBytes = 0;
              return this.fail(new Error('Acp JSONL line exceeds limit'));
            }
            chunks.push(tail);
            bufferedBytes += tail.length;
          }
          return;
        }
        const head = chunk.subarray(start, newline);
        start = newline + 1;
        const lineLength = bufferedBytes + head.length;
        if (lineLength > MAX_LINE_BYTES) {
          chunks = [];
          bufferedBytes = 0;
          return this.fail(new Error('Acp JSONL line exceeds limit'));
        }
        const rawLine = chunks.length === 0
          ? head
          : head.length === 0 && chunks.length === 1
            ? chunks[0]!
            : Buffer.concat(head.length === 0 ? chunks : [...chunks, head], lineLength);
        chunks = [];
        bufferedBytes = 0;
        const line = rawLine.toString('utf8').trim();
        if (line) {
          try { this.receive(JSON.parse(line)); }
          catch (error) { return this.fail(asError(error)); }
        }
        if (this.ended) return;
      }
    });
    this.child.stdout.once('end', () => {
      if (!this.closing) this.fail(new Error('Unexpected ACP process stdout EOF'));
    });
    if (options.onNotification) this.notifications.add(options.onNotification);
  }

  static async start(options: AcpTransportOptions): Promise<AcpTransport> {
    for (const value of [options.requestTimeoutMs, options.shutdownGraceMs]) {
      if (value !== undefined && (!Number.isFinite(value) || value <= 0 || value > 2_147_483_647)) {
        throw new RangeError('Acp timeouts must be positive finite milliseconds, at most 2147483647');
      }
    }
    options.signal?.throwIfAborted();
    const transport = new AcpTransport(options);
    const onAbort = () => { void transport.close(200).catch(() => {}); };
    options.signal?.addEventListener('abort', onAbort, { once: true });
    try {
      transport.initializeResult = await transport.sendRequest('initialize', {
        protocolVersion: 1, clientCapabilities: {}, clientInfo: { name: 'hive-desktop', version: '0.1.0' },
      });
      if (!isObject(transport.initializeResult) || transport.initializeResult.protocolVersion !== 1) throw new Error('Unsupported ACP protocol version; Hive supports v1');
      options.signal?.throwIfAborted();
      return transport;
    } catch (error) {
      await transport.close();
      const diagnostic = transport.diagnostics.trim().slice(-1200);
      if (diagnostic && error instanceof Error && !('code' in error)) {
        throw new Error(`Acp 初始化失败（${options.command ?? 'acp'}）。请在「设置 → 外部程序」核对可执行文件。\n${error.message}\n${diagnostic}`, { cause: error });
      }
      throw error;
    } finally {
      options.signal?.removeEventListener('abort', onAbort);
    }
  }

  get pid(): number | undefined { return this.child.pid; }
  get lifecycleDiagnostics() { return this.process.diagnostics; }
  get diagnostics(): string { return this.diagnosticBuffer.length ? 'ACP process wrote stderr; raw content withheld because the process may hold private environment values.' : ''; }

  onNotification(listener: (notification: AcpNotification) => void): () => void {
    this.notifications.add(listener);
    return () => { this.notifications.delete(listener); };
  }

  sendRequest<T = unknown>(method: string, params: unknown, timeoutMs = this.options.requestTimeoutMs ?? 30_000, observeResult?: (value: T) => void): Promise<T> {
    if (this.closing || this.ended) return Promise.reject(new Error('Acp transport is closed'));
    const id = this.nextId++;
    return new Promise<T>((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(`Acp request timed out: ${method}`));
      }, timeoutMs);
      // Observe a response before processing the next frame in the same chunk.
      this.pending.set(id, { resolve: (value) => {
        try { observeResult?.(value as T); resolve(value as T); }
        catch (error) { reject(asError(error)); }
      }, reject, timer });
      void this.write({ method, params, id }).catch((error: unknown) => this.fail(asError(error)));
    });
  }

  close(graceMs = this.options.shutdownGraceMs ?? 1500): Promise<void> {
    if (this.closeTask) return this.closeTask;
    this.closing = true;
    this.rejectPending(new Error('Acp transport is closing'));
    this.closeTask = this.process.close(graceMs);
    return this.closeTask;
  }

  cancelServerRequest(id: AcpRequestId): void { const pending = this.incoming.get(id); if (pending) pending.cancelled = true; }

  notify(method: string, params: unknown): Promise<void> { return this.write({ method, params }); }

  private async write(message: unknown): Promise<void> {
    if (this.closing || this.ended) return Promise.reject(new Error('Acp transport is closed'));
    const line = `${JSON.stringify({ jsonrpc: '2.0', ...(message as Record<string, unknown>) })}\n`;
    return new Promise((resolve, reject) => {
      this.child.stdin.write(line, (error) => error ? reject(error) : resolve());
    });
  }

  private receive(message: unknown): void {
    if (!isObject(message) || message.jsonrpc !== '2.0') throw new Error('Invalid ACP JSON-RPC message');
    if (typeof message.method === 'string') {
      if ('id' in message) {
        if (typeof message.id !== 'string' && typeof message.id !== 'number') throw new Error('Invalid Acp request id');
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
    if (typeof message.id !== 'number') throw new Error('Invalid Acp response id');
    const pending = this.pending.get(message.id);
    if (!pending) return; // A timed-out request may respond later.
    this.pending.delete(message.id);
    clearTimeout(pending.timer);
    if ('error' in message) {
      const error = message.error;
      if (!isObject(error) || typeof error.code !== 'number' || typeof error.message !== 'string') {
        pending.reject(new Error('Invalid Acp RPC error'));
      } else pending.reject(new AcpRpcError(error.code, error.message, error.data));
    } else if ('result' in message) pending.resolve(message.result);
    else pending.reject(new Error('Invalid Acp RPC response'));
  }

  private async answer(request: AcpServerRequest): Promise<void> {
    const incoming = { cancelled: false };
    if (this.incoming.has(request.id)) { this.fail(new Error("Duplicate inbound Acp request id")); return; }
    this.incoming.set(request.id, incoming);
    let response: unknown;
    try {
      if (!this.options.onRequest) throw new AcpRpcError(-32601, `Unsupported server request: ${request.method}`);
      const result = await this.options.onRequest(request);
      response = { id: request.id, result: result ?? null };
    } catch (error) {
      if (error instanceof AcpRequestCancelled) { this.incoming.delete(request.id); return; }
      const rpc = error instanceof AcpRpcError ? error : new AcpRpcError(-32603, asError(error).message);
      response = { id: request.id, error: { code: rpc.code, message: rpc.message, ...(rpc.data === undefined ? {} : { data: rpc.data }) } };
    }
    this.incoming.delete(request.id);
    if (this.closing || this.ended || incoming.cancelled) return;
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

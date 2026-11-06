import type { ChildProcessWithoutNullStreams } from 'node:child_process';
import { OwnedCliProcess } from './owned-cli-process.js';
import { randomUUID } from 'node:crypto';

export type ExternalPiRecord = Record<string, unknown> & { type: string };
export interface ExternalPiTransportOptions {
  cwd: string;
  sessionFile?: string;
  sessionDir: string;
  extensionPath: string;
  command?: string;
  model?: { provider: string; id: string };
  env?: NodeJS.ProcessEnv;
  requestTimeoutMs?: number;
  shutdownGraceMs?: number;
  onEvent?: (event: ExternalPiRecord) => void;
  onClose?: (error: Error) => void;
  onUiRequest?: (request: ExternalPiRecord) => Promise<{ value?: string; confirmed?: boolean; cancelled?: boolean } | undefined>;
}
type Pending = { command: string; resolve(value: unknown): void; reject(error: Error): void; timer: NodeJS.Timeout };
const dialogs = new Set(['select', 'confirm', 'input', 'editor']);

/** Pi 1.4 JSONL. User extensions/config/auth remain native to the external install. */
export class ExternalPiTransport {
  private readonly child: ChildProcessWithoutNullStreams;
  private readonly pending = new Map<string, Pending>();
  private readonly listeners = new Set<(event: ExternalPiRecord) => void>();
  private readonly process: OwnedCliProcess;
  private ended = false;
  private closing = false;
  private closeTask: Promise<void> | undefined;
  private stderr = Buffer.alloc(0);
  readonly bridgeCommand = `hive-desktop-${randomUUID()}`;
  readonly teamToolName = `hive_team_${randomUUID().replaceAll('-', '')}`;

  private constructor(private readonly options: ExternalPiTransportOptions) {
    if (options.onEvent) this.listeners.add(options.onEvent);
    this.process = new OwnedCliProcess(options.command ?? process.env.HIVE_PI_COMMAND ?? 'pi', [
      '--mode', 'rpc', '--session-dir', options.sessionDir,
      ...(options.model ? ['--provider', options.model.provider, '--model', options.model.id] : []),
      ...(options.sessionFile ? ['--session', options.sessionFile] : []), '--extension', options.extensionPath,
    ], { cwd: options.cwd,
      env: { ...process.env, ...options.env, HIVE_PI_BRIDGE_COMMAND: this.bridgeCommand, HIVE_PI_TEAM_TOOL: this.teamToolName } });
    this.child = this.process.child;
    this.child.once('close', (code, signal) => {
      this.finish(new Error(`External Pi closed (${code ?? signal ?? 'unknown'})`));
    });
    this.child.once('error', error => this.fail(error));
    this.child.once('exit', (code, signal) => this.finish(new Error(`External Pi exited (${code ?? signal ?? 'unknown'})`)));
    for (const stream of [this.child.stdin, this.child.stdout, this.child.stderr]) stream.on('error', error => this.fail(error));
    this.child.stderr.on('data', (chunk: Buffer) => { this.stderr = Buffer.concat([this.stderr, chunk]).subarray(-16384); });
    let buffer = Buffer.alloc(0);
    this.child.stdout.on('data', (chunk: Buffer) => {
      if (this.ended) return;
      buffer = Buffer.concat([buffer, chunk]);
      let newline: number;
      while ((newline = buffer.indexOf(10)) !== -1) {
        if (newline > 8 * 1024 * 1024) return this.fail(new Error('External Pi JSONL line exceeds limit'));
        const line = buffer.subarray(0, newline).toString('utf8').trim();
        buffer = buffer.subarray(newline + 1);
        if (line) {
          try { this.receive(JSON.parse(line)); } catch (error) { return this.fail(asError(error)); }
        }
      }
      if (buffer.length > 8 * 1024 * 1024) this.fail(new Error('External Pi JSONL line exceeds limit'));
    });
    this.child.stdout.once('end', () => { if (!this.closing) this.fail(new Error('Unexpected external Pi stdout EOF')); });
  }
  static async start(options: ExternalPiTransportOptions): Promise<ExternalPiTransport> {
    for (const ms of [options.requestTimeoutMs, options.shutdownGraceMs]) {
      if (ms !== undefined && (!Number.isFinite(ms) || ms <= 0 || ms > 2147483647)) throw new RangeError('Invalid external Pi timeout');
    }
    const transport = new ExternalPiTransport(options);
    try {
      await transport.request('get_state');
      const result = await transport.request<{ commands: { name: string; source: string; sourceInfo?: { path?: string } }[] }>('get_commands');
      const commands = result.commands.filter(command => command.name === transport.bridgeCommand);
      if (commands.length !== 1 || commands[0]?.source !== 'extension' || commands[0].sourceInfo?.path !== options.extensionPath) {
        throw new Error('External Pi Hive bridge command missing or colliding');
      }
      await transport.bridge({ action: 'initialize' });
      return transport;
    } catch (error) { await transport.close(); throw error; }
  }
  get pid(): number | undefined { return this.child.pid; }
  get diagnostics(): string { return this.stderr.toString('utf8'); }
  onEvent(listener: (event: ExternalPiRecord) => void): () => void {
    this.listeners.add(listener); return () => { this.listeners.delete(listener); };
  }
  request<T = unknown>(type: string, args: Record<string, unknown> = {}): Promise<T> {
    if (this.closing || this.ended) return Promise.reject(new Error('External Pi transport is closed'));
    const id = randomUUID();
    return new Promise<T>((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(id); reject(new Error(`External Pi request timed out: ${type}`));
      }, this.options.requestTimeoutMs ?? 30000);
      this.pending.set(id, { command: type, resolve: value => resolve(value as T), reject, timer });
      void this.write({ ...args, type, id }).catch(error => this.fail(asError(error)));
    });
  }
  /** Completion of this RPC is command handling only; triggered runs settle separately. */
  async bridge(payload: Record<string, unknown>): Promise<{ disposition: string }> {
    let failure: string | undefined;
    const unsubscribe = this.onEvent(event => {
      if (event.type === 'extension_error' && event.extensionPath === `command:${this.bridgeCommand}`) failure = String(event.error);
    });
    try {
      const result = await this.request<{ disposition: string }>('prompt', { message: `/${this.bridgeCommand} ${JSON.stringify(payload)}` });
      if (failure) throw new Error(failure);
      if (result.disposition !== 'handled') throw new Error('External Pi did not handle Hive bridge command');
      return result;
    } finally { unsubscribe(); }
  }
  close(): Promise<void> {
    if (this.closeTask) return this.closeTask;
    this.closing = true;
    this.rejectPending(new Error('External Pi transport is closing'));
    this.closeTask = this.process.close(this.options.shutdownGraceMs ?? 1500); return this.closeTask;
  }
  private write(record: unknown): Promise<void> {
    if (this.closing || this.ended) return Promise.reject(new Error('External Pi transport is closed'));
    return new Promise((resolve, reject) => this.child.stdin.write(`${JSON.stringify(record)}\n`, error => error ? reject(error) : resolve()));
  }
  private receive(value: unknown): void {
    if (!isRecord(value)) throw new Error('Invalid external Pi JSONL record');
    if (value.type === 'response') {
      if (typeof value.id !== 'string') throw new Error('Uncorrelated external Pi response');
      const pending = this.pending.get(value.id);
      if (!pending) return;
      this.pending.delete(value.id); clearTimeout(pending.timer);
      if (value.command !== pending.command) pending.reject(new Error('External Pi response command mismatch'));
      else if (value.success === true) pending.resolve(value.data);
      else pending.reject(new Error(typeof value.error === 'string' ? value.error : 'External Pi command failed'));
    } else {
      for (const listener of this.listeners) {
        try { listener(value); } catch (error) { this.fail(asError(error)); return; }
      }
      if (value.type === 'extension_ui_request') void this.handleUi(value).catch(error => this.fail(asError(error)));
    }
  }
  private async handleUi(request: ExternalPiRecord): Promise<void> {
    const response = await this.options.onUiRequest?.(request);
    if (!dialogs.has(String(request.method))) return;
    if (typeof request.id !== 'string') throw new Error('Invalid external Pi UI request');
    if (this.closing || this.ended) return;
    await this.write({ ...response, type: 'extension_ui_response', id: request.id, ...(response ? {} : { cancelled: true }) });
    if (!response) {
      const error: ExternalPiRecord = { type: 'hive_extension_ui_unsupported', method: request.method, message: 'External Pi extension dialog cancelled: desktop UI handler unavailable' };
      for (const listener of this.listeners) listener(error);
    }
  }
  private rejectPending(error: Error): void {
    for (const pending of this.pending.values()) { clearTimeout(pending.timer); pending.reject(error); }
    this.pending.clear();
  }
  private finish(error: Error): void {
    if (this.ended) return;
    this.ended = true; this.rejectPending(error); this.options.onClose?.(error);
  }
  private fail(error: Error): void { this.finish(error); void this.close().catch(() => { /* Owner receives this failure from awaited close(). */ }); }
}
function isRecord(value: unknown): value is ExternalPiRecord {
  return !!value && typeof value === 'object' && 'type' in value && typeof value.type === 'string' && !Array.isArray(value);
}
function asError(value: unknown): Error { return value instanceof Error ? value : new Error(String(value)); }

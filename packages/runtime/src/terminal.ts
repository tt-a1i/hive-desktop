import type { HiveHost } from './hive-host.js';
import { TerminalConnection, type TerminalAttachment, type TerminalEvent } from './terminal-connection.js';
export type { TerminalEvent } from './terminal-connection.js';
export interface ShellRun { runId: string; name: string; status: 'starting' | 'running'; exitCode: null }
const size = (cols: number, rows: number): void => {
  if (!Number.isSafeInteger(cols) || !Number.isSafeInteger(rows) || cols < 1 || rows < 1 || cols > 1000 || rows > 1000) throw new Error('Invalid terminal dimensions');
};
const bytes = (value: unknown): Buffer => {
  if (!Array.isArray(value) || value.length > 64 * 1024 || !value.every(byte => Number.isInteger(byte) && byte >= 0 && byte <= 255)) throw new Error('Expected terminal input bytes');
  return Buffer.from(value);
};
export class Terminals {
  private readonly attachments = new Map<string, TerminalConnection>();
  private closing = false;
  private readonly pending = new Map<string, Set<Promise<unknown>>>();
  constructor(private readonly hive: HiveHost, private readonly available: (workspaceId: string) => void, private readonly emit: (event: TerminalEvent) => void) {}
  async list(workspaceId: string): Promise<{ runs: ShellRun[] }> {
    if (!workspaceId.trim()) throw new Error('workspaceId is required');
    const runs = await this.hive.request('GET', `/api/ui/workspaces/${encodeURIComponent(workspaceId)}/runs`) as { run_id: string; agent_id: string; agent_name: string; status: string }[];
    if (!Array.isArray(runs)) throw new Error('Invalid workspace terminal list');
    return { runs: runs.filter(run => run.agent_id === `${workspaceId}:shell` && ['starting', 'running'].includes(run.status)).map(run => {
      if (typeof run.run_id !== 'string' || typeof run.agent_name !== 'string') throw new Error('Invalid workspace shell run');
      return { runId: run.run_id, name: run.agent_name, status: run.status as ShellRun['status'], exitCode: null };
    }) };
  }
  start(workspaceId: string): Promise<ShellRun> { return this.track(workspaceId, () => this.startShell(workspaceId)); }
  private async startShell(workspaceId: string): Promise<ShellRun> {
    this.check(workspaceId);
    const run = await this.hive.request('POST', `/api/workspaces/${encodeURIComponent(workspaceId)}/shell/start`) as { run_id: string; agent_id: string; agent_name: string; status: string };
    if (run.agent_id !== `${workspaceId}:shell` || typeof run.run_id !== 'string' || typeof run.agent_name !== 'string' || !['starting', 'running'].includes(run.status)) throw new Error('Hive returned an invalid workspace shell');
    this.check(workspaceId);
    return { runId: run.run_id, name: run.agent_name, status: run.status as ShellRun['status'], exitCode: null };
  }
  attach(input: TerminalAttachment): Promise<{ ok: true; attachmentId: string }> { return this.track(input.workspaceId, () => this.attachShell(input)); }
  private async attachShell(input: TerminalAttachment): Promise<{ ok: true; attachmentId: string }> {
    this.check(input.workspaceId); size(input.cols, input.rows);
    if (!/^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(input.attachmentId) || this.attachments.has(input.attachmentId)) throw new Error('Invalid or duplicate terminal attachment identity');
    if (!(await this.list(input.workspaceId)).runs.some(run => run.runId === input.runId)) throw new Error('Shell run does not belong to this workspace');
    this.check(input.workspaceId);
    if (this.attachments.has(input.attachmentId)) throw new Error('Duplicate terminal attachment identity');
    const connection = new TerminalConnection(input, this.hive, this.emit, () => { if (this.attachments.get(input.attachmentId) === connection) this.attachments.delete(input.attachmentId); });
    this.attachments.set(input.attachmentId, connection);
    try { await connection.connect(); this.check(input.workspaceId); }
    catch (error) { await connection.close('attach-failed'); throw error; }
    return { ok: true, attachmentId: input.attachmentId };
  }
  private track<T>(workspaceId: string, operation: () => Promise<T>): Promise<T> {
    let pending = this.pending.get(workspaceId);
    if (!pending) { pending = new Set(); this.pending.set(workspaceId, pending); }
    const task = operation().finally(() => { pending.delete(task); if (!pending.size) this.pending.delete(workspaceId); });
    pending.add(task); return task;
  }
  private check(workspaceId: string): void { if (this.closing) throw new Error('Terminal runtime is closing'); this.available(workspaceId); }
  private require(attachmentId: string): TerminalConnection {
    const connection = this.attachments.get(attachmentId); if (!connection) throw new Error('Terminal attachment is no longer active'); return connection;
  }
  restoreComplete(input: { attachmentId: string; sequence: number }) { return this.require(input.attachmentId).restoreComplete(input.sequence); }
  ack(input: { attachmentId: string; sequence: number; bytes: number }) { return this.require(input.attachmentId).ack(input.sequence, input.bytes); }
  input(input: { attachmentId: string; bytes: unknown; userInput?: boolean }) {
    const connection = this.require(input.attachmentId); this.check(connection.attachment.workspaceId);
    if (input.userInput !== undefined && typeof input.userInput !== 'boolean') throw new Error('Invalid terminal input origin');
    return connection.input(bytes(input.bytes), input.userInput ?? true);
  }
  resize(input: { attachmentId: string; cols: number; rows: number }) {
    const connection = this.require(input.attachmentId); this.check(connection.attachment.workspaceId); size(input.cols, input.rows);
    return connection.resize(input.cols, input.rows);
  }
  async detach(input: { attachmentId: string }): Promise<{ ok: true }> { await this.attachments.get(input.attachmentId)?.close('detached'); return { ok: true }; }
  async stop(input: { workspaceId: string; runId: string }): Promise<{ ok: true }> {
    const known = [...this.attachments.values()].some(connection => connection.attachment.workspaceId === input.workspaceId && connection.attachment.runId === input.runId);
    if (!known && !(await this.list(input.workspaceId)).runs.some(run => run.runId === input.runId)) throw new Error('Shell run does not belong to this workspace');
    await this.hive.request('DELETE', `/api/workspaces/${encodeURIComponent(input.workspaceId)}/shell/${encodeURIComponent(input.runId)}`);
    await Promise.all([...this.attachments.values()].filter(connection => connection.attachment.workspaceId === input.workspaceId && connection.attachment.runId === input.runId).map(connection => connection.close('stopped')));
    return { ok: true };
  }
  async detachWorkspace(workspaceId: string): Promise<void> {
    await Promise.allSettled(this.pending.get(workspaceId) ?? []);
    await Promise.all([...this.attachments.values()].filter(connection => connection.attachment.workspaceId === workspaceId).map(connection => connection.close('workspace-removed')));
  }
  async close(): Promise<void> { this.closing = true; await Promise.allSettled([...this.pending.values()].flatMap(tasks => [...tasks])); await Promise.all([...this.attachments.values()].map(connection => connection.close('runtime-closed'))); }
}

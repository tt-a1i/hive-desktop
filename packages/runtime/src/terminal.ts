import type { HiveHost } from './hive-host.js';
import { TerminalConnection, type TerminalAttachment, type TerminalEvent } from './terminal-connection.js';
export type { TerminalEvent } from './terminal-connection.js';
export interface ShellRun { runId: string; sessionId?: string; name: string; status: 'starting' | 'running'; exitCode: null }
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
  constructor(private readonly hive: HiveHost, private readonly available: (workspaceId: string) => void, private readonly emit: (event: TerminalEvent) => void, private readonly executionDirectory?: (workspaceId: string, sessionId: string) => Promise<string>) {}
  async list(workspaceId: string, sessionId?: string): Promise<{ workspaceId: string; sessionId?: string; runs: ShellRun[]; sourceUnavailable?: string }> {
    this.validateSessionId(sessionId);
    if (!workspaceId.trim()) throw new Error('workspaceId is required');
    let sourceFailure: {error: unknown} | undefined;
    if (sessionId !== undefined) {
      this.check(workspaceId);
      if (!this.executionDirectory) throw new Error('Session terminal execution directory resolver is unavailable');
      try { await this.executionDirectory(workspaceId, sessionId); }
      catch (error) { sourceFailure = {error}; }
      this.check(workspaceId);
    }
    const runs = await this.ownedRuns(workspaceId, sessionId);
    if (sourceFailure && !runs.length) throw sourceFailure.error;
    return {workspaceId, ...(sessionId !== undefined && {sessionId}), runs, ...(sourceFailure && {sourceUnavailable: sourceFailure.error instanceof Error ? sourceFailure.error.message : String(sourceFailure.error)})};
  }
  /** Run ownership comes from Hive's live shell registry, independent of source availability. */
  private async ownedRuns(workspaceId: string, sessionId?: string): Promise<ShellRun[]> {
    if (!workspaceId.trim()) throw new Error('workspaceId is required');
    const runs = await this.hive.request('GET', `/api/ui/workspaces/${encodeURIComponent(workspaceId)}/runs`) as { run_id: string; agent_id: string; agent_name: string; status: string; sessionId?: string }[];
    if (!Array.isArray(runs)) throw new Error('Invalid workspace terminal list');
    return runs.filter(run => run.agent_id === this.agentId(workspaceId, sessionId) && run.sessionId === sessionId && ['starting', 'running'].includes(run.status)).map(run => {
      if (typeof run.run_id !== 'string' || typeof run.agent_name !== 'string') throw new Error('Invalid workspace shell run');
      return { runId: run.run_id, ...(sessionId !== undefined && {sessionId}), name: run.agent_name, status: run.status as ShellRun['status'], exitCode: null };
    });
  }
  start(workspaceId: string, sessionId?: string): Promise<ShellRun> { return this.track(workspaceId, () => this.startShell(workspaceId, sessionId)); }
  private async startShell(workspaceId: string, sessionId?: string): Promise<ShellRun> {
    this.check(workspaceId); this.validateSessionId(sessionId);
    let sessionRun: {runId: string} | undefined;
    if (sessionId !== undefined) {
      if (!this.executionDirectory) throw new Error("Session terminal execution directory resolver is unavailable");
      const cwd = await this.executionDirectory(workspaceId, sessionId);
      this.check(workspaceId);
      sessionRun = await this.hive.startSessionShell(workspaceId, {sessionId, cwd});
    }
    if (sessionRun) {
      try {
        const run = (await this.ownedRuns(workspaceId, sessionId)).find(run => run.runId === sessionRun.runId);
        if (!run) throw new Error('Hive returned an invalid session shell');
        this.check(workspaceId);
        return run;
      } catch (error) {
        try { await this.hive.request('DELETE', `/api/workspaces/${encodeURIComponent(workspaceId)}/shell/${encodeURIComponent(sessionRun.runId)}`); }
        catch (cleanupError) { throw new AggregateError([error, cleanupError], 'Session terminal startup failed and shell cleanup could not be confirmed'); }
        throw error;
      }
    }
    const run = await this.hive.request('POST', `/api/workspaces/${encodeURIComponent(workspaceId)}/shell/start`) as { run_id: string; agent_id: string; agent_name: string; status: string };
    if (run.agent_id !== `${workspaceId}:shell` || typeof run.run_id !== 'string' || typeof run.agent_name !== 'string' || !['starting', 'running'].includes(run.status)) throw new Error('Hive returned an invalid workspace shell');
    this.check(workspaceId);
    return { runId: run.run_id, name: run.agent_name, status: run.status as ShellRun['status'], exitCode: null };
  }
  attach(input: TerminalAttachment): Promise<{ ok: true; attachmentId: string }> { return this.track(input.workspaceId, () => this.attachShell(input)); }
  private async attachShell(input: TerminalAttachment): Promise<{ ok: true; attachmentId: string }> {
    this.check(input.workspaceId); size(input.cols, input.rows);
    if (!/^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(input.attachmentId) || this.attachments.has(input.attachmentId)) throw new Error('Invalid or duplicate terminal attachment identity');
    const listed = await this.list(input.workspaceId, input.sessionId);
    if (listed.sourceUnavailable !== undefined) throw new Error(listed.sourceUnavailable);
    if (!listed.runs.some(run => run.runId === input.runId)) throw new Error('Shell run does not belong to this workspace and session');
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
  private validateSessionId(sessionId?: string): void { if (sessionId !== undefined && (typeof sessionId !== "string" || !sessionId.trim())) throw new Error("Invalid session ID"); }
  private agentId(workspaceId: string, sessionId?: string): string { return sessionId === undefined ? `${workspaceId}:shell` : `${workspaceId}:session:${sessionId}:shell`; }
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
  async stop(input: { workspaceId: string; sessionId?: string; runId: string }): Promise<{ ok: true }> {
    this.validateSessionId(input.sessionId);
    const known = [...this.attachments.values()].some(connection => connection.attachment.workspaceId === input.workspaceId && connection.attachment.sessionId === input.sessionId && connection.attachment.runId === input.runId);
    if (!known && !(await this.ownedRuns(input.workspaceId, input.sessionId)).some(run => run.runId === input.runId)) throw new Error('Shell run does not belong to this workspace and session');
    await this.hive.request('DELETE', `/api/workspaces/${encodeURIComponent(input.workspaceId)}/shell/${encodeURIComponent(input.runId)}`);
    await this.closeConnections([...this.attachments.values()].filter(connection => connection.attachment.workspaceId === input.workspaceId && connection.attachment.sessionId === input.sessionId && connection.attachment.runId === input.runId), 'stopped');
    return { ok: true };
  }
  async detachWorkspace(workspaceId: string): Promise<void> {
    await Promise.allSettled(this.pending.get(workspaceId) ?? []);
    await this.closeConnections([...this.attachments.values()].filter(connection => connection.attachment.workspaceId === workspaceId), 'workspace-removed');
  }
  async close(): Promise<void> {
    this.closing = true;
    await Promise.allSettled([...this.pending.values()].flatMap(tasks => [...tasks]));
    await this.closeConnections([...this.attachments.values()], 'runtime-closed');
  }
  private async closeConnections(connections: TerminalConnection[], reason: string): Promise<void> {
    const results = await Promise.allSettled(connections.map(connection => connection.close(reason)));
    const errors = results.flatMap(result => result.status === 'rejected' ? [result.reason] : []);
    if (errors.length === 1) throw errors[0];
    if (errors.length > 1) throw new AggregateError(errors, 'Terminal connection close failed');
  }
}

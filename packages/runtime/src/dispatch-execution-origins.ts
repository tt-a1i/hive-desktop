import { createHash, randomUUID } from 'node:crypto';
import { constants, closeSync, fstatSync, linkSync, lstatSync, mkdirSync, openSync, readSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

/** Source identity only. Hive remains the dispatch/task authority. */
export interface DispatchExecutionOrigin {
  workspaceId: string;
  dispatchId: string;
  sourceSessionId: string;
  sourceAgentId: string;
}
export class DispatchOriginError extends Error {
  constructor(readonly code: string, message: string, options?: ErrorOptions) {
    super(`[${code}] ${message}`, options); this.name = 'DispatchOriginError';
  }
}
function valid(value: unknown): value is string { return typeof value === 'string' && !!value && value.length <= 4096 && !value.includes('\0'); }
function origin(value: unknown): value is DispatchExecutionOrigin {
  return !!value && typeof value === 'object' && !Array.isArray(value)
    && ['workspaceId', 'dispatchId', 'sourceSessionId', 'sourceAgentId'].every(key => valid((value as Record<string, unknown>)[key]));
}
function equal(a: DispatchExecutionOrigin, b: DispatchExecutionOrigin): boolean {
  return a.workspaceId === b.workspaceId && a.dispatchId === b.dispatchId
    && a.sourceSessionId === b.sourceSessionId && a.sourceAgentId === b.sourceAgentId;
}
/** Immutable per-dispatch files. Atomic no-clobber publication also handles two owners racing. */
export class DispatchExecutionOrigins {
  constructor(private readonly directory: string) {}
  private path(workspaceId: string, dispatchId: string): string {
    if (!valid(workspaceId) || !valid(dispatchId)) throw new DispatchOriginError('DISPATCH_ORIGIN_INVALID', 'Workspace and dispatch identities are required');
    return join(this.directory, `${createHash('sha256').update(JSON.stringify([workspaceId, dispatchId])).digest('hex')}.json`);
  }
  get(workspaceId: string, dispatchId: string): DispatchExecutionOrigin | undefined {
    const value = this.read(this.path(workspaceId, dispatchId));
    if (value && (value.workspaceId !== workspaceId || value.dispatchId !== dispatchId))
      throw new DispatchOriginError('DISPATCH_ORIGIN_UNREADABLE', 'Saved dispatch source identity does not match its key');
    return value;
  }
  /** Exact reverse provenance only; no current worker binding or timestamp inference. */
  sourceAgentForSession(workspaceId: string, sessionId: string): string | undefined {
    let files: string[];
    try { files = readdirSync(this.directory); }
    catch (error) { if ((error as NodeJS.ErrnoException).code === 'ENOENT') return undefined; throw new DispatchOriginError('DISPATCH_ORIGIN_UNREADABLE', 'Dispatch source records cannot be read', {cause: error}); }
    const agents = new Set<string>();
    for (const file of files) {
      if (!/^[a-f0-9]{64}\.json$/.test(file)) continue;
      const value = this.read(join(this.directory, file));
      if (!value) throw new DispatchOriginError('DISPATCH_ORIGIN_UNREADABLE', 'Dispatch source record disappeared while reading');
      if (this.path(value.workspaceId, value.dispatchId) !== join(this.directory, file)) throw new DispatchOriginError('DISPATCH_ORIGIN_UNREADABLE', 'Dispatch source record has an invalid key');
      if (value.workspaceId === workspaceId && value.sourceSessionId === sessionId) agents.add(value.sourceAgentId);
    }
    if (agents.size > 1) throw new DispatchOriginError('DISPATCH_ORIGIN_CONFLICT', 'Saved dispatch sources disagree about the original agent for this conversation');
    return agents.values().next().value;
  }
  private read(path: string): DispatchExecutionOrigin | undefined {
    let fd: number;
    try {
      const directory = lstatSync(this.directory);
      if (!directory.isDirectory() || directory.isSymbolicLink()) throw new Error('Invalid source directory');
      fd = openSync(path, constants.O_RDONLY | (constants.O_NOFOLLOW ?? 0));
    }
    catch (error) {
      if ((error as NodeJS.ErrnoException).code === 'ENOENT') return undefined;
      throw new DispatchOriginError('DISPATCH_ORIGIN_UNREADABLE', 'Saved dispatch source cannot be read; restore its metadata before opening this artifact', {cause: error});
    }
    try {
      const info = fstatSync(fd);
      if (!info.isFile() || info.size > 32768) throw new Error('Invalid source file');
      const bytes = Buffer.alloc(32769); let length = 0;
      while (length < bytes.length) { const count = readSync(fd, bytes, length, bytes.length - length, null); if (!count) break; length += count; }
      if (length > 32768 || length !== info.size) throw new Error('Source file changed');
      const value: unknown = JSON.parse(bytes.subarray(0, length).toString('utf8'));
      if (!value || typeof value !== 'object' || !('version' in value) || value.version !== 1 || !('origin' in value) || !origin(value.origin)) throw new Error('Invalid source record');
      const {workspaceId: storedWorkspaceId, dispatchId: storedDispatchId, sourceSessionId, sourceAgentId} = value.origin;
      return {workspaceId: storedWorkspaceId, dispatchId: storedDispatchId, sourceSessionId, sourceAgentId};
    } catch (error) {
      throw new DispatchOriginError('DISPATCH_ORIGIN_UNREADABLE', 'Saved dispatch source is invalid; restore its metadata before opening this artifact', {cause: error});
    } finally { closeSync(fd); }
  }
  put(value: DispatchExecutionOrigin): void {
    if (!origin(value)) throw new DispatchOriginError('DISPATCH_ORIGIN_INVALID', 'Dispatch source identity is incomplete');
    const path = this.path(value.workspaceId, value.dispatchId);
    const existing = this.get(value.workspaceId, value.dispatchId);
    if (existing) {
      if (!equal(existing, value)) throw new DispatchOriginError('DISPATCH_ORIGIN_CONFLICT', 'This dispatch belongs to another execution session; its historical source cannot be rebound');
      return;
    }
    const temporary = join(this.directory, `.${randomUUID()}.tmp`);
    const {workspaceId, dispatchId, sourceSessionId, sourceAgentId} = value;
    const payload = JSON.stringify({version: 1, origin: {workspaceId, dispatchId, sourceSessionId, sourceAgentId}});
    if (Buffer.byteLength(payload, 'utf8') > 32768) throw new DispatchOriginError('DISPATCH_ORIGIN_INVALID', 'Dispatch source identity exceeds the metadata limit');
    try {
      mkdirSync(this.directory, {recursive: true, mode: 0o700});
      const info = lstatSync(this.directory);
      if (!info.isDirectory() || info.isSymbolicLink()) throw new Error('Invalid source directory');
      writeFileSync(temporary, payload, {flag: 'wx', mode: 0o600, flush: true});
      try { linkSync(temporary, path); }
      catch (error) {
        if ((error as NodeJS.ErrnoException).code !== 'EEXIST') throw error;
        const accepted = this.get(value.workspaceId, value.dispatchId);
        if (!accepted || !equal(accepted, value)) throw new DispatchOriginError('DISPATCH_ORIGIN_CONFLICT', 'This dispatch belongs to another execution session; its historical source cannot be rebound');
      }
    } catch (error) {
      if (error instanceof DispatchOriginError) throw error;
      throw new DispatchOriginError('DISPATCH_ORIGIN_WRITE_FAILED', 'Dispatch source could not be saved; delivery was not accepted. Check disk access and retry in the original session', {cause: error});
    } finally {
      // A retained private scratch link is harmless; it must not hide the
      // actionable publication error or undo an already published source.
      try { rmSync(temporary, {force: true}); } catch { /* best effort scratch cleanup */ }
    }
  }
}

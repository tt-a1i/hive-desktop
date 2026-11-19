import type { ExecutionMutations } from './execution-mutations.js';
import { createHash, randomUUID } from 'node:crypto';
import { closeSync, constants, existsSync, fstatSync, fsyncSync, linkSync, lstatSync, mkdirSync, openSync, readFileSync, readSync, readdirSync, renameSync, statSync, unlinkSync, writeFileSync, writeSync } from 'node:fs';
import { realpath } from 'node:fs/promises';
import { join } from 'node:path';
import type { HiveHost } from './hive-host.js';
import type { ProjectScript, ProjectScriptSettings } from './project-script-settings.js';
export type ScriptStatus = 'starting' | 'running' | 'succeeded' | 'failed' | 'cancelled' | 'interrupted' | 'unknown';
export interface ProjectScriptRun {
  version: 1; runId: string; requestId: string; workspaceId: string; sessionId?: string;
  cwd: string; script: ProjectScript; status: ScriptStatus; startedAt: number; finishedAt?: number;
  nativeRunId?: string; signal?: number; exitCode: number | null; cancelRequested: boolean; logBytes: number; logTruncated: boolean;
  errorCode?: string;
}
export type ScriptEvent = {type: 'status'; run: ProjectScriptRun} | {type: 'output'; workspaceId: string; runId: string; offset: number; length: number};
export class ProjectScriptRunError extends Error {
  readonly wireCode: string;
  constructor(readonly code: string, message: string) {
    const wireCode = code.startsWith('SCRIPT_RUN_') ? code : code.replace(/^SCRIPT_/, 'SCRIPT_RUN_');
    super(`[${wireCode}] ${message}`); this.wireCode = wireCode; this.name = 'ProjectScriptRunError';
  }
}
const fail = (code: string, message: string): never => { throw new ProjectScriptRunError(code, message); };
const id = (value: unknown): value is string => typeof value === 'string' && /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(value);
const workspaceId = (value: unknown): value is string => typeof value === 'string' && !!value && value.length <= 256 && !/[\x00-\x1f\x7f]/.test(value);
const active = (run: ProjectScriptRun) => run.status === 'starting' || run.status === 'running';
const LIMIT = 16 * 1024 * 1024;
const RECEIPT_FILE = /^[0-9a-f]{64}\.json$/;
type RequestState = 'absent' | 'present' | 'unreadable';
interface Live {run: ProjectScriptRun; file: string; fd: number; exited: Promise<void>; resolve: () => void; checkpointBytes: number; checkpointAt: number; persistenceError?: boolean; releaseMutation?: () => void}
/** Captured run manifests are independent of editable settings. A request is durably claimed before spawning. */
export class ProjectScriptRuns {
  private readonly directory: string;
  private readonly live = new Map<string, Live>();
  private readonly undurable = new Map<string, ProjectScriptRun>();
  private readonly pending = new Map<string, Set<Promise<unknown>>>();
  private closing = false;
  constructor(private readonly hive: HiveHost, private readonly settings: ProjectScriptSettings, directory: string,
    private readonly available: (workspaceId: string) => void,
    private readonly executionDirectory: (workspaceId: string, sessionId?: string) => Promise<string>,
    private readonly emit: (event: ScriptEvent) => void = () => {}, private readonly mutations?: ExecutionMutations) {
    this.directory = join(directory, 'project-script-runs'); mkdirSync(this.directory, {recursive: true, mode: 0o700});
    for (const name of readdirSync(this.directory)) if (RECEIPT_FILE.test(name)) {
      const file = join(this.directory, name);
      let run: ProjectScriptRun;
      try { run = this.read(file); }
      catch (error) {
        // Leave the unreadable receipt in place to block replay of its request.
        if (error instanceof ProjectScriptRunError && error.code === 'SCRIPT_RUN_CORRUPT') continue;
        throw error;
      }
      if (!active(run)) continue;
      const log = statSync(this.logFile(run.runId), {throwIfNoEntry: false});
      this.persist(file, {...run, status: 'interrupted', finishedAt: Date.now(), logBytes: Math.min(LIMIT, log?.size ?? 0),
        errorCode: log ? 'SCRIPT_RUNTIME_INTERRUPTED' : 'SCRIPT_LOG_MISSING'});
    }
  }
  private requestFile(workspace: string, requestId: string) { return join(this.directory, `${createHash('sha256').update(JSON.stringify([workspace, requestId])).digest('hex')}.json`); }
  private logFile(runId: string) { return join(this.directory, `${runId}.log`); }
  private read(file: string): ProjectScriptRun {
    try {
      const fd = openSync(file, constants.O_RDONLY | (constants.O_NOFOLLOW ?? 0));
      let run: ProjectScriptRun;
      try { if (fstatSync(fd).size > 128 * 1024) throw 0; run = JSON.parse(readFileSync(fd, 'utf8')) as ProjectScriptRun; }
      finally { closeSync(fd); }
      if (run.version !== 1 || !id(run.runId) || !id(run.requestId) || !workspaceId(run.workspaceId)
        || run.sessionId !== undefined && (typeof run.sessionId !== 'string' || !run.sessionId)
        || typeof run.cwd !== 'string' || !run.cwd || !run.script || !id(run.script.id) || !id(run.script.version)
        || typeof run.script.name !== 'string' || typeof run.script.command !== 'string'
        || !['starting','running','succeeded','failed','cancelled','interrupted','unknown'].includes(run.status)
        || run.signal !== undefined && (!Number.isSafeInteger(run.signal) || run.signal <= 0)
        || typeof run.cancelRequested !== 'boolean' || !Number.isSafeInteger(run.logBytes) || run.logBytes < 0 || run.logBytes > LIMIT
        || typeof run.logTruncated !== 'boolean' || !(run.exitCode === null || Number.isInteger(run.exitCode))
        || typeof run.startedAt !== 'number' || this.requestFile(run.workspaceId, run.requestId) !== file) throw 0;
      return run;
    } catch { return fail('SCRIPT_RUN_CORRUPT', 'Saved script run metadata is unreadable; it was not replaced or rerun'); }
  }
  private persist(file: string, run: ProjectScriptRun, exclusive = false) {
    const temporary = `${file}.${randomUUID()}.tmp`;
    try {
      writeFileSync(temporary, JSON.stringify(run), {mode: 0o600, flush: true});
      if (exclusive) linkSync(temporary, file); else renameSync(temporary, file);
    } finally { if (existsSync(temporary)) unlinkSync(temporary); }
  }
  private event(event: ScriptEvent) { try { this.emit(event); } catch { /* A disconnected viewer cannot change process ownership. */ } }
  private save(live: Live, notify = true) {
    try { this.persist(live.file, live.run); }
    catch { live.persistenceError = true; fail('SCRIPT_RUN_WRITE_FAILED', 'Script run state could not be saved; inspect its retained log before starting another run'); }
    if (notify) this.event({type: 'status', run: structuredClone(live.run)});
  }
  private check(workspace: string) { if (this.closing) fail('SCRIPT_RUNTIME_CLOSING', 'Script runtime is closing'); this.available(workspace); }
  run(input: unknown): Promise<ProjectScriptRun> {
    if (!input || typeof input !== 'object' || Array.isArray(input)) return Promise.reject(new ProjectScriptRunError('SCRIPT_INPUT_INVALID', 'Invalid script run request'));
    const value = input as Record<string, unknown>;
    if (Object.keys(value).some(key => !['workspaceId','scriptId','expectedVersion','sessionId','requestId'].includes(key))
      || !workspaceId(value.workspaceId) || !id(value.scriptId) || !id(value.expectedVersion) || !id(value.requestId)
      || value.sessionId !== undefined && (typeof value.sessionId !== 'string' || !value.sessionId || value.sessionId.length > 256))
      return Promise.reject(new ProjectScriptRunError('SCRIPT_INPUT_INVALID', 'Supply only a saved script, expected version, source session and unique request identity'));
    const workspace = value.workspaceId;
    let tasks = this.pending.get(workspace); if (!tasks) { tasks = new Set(); this.pending.set(workspace, tasks); }
    const release = this.mutations?.begin(workspace);
    const task = this.start(value as {workspaceId: string; scriptId: string; expectedVersion: string; sessionId?: string; requestId: string})
      .finally(() => { release?.(); tasks.delete(task); if (!tasks.size) this.pending.delete(workspace); });
    tasks.add(task); return task;
  }
  /** Internal coordinator entry; never exposed as an RPC accepting a command. */
  runCaptured(workspaceId: string, sessionId: string, requestId: string, script: ProjectScript, onExit: () => void): Promise<ProjectScriptRun> {
    let tasks = this.pending.get(workspaceId); if (!tasks) {tasks = new Set(); this.pending.set(workspaceId, tasks);}
    const release = this.mutations?.begin(workspaceId);
    const task = this.start({workspaceId, sessionId, requestId, scriptId: script.id, expectedVersion: script.version}, script, onExit)
      .finally(() => {release?.(); tasks.delete(task); if (!tasks.size) this.pending.delete(workspaceId);});
    tasks.add(task); return task;
  }
  private async start(input: {workspaceId: string; scriptId: string; expectedVersion: string; sessionId?: string; requestId: string}, captured?: ProjectScript, onCapturedExit?: () => void): Promise<ProjectScriptRun> {
    this.check(input.workspaceId);
    const file = this.requestFile(input.workspaceId, input.requestId);
    const duplicate = () => {
      const run = this.read(file);
      if (run.script.id !== input.scriptId || run.script.version !== input.expectedVersion || run.sessionId !== input.sessionId)
        fail('SCRIPT_REQUEST_CONFLICT', 'This request identity already belongs to a different script run');
      return structuredClone(run);
    };
    if (existsSync(file)) return duplicate();
    const script = captured ?? await this.settings.capture(input.workspaceId, input.scriptId, input.expectedVersion);
    const cwd = await realpath(await this.executionDirectory(input.workspaceId, input.sessionId));
    if (!statSync(cwd).isDirectory()) fail('SCRIPT_SOURCE_UNAVAILABLE', 'The selected execution directory is unavailable');
    this.check(input.workspaceId);
    const run: ProjectScriptRun = {version: 1, runId: randomUUID(), requestId: input.requestId, workspaceId: input.workspaceId,
      ...(input.sessionId !== undefined && {sessionId: input.sessionId}), cwd, script, status: 'starting', startedAt: Date.now(), exitCode: null,
      cancelRequested: false, logBytes: 0, logTruncated: false};
    const fd = openSync(this.logFile(run.runId), 'wx', 0o600);
    try { this.persist(file, run, true); }
    catch (error) { closeSync(fd); unlinkSync(this.logFile(run.runId)); if ((error as NodeJS.ErrnoException).code === 'EEXIST') return duplicate(); throw new ProjectScriptRunError('SCRIPT_RUN_WRITE_FAILED', 'Script run could not be recorded; no command was started'); }
    let resolve!: () => void;
    const live: Live = {run, file, fd, exited: new Promise<void>(done => {resolve = done;}), resolve: () => resolve(), checkpointBytes: 0, checkpointAt: Date.now()};
    if (this.mutations) live.releaseMutation = this.mutations.begin(input.workspaceId);
    this.live.set(run.runId, live); this.event({type: 'status', run: structuredClone(run)});
    let nativeStarted = false;
    try {
      await this.hive.startSavedScript(input.workspaceId, {scriptRunId: run.runId, cwd, command: script.command, ...(input.sessionId !== undefined && {sessionId: input.sessionId})}, {
        onCreated: nativeRunId => {run.nativeRunId = nativeRunId; this.save(live);},
        onOutput: chunk => {
          try {
            if (run.logBytes === LIMIT && run.logTruncated) return;
            const bytes = Buffer.from(chunk), offset = run.logBytes, retained = bytes.subarray(0, LIMIT - offset);
            const changed = run.status === 'starting' || !run.logTruncated && retained.length < bytes.length;
            if (retained.length) { let written = 0; while (written < retained.length) written += writeSync(fd, retained, written); run.logBytes += retained.length; }
            if (retained.length < bytes.length) run.logTruncated = true;
            if (active(run)) run.status = 'running';
            // Bound synchronous disk work; every terminal transition flushes the log.
            // Cold recovery measures retained bytes even if a process dies before this checkpoint.
            if (changed || run.logBytes - live.checkpointBytes >= 256 * 1024 || Date.now() - live.checkpointAt >= 1000) {
              fsyncSync(fd); this.save(live, changed); live.checkpointBytes = run.logBytes; live.checkpointAt = Date.now();
            }
            if (retained.length) this.event({type: 'output', workspaceId: run.workspaceId, runId: run.runId, offset, length: retained.length});
          } catch {
            live.persistenceError = true; run.errorCode = 'SCRIPT_LOG_WRITE_FAILED';
            try {if (run.nativeRunId) this.hive.stopSavedScript(run.workspaceId, run.runId, run.nativeRunId);} catch {run.errorCode = 'SCRIPT_STOP_UNCONFIRMED';}
          }
        },
        onExit: (exitCode, signal) => {
          if (signal !== undefined && signal !== 0) run.signal = signal;
          if (run.signal && !run.cancelRequested) run.errorCode ??= 'SCRIPT_SIGNALLED';
          run.exitCode = exitCode; run.finishedAt = Date.now();
          run.status = live.persistenceError || exitCode === null ? 'unknown' : run.cancelRequested ? 'cancelled' : run.signal ? 'failed' : exitCode === 0 ? 'succeeded' : 'failed';
          try {fsyncSync(fd);} catch {live.persistenceError = true; run.status = 'unknown'; run.errorCode ??= 'SCRIPT_LOG_WRITE_FAILED';}
          try {closeSync(fd);} catch {live.persistenceError = true; run.status = 'unknown'; run.errorCode ??= 'SCRIPT_LOG_WRITE_FAILED';}
          try {this.save(live);} catch {live.persistenceError = true;}
          if (live.persistenceError) {run.status = 'unknown'; run.errorCode ??= 'SCRIPT_RUN_WRITE_FAILED'; this.undurable.set(run.runId, structuredClone(run)); this.event({type: 'status', run: structuredClone(run)});}
          this.live.delete(run.runId); live.releaseMutation?.(); live.resolve();
          onCapturedExit?.();
        },
      });
      nativeStarted = true;
      if (active(run)) {run.status = 'running'; this.save(live);}
    } catch {
      // Preserve ownership when a persistence failure follows a successful spawn.
      if (nativeStarted && active(run)) {
        try {await this.stop(run.workspaceId, run.runId);} catch { /* stop retains ownership until actual exit */ }
        fail('SCRIPT_RUN_WRITE_FAILED', 'Script state could not be saved after startup; automatic retry is disabled');
      }
      if (active(run)) {
        run.status = 'failed'; run.errorCode = 'SCRIPT_START_FAILED'; run.finishedAt = Date.now();
        try { closeSync(fd); this.save(live); } finally {this.live.delete(run.runId); live.releaseMutation?.(); live.resolve();}
      }
    }
    if (live.persistenceError) fail('SCRIPT_RUN_WRITE_FAILED', 'Script process state could not be durably recorded; inspect this run before retrying');
    return structuredClone(run);
  }
  /** Internal initialization lookup keeps trusted live ownership visible if its manifest disappears. */
  capturedRequest(workspace: string, requestId: string): ProjectScriptRun | undefined {
    const live = [...this.live.values()].find(value => value.run.workspaceId === workspace && value.run.requestId === requestId);
    if (live) return structuredClone(live.run);
    const undurable = [...this.undurable.values()].find(run => run.workspaceId === workspace && run.requestId === requestId);
    if (undurable) return structuredClone(undurable);
    const file = this.requestFile(workspace, requestId);
    return lstatSync(file, {throwIfNoEntry: false}) ? this.read(file) : undefined;
  }
  private requestState(workspace: string, requestId: string): RequestState {
    try { return this.capturedRequest(workspace, requestId) ? 'present' : 'absent'; }
    catch (error) {
      if (error instanceof ProjectScriptRunError && error.code === 'SCRIPT_RUN_CORRUPT') return 'unreadable';
      throw error;
    }
  }
  list(workspace: string, requestId?: unknown): {runs: ProjectScriptRun[]; unreadableRunCount: number; requestState?: RequestState} {
    if (!workspaceId(workspace)) fail('SCRIPT_INPUT_INVALID', 'Invalid workspace identity');
    if (requestId !== undefined && !id(requestId)) return fail('SCRIPT_INPUT_INVALID', 'Invalid script request identity');
    const runs: ProjectScriptRun[] = [];
    let unreadableRunCount = 0;
    for (const name of readdirSync(this.directory)) {
      if (!RECEIPT_FILE.test(name)) continue;
      let run: ProjectScriptRun;
      try {run = this.read(join(this.directory, name));}
      catch (error) {
        if (!(error instanceof ProjectScriptRunError) || error.code !== 'SCRIPT_RUN_CORRUPT') throw error;
        // The receipt cannot establish a workspace, so report it without attributing it to one.
        unreadableRunCount++;
        continue;
      }
      if (run.workspaceId === workspace) runs.push(structuredClone(this.live.get(run.runId)?.run ?? this.undurable.get(run.runId) ?? run));
    }
    return {runs: runs.sort((a,b) => b.startedAt - a.startedAt), unreadableRunCount,
      ...(requestId !== undefined && {requestState: this.requestState(workspace, requestId)})};
  }
  private owned(workspace: string, runId: string) {
    if (!workspaceId(workspace) || !id(runId)) fail('SCRIPT_INPUT_INVALID', 'Invalid script run identity');
    return this.list(workspace).runs.find(run => run.runId === runId) ?? fail('SCRIPT_RUN_NOT_FOUND', 'Script run was not found in this project');
  }
  get(workspace: string, runId: string, offset = 0, limit = 65536) {
    const run = this.owned(workspace, runId);
    if (!Number.isSafeInteger(offset) || offset < 0 || !Number.isSafeInteger(limit) || limit < 1 || limit > 65536) fail('SCRIPT_INPUT_INVALID', 'Invalid script log page');
    let fd: number;
    try {fd = openSync(this.logFile(run.runId), constants.O_RDONLY | (constants.O_NOFOLLOW ?? 0));}
    catch (error) {
      if ((error as NodeJS.ErrnoException).code === 'ENOENT') return fail('SCRIPT_LOG_MISSING', 'The saved log file for this script run is missing.');
      throw error;
    }
    try {
      const bytes = Buffer.alloc(Math.min(limit, Math.max(0, fstatSync(fd).size - offset)));
      const length = readSync(fd, bytes, 0, bytes.length, offset);
      return {run, output: {offset, nextOffset: offset + length, bytes: [...bytes.subarray(0, length)], truncated: run.logTruncated}};
    } finally {closeSync(fd);}
  }
  async stop(workspace: string, runId: string, requestId?: string): Promise<ProjectScriptRun> {
    if (!workspaceId(workspace) || !id(runId)) fail('SCRIPT_INPUT_INVALID', 'Invalid script run identity');
    const live = this.live.get(runId);
    if (live && (live.run.workspaceId !== workspace || requestId !== undefined && live.run.requestId !== requestId))
      fail('SCRIPT_RUN_NOT_FOUND', 'Script run was not found in this project');
    if (!live) {
      if (requestId === undefined) return this.owned(workspace, runId);
      const run = this.capturedRequest(workspace, requestId);
      return run?.runId === runId ? run : fail('SCRIPT_RUN_NOT_FOUND', 'Script run was not found in this project');
    }
    live.run.cancelRequested = true;
    let writeFailure: unknown;
    try {this.save(live);} catch (error) {writeFailure = error;}
    if (live.run.nativeRunId) this.hive.stopSavedScript(workspace, runId, live.run.nativeRunId);
    let timer: ReturnType<typeof setTimeout> | undefined;
    try {
      await Promise.race([live.exited, new Promise<never>((_, reject) => {timer = setTimeout(() => reject(new ProjectScriptRunError('SCRIPT_STOP_UNCONFIRMED', 'Native script process exit could not be confirmed')), 10000);})]);
    } finally {if (timer) clearTimeout(timer);}
    if (writeFailure) throw writeFailure;
    if (live.persistenceError) fail('SCRIPT_RUN_WRITE_FAILED', 'Script cancellation was requested but its durable state needs inspection');
    return structuredClone(live.run);
  }
  async stopWorkspace(workspace: string) {
    await Promise.allSettled(this.pending.get(workspace) ?? []);
    const results = await Promise.allSettled([...this.live.values()].filter(value => value.run.workspaceId === workspace).map(value => this.stop(workspace, value.run.runId)));
    const errors = results.flatMap(result => result.status === 'rejected' ? [result.reason] : []);
    if (errors.length) throw new AggregateError(errors, 'Project script shutdown could not be confirmed');
  }
  async close() {
    this.closing = true;
    await Promise.allSettled([...this.pending.values()].flatMap(tasks => [...tasks]));
    const results = await Promise.allSettled([...this.live.values()].map(value => this.stop(value.run.workspaceId, value.run.runId)));
    const errors = results.flatMap(result => result.status === 'rejected' ? [result.reason] : []);
    if (errors.length) throw new AggregateError(errors, 'Script runtime shutdown could not be confirmed');
  }
}

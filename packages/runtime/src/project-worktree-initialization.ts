import { createHash, randomUUID } from 'node:crypto';
import { constants, closeSync, existsSync, mkdirSync, openSync, readFileSync, renameSync, fstatSync, unlinkSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import type { ProjectScript, ProjectScriptSettings } from './project-script-settings.js';
import type { ProjectScriptRuns, ScriptStatus } from './project-script-runs.js';

export type InitializationPlan = {enabled: false} | {enabled: true; requestId: string; script: ProjectScript};
type Status = 'disabled' | 'pending' | 'running' | ScriptStatus | 'skipped';
interface State {schema: 1; workspaceId: string; sessionId: string; planHash: string; version: string; status: Status; requestId?: string; runId?: string | undefined; errorCode?: string | undefined}
const uuid = (v: unknown): v is string => typeof v === 'string' && /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(v);
export class WorktreeInitializationError extends Error {
  readonly code = 'WORKTREE_INITIALIZATION_BLOCKED';
  constructor(message: string) {super(message); this.name = 'WorktreeInitializationError';}
}
function fail(message: string): never {throw new WorktreeInitializationError(message);}
export function validateInitializationPlan(value: unknown): asserts value is InitializationPlan {
  if (!value || typeof value !== 'object' || Array.isArray(value)) fail('The saved setup plan is invalid; no command was run.');
  const plan = value as InitializationPlan;
  if (plan.enabled === false && Object.keys(plan).join(',') === 'enabled') return;
  if (plan.enabled !== true || Object.keys(plan).sort().join(',') !== 'enabled,requestId,script' || !uuid(plan.requestId)) fail('The saved setup plan is invalid.');
  const s = plan.script;
  if (!s || !uuid(s.id) || !uuid(s.version) || typeof s.name !== 'string' || !s.name.trim() || s.name.length > 120
    || typeof s.command !== 'string' || !s.command.trim() || s.command.length > 16384 || /[\x00-\x08\x0b\x0c\x0e-\x1f\x7f]/.test(s.command)
    || Object.keys(s).some(k => !['id','version','name','command','runOnWorktreeCreate'].includes(k)) || s.runOnWorktreeCreate !== true) fail('The original setup command is invalid.');
}
/** Stores only initialization state; script process ownership remains in ProjectScriptRuns. Cold reads never start a process. */
export class ProjectWorktreeInitialization {
  private readonly directory: string;
  private readonly locks = new Map<string, Promise<unknown>>();
  private closing = false;
  private readonly launching = new Set<string>();
  constructor(directory: string, private readonly settings: ProjectScriptSettings, private readonly runs: ProjectScriptRuns,
    private readonly source: (workspaceId: string, sessionId: string) => Promise<InitializationPlan | undefined>,
    private readonly available: (workspaceId: string) => void) {
    this.directory = join(directory, 'worktree-initialization'); mkdirSync(this.directory, {recursive: true, mode: 0o700});
  }
  async capture(workspaceId: string, skip = false, expectedRevision?: string): Promise<InitializationPlan> {
    this.check(workspaceId);
    const snapshot = await this.settings.list(workspaceId);
    if (expectedRevision !== undefined && snapshot.revision !== expectedRevision) fail('Project setup selection changed; reload before creating the worktree.');
    const script = snapshot.scripts.find(item => item.runOnWorktreeCreate);
    return !script || skip ? {enabled: false} : {enabled: true, requestId: randomUUID(), script: {...script}};
  }
  private check(workspaceId: string) {if (this.closing) fail('Worktree initialization is closing.'); this.available(workspaceId);}
  private file(workspaceId: string, sessionId: string) {return join(this.directory, `${createHash('sha256').update(JSON.stringify([workspaceId, sessionId])).digest('hex')}.json`);}
  private hash(plan: InitializationPlan) {return createHash('sha256').update(JSON.stringify(plan)).digest('hex');}
  private write(state: State) {
    const file = this.file(state.workspaceId, state.sessionId), temporary = `${file}.${randomUUID()}.tmp`;
    try {writeFileSync(temporary, JSON.stringify(state), {mode: 0o600, flush: true}); renameSync(temporary, file);}
    finally {if (existsSync(temporary)) unlinkSync(temporary);}
  }
  private save(state: State, change: Partial<State>): State {const next = {...state, ...change, version: randomUUID()}; this.write(next); return next;}
  private read(workspaceId: string, sessionId: string, plan: InitializationPlan): State {
    const file = this.file(workspaceId, sessionId);
    if (!existsSync(file)) {
      // Missing state is not evidence that a captured command has never run.
      const state: State = {schema: 1, workspaceId, sessionId, planHash: this.hash(plan), version: randomUUID(), status: plan.enabled ? 'interrupted' : 'disabled', ...(plan.enabled && {requestId: plan.requestId})};
      this.write(state); return state;
    }
    try {
      const fd = openSync(file, constants.O_RDONLY | (constants.O_NOFOLLOW ?? 0)); let state: State;
      try {if (fstatSync(fd).size > 16384) throw 0; state = JSON.parse(readFileSync(fd, 'utf8')) as State;} finally {closeSync(fd);}
      if (Object.keys(state).some(k => !['schema','workspaceId','sessionId','planHash','version','status','requestId','runId','errorCode'].includes(k)) || state.schema !== 1 || state.workspaceId !== workspaceId || state.sessionId !== sessionId || state.planHash !== this.hash(plan) || !uuid(state.version)
        || !['disabled','pending','running','succeeded','failed','cancelled','interrupted','unknown','skipped'].includes(state.status)
        || (plan.enabled ? !uuid(state.requestId) || state.status === 'disabled' : state.status !== 'disabled')
        || state.runId !== undefined && !uuid(state.runId)
        || state.errorCode !== undefined && (typeof state.errorCode !== 'string' || state.errorCode.length > 8192)
        || state.status === 'succeeded' && !uuid(state.runId)) throw 0;
      return state;
    } catch {return fail('Saved worktree initialization state is unreadable; it was not replaced or rerun.');}
  }
  private async current(workspaceId: string, sessionId: string) {
    this.check(workspaceId);
    const plan = await this.source(workspaceId, sessionId);
    if (!plan) return null;
    validateInitializationPlan(plan);
    let state = this.read(workspaceId, sessionId, plan);
    if (plan.enabled && state.status !== 'skipped') {
      const run = this.runs.capturedRequest(workspaceId, state.requestId!);
      if (run) {
        if (run.sessionId !== sessionId || JSON.stringify(run.script) !== JSON.stringify(plan.script)) fail('The setup run does not match its original worktree plan.');
        const status = run.status === 'starting' ? 'running' : run.status;
        if (state.status !== status || state.runId !== run.runId || state.errorCode !== run.errorCode) state = this.save(state, {status, runId: run.runId, errorCode: run.errorCode});
      } else if (['running','succeeded'].includes(state.status) && !this.launching.has(sessionId)) state = this.save(state, {status: 'unknown', errorCode: 'SETUP_RUN_MISSING'});
    }
    return {plan, state};
  }
  private public(current: {plan: InitializationPlan; state: State} | null) {
    if (!current) return null;
    const {plan, state} = current;
    const uncertain = ['unknown','interrupted'].includes(state.status) && !!state.runId;
    const starting = this.launching.has(state.sessionId);
    const complete = ['disabled','succeeded','skipped'].includes(state.status);
    return {actions: {retry: !complete && !uncertain && !starting && state.status !== 'running', skip: !complete && !uncertain && !starting,
      ...(uncertain && {reason: 'The previous native process exit is unconfirmed. Refresh for a trusted exit result; retain this worktree and use a new independent session in the meantime.'}),
      ...(starting && {reason: 'Setup is starting; refresh its status before taking another action.'})}, version: state.version, status: state.status, ...(state.requestId && {requestId: state.requestId}), ...(state.runId && {runId: state.runId}), ...(state.errorCode && {errorCode: state.errorCode}),
      ...(plan.enabled && {script: {id: plan.script.id, version: plan.script.version, name: plan.script.name}})};
  }
  async get(workspaceId: string, sessionId: string) {return this.public(await this.current(workspaceId, sessionId));}
  async assertReady(workspaceId: string, sessionId: string) {
    const current = await this.current(workspaceId, sessionId);
    if (current && !['disabled','succeeded','skipped'].includes(current.state.status)) {
      const actions = this.public(current)!.actions;
      fail(`Worktree setup is ${current.state.status}. ${actions.reason ?? 'Retry the original setup or explicitly skip it before starting the Agent.'}`);
    }
  }
  private locked<T>(sessionId: string, fn: () => Promise<T>): Promise<T> {
    const previous = this.locks.get(sessionId) ?? Promise.resolve();
    const task = previous.then(fn, fn); this.locks.set(sessionId, task);
    void task.finally(() => {if (this.locks.get(sessionId) === task) this.locks.delete(sessionId);}).catch(() => {}); return task;
  }
  /** Called only for the creation that published its index in this process. */
  created(workspaceId: string, sessionId: string): Promise<void> {
    return this.locked(sessionId, async () => {
      this.check(workspaceId); const plan = await this.source(workspaceId, sessionId); if (!plan) return; validateInitializationPlan(plan);
      const file = this.file(workspaceId, sessionId); if (existsSync(file)) return;
      const state: State = {schema: 1, workspaceId, sessionId, planHash: this.hash(plan), version: randomUUID(), status: plan.enabled ? 'pending' : 'disabled', ...(plan.enabled && {requestId: plan.requestId})};
      this.write(state); if (plan.enabled) void this.launch(plan, state).catch(() => {});
    });
  }
  private async launch(plan: InitializationPlan, state: State) {
    if (!plan.enabled || !state.requestId) return;
    this.check(state.workspaceId);
    this.launching.add(state.sessionId);
    // Claim before the runner sees the original command. A crash cannot make a cold read replay this request.
    try {
      state = this.save(state, {status: 'running'});
      const original = await this.source(state.workspaceId, state.sessionId);
      if (!original || this.hash(original) !== state.planHash) fail('The original setup plan changed; no command was run.');
      this.check(state.workspaceId);
      const run = await this.runs.runCaptured(state.workspaceId, state.sessionId, state.requestId!, plan.script);
      this.save(state, {runId: run.runId, status: run.status === 'starting' ? 'running' : run.status});
    } catch (error) {this.save(state, {status: 'unknown', errorCode: error instanceof Error ? error.message : String(error)});}
    finally {this.launching.delete(state.sessionId);}
  }
  retry(workspaceId: string, sessionId: string, expectedVersion: string) {
    return this.locked(sessionId, async () => {
      const current = await this.current(workspaceId, sessionId);
      if (!current || !current.plan.enabled || current.state.version !== expectedVersion) fail('Setup state changed; reload before retrying.');
      if (!['failed','cancelled','interrupted','unknown','pending'].includes(current.state.status)) fail('Setup cannot be retried while running or after completion.');
      if (this.launching.has(sessionId)) fail('Setup is still starting; refresh before retrying.');
      // Refuse a new attempt if the prior native run might still be owned/live.
      if (current.state.runId) {const run = await this.runs.stop(workspaceId, current.state.runId); if (run.status === 'unknown' || run.status === 'interrupted') fail('Previous process exit is uncertain; setup cannot be retried.');}
      const state = this.save(current.state, {status: 'pending', requestId: randomUUID(), runId: undefined, errorCode: undefined});
      await this.launch(current.plan, state); return this.get(workspaceId, sessionId);
    });
  }
  skip(workspaceId: string, sessionId: string, expectedVersion: string) {
    return this.locked(sessionId, async () => {
      const current = await this.current(workspaceId, sessionId);
      if (!current || !current.plan.enabled || current.state.version !== expectedVersion) fail('Setup state changed; reload before skipping.');
      if (['succeeded','skipped'].includes(current.state.status)) fail('Setup already completed.');
      if (this.launching.has(sessionId)) fail('Setup is still starting; refresh before skipping.');
      if (current.state.runId) {const stopped = await this.runs.stop(workspaceId, current.state.runId); if (stopped.status === 'unknown' || stopped.status === 'interrupted') fail('Native process exit is uncertain; setup remains blocked.');}
      this.check(workspaceId); await this.source(workspaceId, sessionId); // Revalidate ownership after awaiting exit.
      this.save(current.state, {status: 'skipped'}); return this.get(workspaceId, sessionId);
    });
  }
  close() {this.closing = true;}
}

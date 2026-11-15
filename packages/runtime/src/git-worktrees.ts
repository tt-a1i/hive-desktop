import { validateInitializationPlan, type InitializationPlan } from './project-worktree-initialization.js';
import { execFile } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { mkdir, open, readFile, realpath, rename, rm, rmdir, unlink } from 'node:fs/promises';
import { isAbsolute, join, relative, resolve, sep } from 'node:path';
import { promisify } from 'node:util';

const run = promisify(execFile);
export interface ManagedWorktree {
  version: 1;
  projectRoot: string;
  chatId: string;
  ownerToken: string;
  repoCommonDir: string;
  gitDir?: string;
  managedWorktreeRoot: string;
  executionPath: string;
  projectPrefix: string;
  branch: string;
  baseOid: string;
}
/** Private creation provenance, saved before any checkout; never accepted from an RPC caller. */
export interface WorktreeCreation {
  version: 1 | 2;
  setup?: InitializationPlan;
  workspaceId: string;
  backend: 'pi' | 'codex' | 'pi-rpc' | 'claude';
  identityState: 'new' | 'known';
  originAgentId?: string;
  model?: {provider: string; id: string; name: string};
  forkOperationId?: string;
}
type Intent = { version?: 2 | 3; state: 'creating' | 'ready'; record: ManagedWorktree; creation?: WorktreeCreation };
function validateIntentCreation(intent: Intent): void {
  if (intent?.version !== undefined && (![2, 3].includes(intent.version) || intent.creation === undefined)
    || intent?.creation !== undefined && ![2, 3].includes(intent.version!))
    throw new GitWorktreeError('IDENTITY_MISMATCH', 'Versioned worktree creation provenance is missing or unsupported; legacy recovery was not used');
  if (intent.version === 3 ? intent.creation?.version !== 2 : intent.creation?.version === 2) throw new GitWorktreeError('IDENTITY_MISMATCH', 'Setup creation intent version is invalid');
  validateCreation(intent?.creation);
}
function validateCreation(creation: WorktreeCreation | undefined): void {
  if (creation === undefined) return; // Older generic intents have no recoverable Worker identity.
  const text = (value: unknown): value is string => typeof value === 'string' && !!value.trim() && value.length <= 4096 && !value.includes('\0');
  if (!creation || ![1, 2].includes(creation.version) || !text(creation.workspaceId)
    || !['pi','codex','pi-rpc','claude'].includes(creation.backend)
    || !['new','known'].includes(creation.identityState)
    || (creation.identityState === 'known' ? !text(creation.originAgentId) : creation.originAgentId !== undefined)
    || (creation.model !== undefined && (!creation.model || !text(creation.model.provider) || !text(creation.model.id) || !text(creation.model.name))))
    throw new GitWorktreeError('IDENTITY_MISMATCH', 'Worktree creation identity is invalid; it cannot be replaced with caller identity');
  if (creation.forkOperationId !== undefined && !/^[0-9a-f]{8}-[0-9a-f-]{27}$/i.test(creation.forkOperationId)) throw new GitWorktreeError('IDENTITY_MISMATCH', 'Invalid fork operation identity');
  if (creation.version === 2) validateInitializationPlan(creation.setup);
  else if (creation.setup !== undefined) throw new GitWorktreeError('IDENTITY_MISMATCH', 'Legacy intent cannot carry setup');
}

export class GitWorktreeError extends Error {
  constructor(public readonly code: string, message: string, options?: ErrorOptions) { super(message, options); this.name = 'GitWorktreeError'; }
}
function environment(): NodeJS.ProcessEnv {
  const env = { ...process.env };
  const config = { GIT_CONFIG_GLOBAL: env.GIT_CONFIG_GLOBAL, GIT_CONFIG_SYSTEM: env.GIT_CONFIG_SYSTEM, GIT_CONFIG_NOSYSTEM: env.GIT_CONFIG_NOSYSTEM };
  for (const key of Object.keys(env)) if (key.toUpperCase().startsWith('GIT_')) delete env[key];
  return { ...env, ...config, LC_ALL: 'C', GIT_OPTIONAL_LOCKS: '0', GIT_TERMINAL_PROMPT: '0', GIT_NO_LAZY_FETCH: '1', GIT_ALLOW_PROTOCOL: '', GIT_PROTOCOL_FROM_USER: '0', GIT_LITERAL_PATHSPECS: '1', GIT_PAGER: 'cat', PAGER: 'cat' };
}
async function git(cwd: string, ...args: string[]): Promise<string> {
  const checkout = args[0] === 'worktree' && args[1] === 'add';
  const config = ['--no-pager', '--no-optional-locks', '-c', 'core.fsmonitor=false', '-c', 'maintenance.auto=false', '-c', 'gc.auto=0', '-c', 'protocol.allow=never'];
  if (!checkout || args.includes('--no-checkout')) config.push('-c', `core.hooksPath=${process.platform === 'win32' ? 'NUL' : '/dev/null'}`);
  try {
    const result = await run('git', [...config, ...args], { cwd, maxBuffer: 16 * 1024 * 1024, env: environment(), timeout: 30000, windowsHide: true });
    // NUL-framed output must retain every byte. A line-oriented Git response
    // contributes exactly one LF; whitespace in a path is part of the path.
    const output = result.stdout;
    return output.endsWith('\0') ? output : output.endsWith('\n') ? output.slice(0, -1) : output;
  } catch (error) {
    const fault = error as NodeJS.ErrnoException & { killed?: boolean; stderr?: string };
    const detail = (fault.stderr ?? '').slice(0, 1500).replace(/[\x00-\x08\x0b-\x1f\x7f]/g, '');
    let code = fault.code === 'ENOENT' ? 'GIT_NOT_FOUND' : fault.killed ? 'GIT_TIMEOUT' : /not a git repository/.test(detail) ? 'NOT_REPOSITORY' : 'GIT_COMMAND_FAILED';
    if (args[0] === 'rev-parse' && args.includes('HEAD^{commit}') && code === 'GIT_COMMAND_FAILED') code = 'UNBORN_OR_MISSING_HEAD';
    if (checkout && /already exists|already checked out/.test(detail)) code = 'BRANCH_OCCUPIED';
    throw new GitWorktreeError(code, detail ? `Git operation failed: ${detail}` : `Git operation failed: ${code}`, { cause: error });
  }
}
function chatIdValid(id: string): boolean { return /^[A-Za-z0-9_-]{1,100}$/.test(id); }
function contained(root: string, path: string): boolean {
  const r = relative(root, path);
  return r !== '' && r !== '..' && !r.startsWith(`..${sep}`) && !isAbsolute(r);
}
async function commonDir(cwd: string): Promise<string> {
  return realpath(resolve(cwd, await git(cwd, 'rev-parse', '--git-common-dir')));
}
async function writeIntent(ownerDir: string, intent: Intent): Promise<void> {
  const tmp = join(ownerDir, `.${randomUUID()}.tmp`);
  try {
    const document: Intent = intent.creation ? {...intent, version: intent.creation.version === 2 ? 3 : 2} : intent;
    validateIntentCreation(document);
    const file = await open(tmp, 'wx', 0o600);
    try { await file.writeFile(JSON.stringify(document)); await file.sync(); }
    finally { await file.close(); }
    await rename(tmp, join(ownerDir, 'intent.json'));
    if (process.platform !== 'win32') {
      const directory = await open(ownerDir, 'r');
      try { await directory.sync(); } finally { await directory.close(); }
    }
  } finally { await rm(tmp, { force: true }); }
}
function ownerDir(dataRoot: string, chatId: string): string {
  if (!chatIdValid(chatId)) throw new TypeError('Invalid chat ID');
  return join(dataRoot, 'worktrees', chatId);
}
/** Explicit checkout: Git may invoke the user's configured checkout filters or hooks. */
export async function createManagedWorktree(input: { projectRoot: string; dataRoot: string; chatId: string; branch?: string; creation?: WorktreeCreation }): Promise<ManagedWorktree> {
  if (input.creation?.forkOperationId) throw new GitWorktreeError('IDENTITY_MISMATCH', 'Fork worktrees require the dedicated creation path');
  return createWorktree(input);
}
export async function createForkWorktree(input: { projectRoot: string; dataRoot: string; chatId: string; branch?: string; creation: WorktreeCreation; operationId: string; baseOid: string }): Promise<ManagedWorktree> {
  if (!/^[0-9a-f]{40}([0-9a-f]{24})?$/i.test(input.baseOid) || input.creation.forkOperationId !== input.operationId) throw new GitWorktreeError('IDENTITY_MISMATCH', 'Fork provenance is invalid');
  return createWorktree(input, input.baseOid);
}
async function createWorktree(input: { projectRoot: string; dataRoot: string; chatId: string; branch?: string; creation?: WorktreeCreation }, forkBase?: string): Promise<ManagedWorktree> {
  if (!chatIdValid(input.chatId)) throw new TypeError('Invalid chat ID');
  validateCreation(input.creation);
  const project = await realpath(input.projectRoot);
  const repository = await realpath(await git(project, 'rev-parse', '--show-toplevel'));
  const prefix = relative(repository, project);
  if (prefix && !contained(repository, project)) throw new Error('Project is outside repository');
  const common = await commonDir(project);
  const baseOid = await git(project, 'rev-parse', '--verify', `${forkBase ?? 'HEAD'}^{commit}`);
  if (forkBase && baseOid !== forkBase) throw new GitWorktreeError('IDENTITY_MISMATCH', 'Fork base commit changed');
  if (forkBase && prefix) throw new GitWorktreeError('UNSUPPORTED_PROJECT_PREFIX', 'Fork materialization requires the repository root');
  if (prefix) {
    try {
      const gitPath = prefix.split(sep).join('/');
      if (await git(project, 'cat-file', '-t', `${baseOid}:${gitPath}`) !== 'tree') throw new Error('Not a directory');
    } catch (error) {
      if (error instanceof GitWorktreeError && error.code !== 'GIT_COMMAND_FAILED') throw error;
      throw new GitWorktreeError('PROJECT_NOT_IN_BASE', 'Project directory is not present in the base commit', { cause: error });
    }
  }
  const branch = input.branch ?? `hive/${input.chatId}`;
  if (!branch || branch.startsWith('-')) throw new TypeError('Invalid branch');
  await git(project, 'check-ref-format', '--branch', branch);
  await mkdir(input.dataRoot, { recursive: true, mode: 0o700 });
  const root = await realpath(input.dataRoot);
  const owners = join(root, 'worktrees');
  await mkdir(owners, { recursive: true, mode: 0o700 });
  if (await realpath(owners) !== owners) throw new GitWorktreeError('IDENTITY_MISMATCH', 'Managed worktree directory was replaced');
  const dir = ownerDir(root, input.chatId);
  await mkdir(dir, { mode: 0o700 }); // exclusive ownership; never reuse an existing path
  const worktree = join(dir, 'tree');
  const record: ManagedWorktree = { version: 1, projectRoot: project, chatId: input.chatId, ownerToken: randomUUID(), repoCommonDir: common,
    managedWorktreeRoot: worktree, executionPath: join(worktree, prefix), projectPrefix: prefix, branch, baseOid };
  try { await writeIntent(dir, { state: 'creating', record, ...(input.creation && {creation: input.creation}) }); }
  catch (error) { await rmdir(dir).catch(() => {}); throw new GitWorktreeError('METADATA_WRITE_FAILED', 'Could not save worktree creation intent', { cause: error }); }
  await git(project, 'worktree', 'add', ...(forkBase ? ['--no-checkout'] : []), '-b', branch, '--', worktree, baseOid);
  try {
    const gitDir = await worktreeGitDir(worktree, common);
    const marker = await open(join(gitDir, 'hive-desktop-owner.json'), 'wx', 0o600);
    try { await marker.writeFile(JSON.stringify({ version: 1, chatId: record.chatId, ownerToken: record.ownerToken })); await marker.sync(); }
    finally { await marker.close(); }
    record.gitDir = gitDir;
  } catch (error) {
    throw new GitWorktreeError('METADATA_WRITE_FAILED', 'Worktree created but ownership marker could not be established; retain it for manual inspection', { cause: error });
  }
  try { await writeIntent(dir, { state: 'ready', record, ...(input.creation && {creation: input.creation}) }); }
  catch (error) { throw new GitWorktreeError('METADATA_WRITE_FAILED', 'Worktree created but ownership publication failed; inspect creation intent to recover', { cause: error }); }
  if (!forkBase) await validateManagedWorktree(root, record);
  return record;
}
async function worktreeGitDir(worktree: string, common: string): Promise<string> {
  const gitDir = await realpath(await git(worktree, 'rev-parse', '--absolute-git-dir'));
  if (!contained(join(common, 'worktrees'), gitDir)) throw new GitWorktreeError('IDENTITY_MISMATCH', 'Worktree Git metadata is outside the original repository');
  return gitDir;
}
async function verifyIdentity(dataRoot: string, record: ManagedWorktree): Promise<{ root: string; intent: Intent; locked: boolean; gitDir: string }> {
  const root = await realpath(dataRoot);
  const dir = ownerDir(root, record.chatId);
  if (await realpath(join(root, 'worktrees')) !== join(root, 'worktrees') || await realpath(dir) !== dir) throw new GitWorktreeError('IDENTITY_MISMATCH', 'Worktree ownership directory was replaced');
  const intent = JSON.parse(await readFile(join(dir, 'intent.json'), 'utf8')) as Intent;
  validateIntentCreation(intent);
  const keys: (keyof ManagedWorktree)[] = ['version', 'projectRoot', 'chatId', 'ownerToken', 'repoCommonDir', 'gitDir', 'managedWorktreeRoot', 'executionPath', 'projectPrefix', 'branch', 'baseOid'];
  if (!intent || !['creating', 'ready'].includes(intent.state) || !intent.record || keys.some(key => intent.record[key] !== record[key]) || record.version !== 1) throw new GitWorktreeError('IDENTITY_MISMATCH', 'Worktree ownership does not match');
  if (await realpath(record.projectRoot) !== record.projectRoot || await commonDir(record.projectRoot) !== record.repoCommonDir) throw new GitWorktreeError('IDENTITY_MISMATCH', 'Original project repository identity changed');
  const originalTop = await realpath(await git(record.projectRoot, 'rev-parse', '--show-toplevel'));
  if (relative(originalTop, record.projectRoot) !== record.projectPrefix) throw new GitWorktreeError('IDENTITY_MISMATCH', 'Original project scope changed');
  const expected = join(dir, 'tree');
  if (record.managedWorktreeRoot !== expected || record.executionPath !== join(expected, record.projectPrefix) || isAbsolute(record.projectPrefix) || record.projectPrefix.split(/[\\/]/).includes('..')) throw new GitWorktreeError('IDENTITY_MISMATCH', 'Invalid managed worktree location');
  if (await realpath(expected) !== expected || await realpath(record.executionPath) !== record.executionPath) throw new GitWorktreeError('IDENTITY_MISMATCH', 'Worktree path was replaced');
  if (await commonDir(expected) !== record.repoCommonDir || await realpath(await git(expected, 'rev-parse', '--show-toplevel')) !== expected) throw new GitWorktreeError('IDENTITY_MISMATCH', 'Worktree repository identity changed');
  const gitDir = await worktreeGitDir(expected, record.repoCommonDir);
  if (record.gitDir !== undefined && record.gitDir !== gitDir) throw new GitWorktreeError('IDENTITY_MISMATCH', 'Worktree Git metadata identity changed');
  const marker = JSON.parse(await readFile(join(gitDir, 'hive-desktop-owner.json'), 'utf8')) as { version?: unknown; chatId?: unknown; ownerToken?: unknown };
  if (marker.version !== 1 || marker.chatId !== record.chatId || marker.ownerToken !== record.ownerToken) throw new GitWorktreeError('IDENTITY_MISMATCH', 'Worktree ownership marker does not match');
  if (intent.state === 'ready' && !record.gitDir) throw new GitWorktreeError('IDENTITY_MISMATCH', 'Worktree identity was not recorded');
  if (await git(expected, 'symbolic-ref', '--short', 'HEAD') !== record.branch) throw new GitWorktreeError('IDENTITY_MISMATCH', 'Worktree branch changed');
  const list = await git(expected, 'worktree', 'list', '--porcelain', '-z');
  const blocks = list.split('\0\0').map(block => block.split('\0'));
  let item: string[] | undefined;
  for (const block of blocks) {
    const entry = block.find(line => line.startsWith('worktree '));
    if (!entry) continue;
    try {
      if (await realpath(entry.slice('worktree '.length)) === expected) { item = block; break; }
    } catch {
      // An unrelated registered worktree may be missing or inaccessible.
    }
  }
  if (!item || !item.includes(`branch refs/heads/${record.branch}`)) throw new GitWorktreeError('IDENTITY_MISMATCH', 'Worktree is not registered to this repository');
  return { root, intent, gitDir, locked: item.some(line => line === 'locked' || line.startsWith('locked ')) };
}
async function verify(dataRoot: string, record: ManagedWorktree): Promise<{ root: string; intent: Intent; locked: boolean; gitDir: string }> {
  try { return await verifyIdentity(dataRoot, record); }
  catch (error) {
    if ((error as NodeJS.ErrnoException).code === 'ENOENT' || error instanceof SyntaxError || error instanceof TypeError || (error instanceof GitWorktreeError && error.code === 'NOT_REPOSITORY'))
      throw new GitWorktreeError('IDENTITY_MISMATCH', 'Original project, ownership record, or managed worktree is missing or replaced', { cause: error });
    throw error;
  }
}
export async function validateManagedWorktree(dataRoot: string, record: ManagedWorktree): Promise<string> {
  const { intent } = await verify(dataRoot, record);
  if (intent.creation?.forkOperationId) throw new GitWorktreeError('FORK_PENDING', 'Fork worktree requires its dedicated operation gate');
  if (intent.state !== 'ready') throw new GitWorktreeError('RECOVERY_REQUIRED', 'Worktree creation was interrupted; recovery is required');
  return record.executionPath;
}
/** Caller must determine that no session or process uses this worktree. Branch is retained. */
export async function removeManagedWorktree(dataRoot: string, record: ManagedWorktree, options: { active: boolean }): Promise<void> {
  if (options.active) throw new GitWorktreeError('WORKTREE_IN_USE', 'Worktree is in use');
  const { root, intent, locked } = await verify(dataRoot, record);
  if (intent.creation?.forkOperationId) throw new GitWorktreeError('FORK_PENDING', 'Fork worktree requires its dedicated operation gate');
  if (intent.state !== 'ready') throw new GitWorktreeError('RECOVERY_REQUIRED', 'Worktree creation was interrupted; recovery is required');
  if (locked) throw new GitWorktreeError('WORKTREE_LOCKED', 'Worktree is locked');
  if (await git(record.managedWorktreeRoot, 'status', '--porcelain', '--untracked-files=all')) throw new GitWorktreeError('WORKTREE_DIRTY', 'Worktree has uncommitted files');
  const head = await git(record.managedWorktreeRoot, 'rev-parse', 'HEAD^{commit}');
  if (head !== record.baseOid) {
    let upstream: string;
    try { upstream = await git(record.managedWorktreeRoot, 'rev-parse', '--abbrev-ref', '--symbolic-full-name', '@{upstream}'); }
    catch { throw new GitWorktreeError('UNPUSHED_COMMITS', 'Worktree has commits without an upstream'); }
    if (await git(record.managedWorktreeRoot, 'rev-list', `${upstream}..HEAD`, '--max-count=1')) throw new GitWorktreeError('UNPUSHED_COMMITS', 'Worktree has unpushed commits');
  }
  await git(record.managedWorktreeRoot, 'worktree', 'remove', '--', record.managedWorktreeRoot);
  await unlink(join(ownerDir(root, record.chatId), 'intent.json'));
  await rmdir(ownerDir(root, record.chatId));
}

/** Inspect an interrupted creation without modifying it. */
export async function readWorktreeIntent(dataRoot: string, chatId: string): Promise<Intent> {
  const root = await realpath(dataRoot);
  const dir = ownerDir(root, chatId);
  if (await realpath(join(root, 'worktrees')) !== join(root, 'worktrees') || await realpath(dir) !== dir) throw new GitWorktreeError('IDENTITY_MISMATCH', 'Worktree ownership directory was replaced');
  const intent = JSON.parse(await readFile(join(dir, 'intent.json'), 'utf8')) as Intent;
  validateIntentCreation(intent);
  if (!intent || !['creating', 'ready'].includes(intent.state) || intent.record?.version !== 1 || intent.record.chatId !== chatId) throw new Error('Invalid worktree intent');
  return intent;
}
/** Claim an already-created, verified worktree after an interrupted save. Never creates or overwrites a tree. */
export async function recoverManagedWorktree(dataRoot: string, record: ManagedWorktree): Promise<ManagedWorktree> {
  const { root, intent, gitDir } = await verify(dataRoot, record);
  if (intent.creation?.forkOperationId) throw new GitWorktreeError('FORK_PENDING', 'Fork worktree requires its dedicated recovery');
  const recovered = { ...record, gitDir };
  if (intent.state === 'creating') await writeIntent(ownerDir(root, record.chatId), { ...intent, state: 'ready', record: recovered });
  await validateManagedWorktree(root, recovered);
  return recovered;
}

/** Verify private fork ownership without making it available to generic recovery. */
export async function validateForkWorktree(dataRoot: string, record: ManagedWorktree, operationId: string): Promise<void> {
  const { intent } = await verify(dataRoot, record);
  if (intent.state !== 'ready' || intent.creation?.forkOperationId !== operationId) throw new GitWorktreeError('IDENTITY_MISMATCH', 'Fork operation does not own this worktree');
}

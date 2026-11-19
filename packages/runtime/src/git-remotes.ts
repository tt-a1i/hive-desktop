import { spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import { access, realpath, stat } from 'node:fs/promises';
import { constants } from 'node:fs';
import { homedir } from 'node:os';
import { dirname, isAbsolute, join, relative, sep } from 'node:path';
import { resolveClaudeExecutable } from './agents/claude.js';
import { OwnedCliProcess } from './agents/owned-cli-process.js';
import { getGitWorkspaceVersion, GitWorkspaceError, type GitReadOptions, type GitWorkspaceVersion } from './git-workspace.js';

export interface GitRemoteOptions extends GitReadOptions {}
export class GitRemoteError extends GitWorkspaceError {}
export const remoteError = (code: string, message: string): GitRemoteError => new GitRemoteError(code, redactRemoteText(message));
export const remoteVersion = (value: unknown): string => createHash('sha256').update(JSON.stringify(value)).digest('hex');
/** Display/error boundary only. Never use the sanitized value as a transport address. */
export function redactRemoteText(value: string): string {
 // Authentication headers can contain a scheme, multiple words or comma-separated
 // parameters. Redact the entire value before processing ordinary key/value text.
 let result = value.replace(/\b((?:proxy-)?authorization[ \t]*[=:][ \t]*)[^\r\n]*(?:\r?\n[ \t]+[^\r\n]*)*/gi, '$1[redacted]');
 // A legal URL password may contain raw @ characters. The last @ in the
 // authority ends userinfo; matching only the first @ leaves password bytes.
 result = result.replace(/([a-z][a-z\d+.-]*:\/\/)([^\s/?#]*)/gi, (_match, scheme: string, authority: string) => {
  const end = authority.lastIndexOf('@'); return scheme + (end < 0 ? authority : '[redacted]@' + authority.slice(end + 1));
 }).replace(/[^\s/:]+:[^\s/]+@(?=[A-Za-z0-9.-]+[:/])/g, '[redacted]@').replace(/\?[^\s]*/g, '?[redacted]').replace(/#[^\s]*/g, '#[redacted]')
  .replace(/\b((?:access[_-]?token|token|password|passwd|signature|credential|api[_-]?key)\s*[=:]\s*)[^\s,;]+/gi, '$1[redacted]');
 for (const key of ['GH_TOKEN', 'GITHUB_TOKEN', 'GH_ENTERPRISE_TOKEN', 'GITHUB_ENTERPRISE_TOKEN']) {const secret = process.env[key]; if (secret) result = result.replaceAll(secret, '[redacted]');}
 return result.replace(/[\x00-\x08\x0b-\x1f\x7f]/g, '');
}
function inside(root: string, path: string): boolean {const r = relative(root, path); return !r || (r !== '..' && !r.startsWith('..' + sep) && !isAbsolute(r));}
/** Fixed internal option overrides are for trusted embedding/tests, never an RPC parameter. */
export async function resolveRemoteExecutable(name: 'git' | 'gh', projectRoot: string, override?: string): Promise<string> {
 const project = await realpath(projectRoot);
 if (override && !isAbsolute(override)) throw remoteError('INVALID_EXECUTABLE', 'Internal executable override must be absolute');
 const paths = [...(process.env.PATH ?? '').split(process.platform === 'win32' ? ';' : ':').filter(path => isAbsolute(path)),
  ...(process.platform === 'darwin' ? ['/opt/homebrew/bin', '/usr/local/bin', join(homedir(), '.local/bin'), '/usr/bin', '/bin'] : [])];
 // Discover repository ancestors without running a candidate binary from that repository.
 let current = project; const roots: string[] = [];
 for (;;) {try {await access(join(current, '.git')); roots.push(current);} catch {} const parent = dirname(current); if (parent === current) break; current = parent;}
 const safePaths: string[] = [];
 for (const path of paths) {try {const canonical = await realpath(path); if (!roots.some(root => inside(root, canonical)) && !inside(project, canonical)) safePaths.push(canonical);} catch {}}
 let executable: string;
 try {executable = await realpath(resolveClaudeExecutable(override ?? name, process.platform, safePaths.join(process.platform === 'win32' ? ';' : ':')));}
 catch {throw remoteError(`${name.toUpperCase()}_NOT_FOUND`, `${name} native executable was not found outside the project; install it or configure the desktop environment PATH`);}
 if (roots.some(root => inside(root, executable)) || inside(project, executable)) throw remoteError('UNTRUSTED_EXECUTABLE', 'The project cannot supply the Git or GitHub CLI executable');
 await access(executable, constants.X_OK); if (!(await stat(executable)).isFile()) throw remoteError('INVALID_EXECUTABLE', 'Expected a native executable file');
 return executable;
}
export function remoteEnvironment(github = false): NodeJS.ProcessEnv {
 const env = {...process.env}; const configuration = {GIT_CONFIG_GLOBAL: env.GIT_CONFIG_GLOBAL, GIT_CONFIG_SYSTEM: env.GIT_CONFIG_SYSTEM, GIT_CONFIG_NOSYSTEM: env.GIT_CONFIG_NOSYSTEM};
 for (const key of Object.keys(env)) if (key.toUpperCase().startsWith('GIT_') || (github && ['GH_REPO','GH_HOST','GH_BROWSER','GH_EDITOR','GH_DEBUG','GH_FORCE_TTY','GH_PAGER'].includes(key.toUpperCase()))) delete env[key];
 return {...env, ...configuration, LC_ALL: 'C', GIT_OPTIONAL_LOCKS: '0', GIT_TERMINAL_PROMPT: '0', GIT_NO_LAZY_FETCH: '1', GIT_NO_REPLACE_OBJECTS: '1', GIT_GRAFT_FILE: process.platform === 'win32' ? 'NUL' : '/dev/null', GIT_PAGER: 'cat', PAGER: 'cat', ...(github && {GH_PROMPT_DISABLED: '1', GH_NO_UPDATE_NOTIFIER: '1', GH_NO_EXTENSION_UPDATE_NOTIFIER: '1'})};
}
export interface RemoteCommandResult {stdout: string; stderr: string; code: number}
/** Bounded process group/Windows Job ownership; callers must treat write failures as uncertain. */
export async function remoteCommand(executable: string, args: string[], cwd: string, timeoutMs = 15000, github = false, accepted = [0]): Promise<RemoteCommandResult> {
 if (!Number.isSafeInteger(timeoutMs) || timeoutMs < 100 || timeoutMs > 120000) throw remoteError('INVALID_ARGUMENT', 'timeoutMs must be from 100 to 120000');
 const env = remoteEnvironment(github), owned = process.platform === 'win32' ? new OwnedCliProcess(executable, args, {cwd, env}) : undefined;
 const child = owned?.child ?? spawn(executable, args, {cwd, env, detached: true, windowsHide: true, stdio: 'pipe'});
 let timedOut = false, exceeded = false, fault: NodeJS.ErrnoException | undefined, bytes = 0; const out: Buffer[] = [], err: Buffer[] = [];
 let escalation: ReturnType<typeof setTimeout> | undefined;
 const kill = (signal: NodeJS.Signals) => {if (owned) {void owned.close(0).catch(() => {}); return;} if (child.pid) {try {process.kill(-child.pid, signal);} catch {child.kill(signal);}}};
 const terminate = () => {kill('SIGTERM'); escalation ??= setTimeout(() => kill('SIGKILL'), 250);};
 const collect = (target: Buffer[], data: Buffer) => {const left = 8 * 1024 * 1024 - bytes; bytes += data.length; if (left > 0) target.push(data.subarray(0, left)); if (bytes > 8 * 1024 * 1024 && !exceeded) {exceeded = true; terminate();}};
 child.stdout.on('data', (data: Buffer) => collect(out, data)); child.stderr.on('data', (data: Buffer) => collect(err, data));
 child.on('error', error => {fault = error;}); child.stdin.on('error', error => {if ((error as NodeJS.ErrnoException).code !== 'EPIPE') {fault = error; terminate();}}); child.stdin.end();
 const timer = setTimeout(() => {timedOut = true; terminate();}, timeoutMs);
 const code = await new Promise<number | null>(resolve => child.once('close', resolve));
 clearTimeout(timer); clearTimeout(escalation); if (timedOut || exceeded) kill('SIGKILL'); if (owned) await owned.close(0);
 const stderr = redactRemoteText(Buffer.concat(err).toString('utf8')).slice(0, 4000);
 if (timedOut || exceeded || fault || code === null || !accepted.includes(code)) throw remoteError(timedOut ? 'REMOTE_TIMEOUT' : exceeded ? 'OUTPUT_LIMIT' : fault?.code === 'ENOENT' ? 'CLI_NOT_FOUND' : 'REMOTE_COMMAND_FAILED', `Remote command ${timedOut ? 'timed out' : 'failed'}${stderr ? ': ' + stderr : ''}`);
 return {stdout: Buffer.concat(out).toString('utf8'), stderr, code};
}
export const gitRemoteArgs = (args: string[]): string[] => ['--no-pager', '-c', 'core.fsmonitor=false', '-c', 'maintenance.auto=false', '-c', 'gc.auto=0', ...args];
export function validateRemoteName(remote: string): void {if (typeof remote !== 'string' || !/^[A-Za-z0-9][A-Za-z0-9._/-]{0,199}$/.test(remote)) throw remoteError('INVALID_REMOTE', 'Select a configured remote name');}
export async function remoteWorkspaceVersion(projectRoot: string, git: string, timeoutMs?: number): Promise<GitWorkspaceVersion> {
 try {return await getGitWorkspaceVersion(projectRoot, {gitExecutable: git, ...(timeoutMs !== undefined && {timeoutMs: Math.min(timeoutMs, 30000)})});}
 catch (error) {throw remoteError(error instanceof GitWorkspaceError ? error.code : 'GIT_READ_FAILED', error instanceof Error ? error.message : String(error));}
}
/** Internal transport context; only sanitized preview/list results may cross the desktop wire. */
export interface GitRemoteContext {workspace: GitWorkspaceVersion; git: string; remote: string; url: string; displayUrl: string}
export async function readGitRemote(projectRoot: string, remote: string, options: GitRemoteOptions = {}): Promise<GitRemoteContext> {
 validateRemoteName(remote); const git = await resolveRemoteExecutable('git', projectRoot, options.gitExecutable);
 const workspace = await remoteWorkspaceVersion(projectRoot, git, options.timeoutMs);
 const run = (args: string[]) => remoteCommand(git, gitRemoteArgs(args), workspace.repositoryRoot, options.timeoutMs);
 const urls = (value: string) => value.replace(/\n$/, '').split('\n');
 const fetch = urls((await run(['remote', 'get-url', '--all', remote])).stdout), push = urls((await run(['remote', 'get-url', '--push', '--all', remote])).stdout);
 if (fetch.length !== 1 || push.length !== 1 || fetch[0] !== push[0]) throw remoteError('UNSUPPORTED_REMOTE', 'Use one identical fetch and push URL for this remote; multiple destinations are not supported');
 const url = push[0]!; if (!url || /[\x00-\x1f\x7f]/.test(url) || url.startsWith('-') || /^[A-Za-z][\w+.-]*::/.test(url)) throw remoteError('UNSUPPORTED_REMOTE', 'This remote transport is not supported');
 return {workspace, git, remote, url, displayUrl: redactRemoteText(url)};
}
export async function remoteBranchOid(context: GitRemoteContext, branch: string, options: GitRemoteOptions = {}): Promise<string | null> {
 if (typeof branch !== 'string' || !branch || branch.length > 1024 || /[\x00-\x1f\x7f]/.test(branch)) throw remoteError('INVALID_BRANCH', 'Select a valid remote branch');
 await remoteCommand(context.git, gitRemoteArgs(['check-ref-format', `refs/heads/${branch}`]), context.workspace.repositoryRoot, options.timeoutMs);
 const output = await remoteCommand(context.git, gitRemoteArgs(['ls-remote', '--refs', '--exit-code', '--', context.remote, `refs/heads/${branch}`]), context.workspace.repositoryRoot, options.timeoutMs, false, [0, 2]);
 if (output.code === 2) return null;
 const lines = output.stdout.trimEnd().split('\n'); if (lines.length !== 1) throw remoteError('INVALID_REMOTE_OUTPUT', 'Remote returned an ambiguous branch');
 const [oid, ref] = lines[0]!.split('\t'); if (!oid || !/^(?:[a-f0-9]{40}|[a-f0-9]{64})$/.test(oid) || ref !== `refs/heads/${branch}`) throw remoteError('INVALID_REMOTE_OUTPUT', 'Remote returned an invalid branch identity');
 return oid;
}
export async function listGitRemotes(projectRoot: string, options: GitRemoteOptions = {}) {
 const git = await resolveRemoteExecutable('git', projectRoot, options.gitExecutable), workspace = await remoteWorkspaceVersion(projectRoot, git, options.timeoutMs);
 const names = (await remoteCommand(git, gitRemoteArgs(['remote']), workspace.repositoryRoot, options.timeoutMs)).stdout.trim().split('\n').filter(Boolean);
 const remotes: {name: string; url?: string; error?: string}[] = [];
 for (const name of names) {try {const context = await readGitRemote(projectRoot, name, options); remotes.push({name, url: context.displayUrl});} catch (error) {remotes.push({name, error: redactRemoteText(error instanceof Error ? error.message : String(error))});}}
 return {workspace, remotes};
}
export interface GitPushTarget {remote: string; branch: string}
export interface GitPushPreview {version: string; workspace: GitWorkspaceVersion; remote: string; remoteUrl: string; branch: string; headOid: string; remoteOid: string | null; relation: 'new'|'fast-forward'|'up-to-date'|'diverged'|'unknown-history'; commits: {oid: string; subject: string}[]; commitsComplete: boolean; canPush: boolean; warnings: string[]; nativeHooks: true; nativeTrackingRefsMayUpdate: true}
export async function previewGitPush(projectRoot: string, target: GitPushTarget, options: GitRemoteOptions = {}): Promise<GitPushPreview> {
 const context = await readGitRemote(projectRoot, target.remote, options), {workspace, git} = context;
 if (!workspace.branch || !workspace.headOid) throw remoteError('BRANCH_REQUIRED', 'Push requires an existing local branch and commit');
 const remoteOid = await remoteBranchOid(context, target.branch, options), headOid = workspace.headOid;
 const run = (args: string[], accepted = [0]) => remoteCommand(git, gitRemoteArgs(args), workspace.repositoryRoot, options.timeoutMs, false, accepted);
 let relation: GitPushPreview['relation'] = remoteOid === headOid ? 'up-to-date' : remoteOid === null ? 'new' : 'unknown-history';
 let known = remoteOid === null;
 if (remoteOid && remoteOid !== headOid) {const available = await run(['cat-file', '-e', `${remoteOid}^{commit}`], [0, 1, 128]); if (available.code === 0) {known = true; relation = (await run(['merge-base', '--is-ancestor', remoteOid, headOid], [0, 1])).code === 0 ? 'fast-forward' : 'diverged';}}
 const log = relation === 'up-to-date' || relation === 'unknown-history' ? '' : (await run(['log', '--format=%H%x00%s', '-z', '--max-count=501', headOid, ...(known && remoteOid ? ['--not', remoteOid] : []), '--'])).stdout;
 const rows = log.split('\0'); const commits: {oid: string; subject: string}[] = [];
 for (let i = 0; i + 1 < rows.length; i += 2) commits.push({oid: rows[i]!, subject: redactRemoteText(rows[i + 1]!)});
 const after = await remoteWorkspaceVersion(projectRoot, git, options.timeoutMs); if (JSON.stringify(after) !== JSON.stringify(workspace)) throw remoteError('CONCURRENT_CHANGE', 'Local branch, HEAD, index, or configuration changed during push preview');
 const payload = {workspace, remote: target.remote, remoteUrl: context.displayUrl, branch: target.branch, headOid, remoteOid, relation, commits: commits.slice(0, 500), commitsComplete: (known || relation === 'up-to-date') && commits.length <= 500, canPush: relation !== 'diverged' && relation !== 'unknown-history', warnings: relation === 'unknown-history' ? ['Remote history is not present locally; outgoing commits cannot be enumerated. No fetch was performed.'] : [], nativeHooks: true as const, nativeTrackingRefsMayUpdate: true as const};
 return {version: remoteVersion({...payload, transportFingerprint: remoteVersion(context.url), git}), ...payload};
}
export interface GitPushResult {ok: boolean; pushed: true|'unknown'; verified: boolean; headOid: string; remoteOid: string | null; error?: {code: string; message: string}; nativeTrackingRefsMayUpdate: true}
export interface VerifiedGitPush extends GitPushTarget {
 workspace: GitWorkspaceVersion; headOid: string; transport: string;
}
export async function pushGitBranch(projectRoot: string, request: GitPushTarget & {expectedVersion: string},
 options: GitRemoteOptions & {recordVerifiedPush?: (proof: VerifiedGitPush) => Promise<void>} = {}): Promise<GitPushResult> {
 const preview = await previewGitPush(projectRoot, request, options); if (preview.version !== request.expectedVersion) throw remoteError('STALE_PREVIEW', 'Branch, HEAD, configuration or remote changed; refresh push preview');
 if (preview.relation === 'diverged') throw remoteError('NON_FAST_FORWARD', 'The approved remote commit is not an ancestor of this local commit; integrate the remote branch and preview again');
 if (preview.relation === 'unknown-history') throw remoteError('REMOTE_HISTORY_REQUIRED', 'Remote history is not present locally; fetch it explicitly in your native Git tools, then preview again. No fetch or push was started');
 const context = await readGitRemote(projectRoot, request.remote, options); if (JSON.stringify(context.workspace) !== JSON.stringify(preview.workspace)) throw remoteError('STALE_PREVIEW', 'Local repository changed before push');
 // The explicit lease binds the server update to the preview's exact old OID
 // (empty for a new branch). It is used only after proving ordinary fast-forward
 // ancestry with replacement objects/grafts disabled, so it cannot authorize a
 // history rewrite. There is no force option exposed to the caller.
 const lease = `--force-with-lease=refs/heads/${request.branch}:${preview.remoteOid ?? ''}`;
 let failure: unknown;
 try {await remoteCommand(context.git, gitRemoteArgs(['-c', `remote.${request.remote}.mirror=false`, '-c', 'push.followTags=false', '-c', 'push.autoSetupRemote=false', 'push', lease, '--porcelain', '--no-follow-tags', '--recurse-submodules=no', '--', request.remote, `${preview.headOid}:refs/heads/${request.branch}`]), context.workspace.repositoryRoot, options.timeoutMs);}
 catch (error) {failure = error;}
 let remoteOid: string | null = null; let verified = false;
 try {
  const observed = await readGitRemote(projectRoot, request.remote, options);
  if (observed.url !== context.url || observed.workspace.configFingerprint !== context.workspace.configFingerprint) throw remoteError('REMOTE_CONFIGURATION_CHANGED', 'Remote configuration changed during push; inspect both targets before retrying');
  remoteOid = await remoteBranchOid(observed, request.branch, options); verified = remoteOid === preview.headOid;
 } catch (error) {failure ??= error;}
 if (failure || !verified) return {ok: false, pushed: 'unknown', verified, headOid: preview.headOid, remoteOid, nativeTrackingRefsMayUpdate: true, error: {code: 'PUSH_STATE_UNCERTAIN', message: redactRemoteText(failure instanceof Error ? failure.message : 'Push completed but the remote branch could not be verified; inspect the remote before retrying')}};
 const result = {ok: true, pushed: true, verified: true, headOid: preview.headOid, remoteOid, nativeTrackingRefsMayUpdate: true} as const;
 try {
  await options.recordVerifiedPush?.({workspace: context.workspace, headOid: preview.headOid,
   remote: request.remote, branch: request.branch, transport: remoteVersion(context.url)});
 } catch {
  // A local receipt error cannot change the already-verified remote outcome.
  return {...result, error: {code: 'WORKTREE_PUSH_RECEIPT_FAILED', message: 'The remote push was verified, but Hive could not save the proof for worktree removal. Check the local worktree before retrying.'}};
 }
 return result;
}

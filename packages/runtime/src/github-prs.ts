import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { readGitRemote, remoteWorkspaceVersion, remoteBranchOid, remoteCommand, remoteError, remoteVersion, resolveRemoteExecutable, redactRemoteText, type GitRemoteOptions } from './git-remotes.js';

export interface GithubPrOptions extends GitRemoteOptions {ghExecutable?: string}
/** Same-repository GitHub.com PRs only. Forks and Enterprise are explicit unsupported results. */
export interface GithubPrTarget {remote: string; base: string; headBranch?: string}
export interface GithubRepository {host: 'github.com'; owner: string; name: string; fullName: string; url: string}
export interface GithubPullRequest {number: number; url: string; title: string; body: string; headRefName: string; baseRefName: string; headRefOid: string}
export interface GithubPrPreview {version: string; workspace: import('./git-workspace.js').GitWorkspaceVersion; repository: GithubRepository; remote: string; remoteUrl: string; headBranch: string; base: string; headOid: string; remoteOid: string | null; baseOid: string; pushed: boolean; existing: GithubPullRequest[]; sameRepositoryOnly: true}
export function githubRepositoryFromRemote(value: string): GithubRepository {
 let host: string, path: string;
 const scp = value.match(/^(?:git@)?([^/:]+):([^?#]+)$/);
 if (scp && !value.includes('://')) {host = scp[1]!; path = scp[2]!;}
 else {
  let url: URL; try {url = new URL(value);} catch {throw remoteError('NOT_GITHUB_REMOTE', 'Select a GitHub.com HTTPS or SSH remote');}
  if (!['https:', 'ssh:'].includes(url.protocol) || url.port || url.search || url.hash) throw remoteError('NOT_GITHUB_REMOTE', 'Select a GitHub.com HTTPS or SSH remote without port, query or fragment');
  host = url.hostname; path = url.pathname.replace(/^\//, '');
 }
 const match = path.replace(/\.git$/, '').match(/^([A-Za-z0-9](?:[A-Za-z0-9-]{0,38}))\/([A-Za-z0-9_.-]{1,100})$/);
 if (host.toLowerCase() !== 'github.com' || !match || ['.', '..'].includes(match[2]!)) throw remoteError('NOT_GITHUB_REMOTE', 'Only same-repository GitHub.com pull requests are supported; forks and Enterprise are not yet supported');
 const owner = match[1]!, name = match[2]!, fullName = owner + '/' + name;
 return {host: 'github.com', owner, name, fullName, url: 'https://github.com/' + fullName};
}
function prUrl(value: unknown, repository: GithubRepository): {url: string; number: number} {
 if (typeof value !== 'string') throw remoteError('INVALID_GITHUB_RESPONSE', 'GitHub did not return a pull request URL');
 let url: URL; try {url = new URL(value.trim());} catch {throw remoteError('INVALID_GITHUB_RESPONSE', 'GitHub did not return a valid pull request URL');}
 const match = url.pathname.match(/^\/([^/]+)\/([^/]+)\/pull\/([1-9]\d*)$/);
 if (url.protocol !== 'https:' || url.hostname !== repository.host || url.port || url.username || url.password || url.search || url.hash || !match || `${match[1]}/${match[2]}`.toLowerCase() !== repository.fullName.toLowerCase() || !Number.isSafeInteger(Number(match[3]))) throw remoteError('WRONG_GITHUB_REPOSITORY', 'The returned pull request URL does not belong to the selected repository');
 return {url: url.href, number: Number(match[3])};
}
function parseJson(text: string): any {try {return JSON.parse(text);} catch {throw remoteError('INVALID_GITHUB_RESPONSE', 'GitHub CLI returned invalid JSON');}}
function validatePr(value: any, repository: GithubRepository, head: string, base: string): GithubPullRequest {
 const identity = prUrl(value?.url, repository);
 if (identity.number !== value.number || value.headRefName !== head || value.baseRefName !== base || value.isCrossRepository !== false || value.headRepositoryOwner?.login?.toLowerCase() !== repository.owner.toLowerCase() || value.headRepository?.name?.toLowerCase() !== repository.name.toLowerCase() || typeof value.title !== 'string' || typeof value.body !== 'string' || !/^(?:[a-f0-9]{40}|[a-f0-9]{64})$/.test(value.headRefOid ?? '')) throw remoteError('WRONG_GITHUB_REPOSITORY', 'Pull request repository or branch identity does not match this preview');
 return {number: identity.number, url: identity.url, title: value.title, body: value.body, headRefName: head, baseRefName: base, headRefOid: value.headRefOid};
}
const fields = 'number,url,title,body,headRefName,baseRefName,headRefOid,headRepositoryOwner,headRepository,isCrossRepository';
async function withGh<T>(projectRoot: string, options: GithubPrOptions, operation: (gh: string, cwd: string) => Promise<T>): Promise<T> {
 const gh = await resolveRemoteExecutable('gh', projectRoot, options.ghExecutable);
 // No repository discovery or relative attachment lookup can route gh through the project.
 const directory = await mkdtemp(join(tmpdir(), 'hive-github-pr-'));
 try {return await operation(gh, directory);} finally {await rm(directory, {recursive: true, force: true});}
}
export async function previewGithubPr(projectRoot: string, target: GithubPrTarget, options: GithubPrOptions = {}): Promise<GithubPrPreview> {
 const context = await readGitRemote(projectRoot, target.remote, options), {workspace} = context;
 if (!workspace.branch || !workspace.headOid) throw remoteError('BRANCH_REQUIRED', 'A pull request requires an existing local branch and commit');
 const headBranch = target.headBranch ?? workspace.branch;
 if (typeof headBranch !== 'string' || typeof target.base !== 'string' || headBranch === target.base) throw remoteError('INVALID_PR_BRANCH', 'Select distinct head and base branches');
 const repository = githubRepositoryFromRemote(context.url);
 const remoteOid = await remoteBranchOid(context, headBranch, options);
 const result = await withGh(projectRoot, options, async (gh, cwd) => {
  const run = (args: string[]) => remoteCommand(gh, args, cwd, options.timeoutMs, true);
  const repo = parseJson((await run(['repo', 'view', repository.url, '--json', 'nameWithOwner,url'])).stdout);
  if (typeof repo.nameWithOwner !== 'string' || repo.nameWithOwner.toLowerCase() !== repository.fullName.toLowerCase() || typeof repo.url !== 'string' || repo.url.toLowerCase() !== repository.url.toLowerCase()) throw remoteError('WRONG_GITHUB_REPOSITORY', 'GitHub CLI resolved a different repository');
  // Validate base as a Git ref before including its encoded spelling in the API route.
  await remoteCommand(context.git, ['check-ref-format', `refs/heads/${target.base}`], workspace.repositoryRoot, options.timeoutMs);
  const base = parseJson((await run(['api', '--hostname', repository.host, `repos/${repository.fullName}/git/ref/heads/${encodeURIComponent(target.base)}`])).stdout);
  if (base.ref !== `refs/heads/${target.base}` || !/^(?:[a-f0-9]{40}|[a-f0-9]{64})$/.test(base.object?.sha ?? '')) throw remoteError('INVALID_GITHUB_RESPONSE', 'GitHub did not confirm the selected base branch');
  const listed = parseJson((await run(['pr', 'list', '--repo', `${repository.host}/${repository.fullName}`, '--state', 'open', '--head', headBranch, '--base', target.base, '--limit', '100', '--json', fields])).stdout);
  if (!Array.isArray(listed) || listed.length > 1) throw remoteError('AMBIGUOUS_PULL_REQUEST', 'More than one open pull request matches this head and base; inspect GitHub');
  const existing = listed.map(value => validatePr(value, repository, headBranch, target.base));
  return {baseOid: base.object.sha as string, existing, gh};
 });
 const after = await remoteWorkspaceVersion(projectRoot, context.git, options.timeoutMs);
 if (JSON.stringify(after) !== JSON.stringify(workspace)) throw remoteError('CONCURRENT_CHANGE', 'Local branch, HEAD, index or configuration changed during pull request preview');
 const payload = {workspace, repository, remote: target.remote, remoteUrl: context.displayUrl, headBranch, base: target.base, headOid: workspace.headOid, remoteOid, baseOid: result.baseOid, pushed: remoteOid === workspace.headOid, existing: result.existing, sameRepositoryOnly: true as const};
 return {version: remoteVersion({...payload, transportFingerprint: remoteVersion(context.url), gh: result.gh, git: context.git}), ...payload};
}
export interface GithubPrWriteRequest extends GithubPrTarget {expectedVersion: string; title: string; body: string}
export interface GithubPrWriteResult {ok: boolean; applied: true|'unknown'; operation: 'create'|'update'; url?: string; number?: number; error?: {code: string; message: string}}
function validateText(request: GithubPrWriteRequest): void {
 if (typeof request.title !== 'string' || !request.title.trim() || request.title.length > 500 || /[\x00-\x1f\x7f]/.test(request.title) || typeof request.body !== 'string' || request.body.includes('\0') || Buffer.byteLength(request.body) > 1024 * 1024) throw remoteError('INVALID_PR_TEXT', 'Supply a title of at most 500 characters and a body of at most 1 MiB');
}
async function writePr(operation: 'create'|'update', projectRoot: string, request: GithubPrWriteRequest & {number?: number; draft?: boolean}, options: GithubPrOptions): Promise<GithubPrWriteResult> {
 validateText(request); const preview = await previewGithubPr(projectRoot, request, options);
 if (request.expectedVersion !== preview.version) throw remoteError('STALE_PREVIEW', 'Repository, branch or pull request changed; refresh the preview');
 if (!preview.pushed) throw remoteError('PUSH_REQUIRED', 'Push the selected commit to the explicit head branch before creating or updating this pull request');
 if (operation === 'create' && preview.existing.length) throw remoteError('PR_ALREADY_EXISTS', `An open pull request already exists: ${preview.existing[0]!.url}`);
 if (operation === 'update' && (!Number.isSafeInteger(request.number) || preview.existing[0]?.number !== request.number)) throw remoteError('PR_NOT_IN_PREVIEW', 'Select the existing open pull request from this preview');
 if (request.draft !== undefined && typeof request.draft !== 'boolean') throw remoteError('INVALID_ARGUMENT', 'draft must be boolean');
 return withGh(projectRoot, options, async (gh, directory) => {
  const bodyFile = join(directory, 'body.md'); await writeFile(bodyFile, request.body, {mode: 0o600, flag: 'wx'});
  const repo = `${preview.repository.host}/${preview.repository.fullName}`;
  const args = operation === 'create' ? ['pr', 'create', '--repo', repo, '--head', preview.headBranch, '--base', preview.base, '--title', request.title, '--body-file', bodyFile, ...(request.draft ? ['--draft'] : [])]
   : ['pr', 'edit', String(request.number), '--repo', repo, '--title', request.title, '--body-file', bodyFile];
  const latest = await readGitRemote(projectRoot, request.remote, options);
  if (JSON.stringify(latest.workspace) !== JSON.stringify(preview.workspace)) throw remoteError('STALE_PREVIEW', 'Local branch, HEAD, index or configuration changed before the pull request write');
  let identity: {url: string; number: number} | undefined;
  try {
   const result = await remoteCommand(gh, args, directory, options.timeoutMs, true);
   identity = operation === 'create' ? prUrl(result.stdout.trim(), preview.repository) : {url: preview.existing[0]!.url, number: request.number!};
   const value = parseJson((await remoteCommand(gh, ['pr', 'view', String(identity.number), '--repo', repo, '--json', fields], directory, options.timeoutMs, true)).stdout);
   const observed = validatePr(value, preview.repository, preview.headBranch, preview.base);
   if (observed.number !== identity.number || observed.headRefOid !== preview.headOid || observed.title !== request.title || observed.body !== request.body) throw remoteError('PR_STATE_UNCERTAIN', 'The pull request was written but its exact content or head could not be confirmed; inspect GitHub before another action');
   return {ok: true, applied: true, operation, url: observed.url, number: observed.number};
  } catch (error) {
   return {ok: false, applied: 'unknown', operation, ...(identity && identity), error: {code: 'PR_STATE_UNCERTAIN', message: redactRemoteText(error instanceof Error ? error.message : String(error))}};
  }
 });
}
export function createGithubPr(projectRoot: string, request: GithubPrWriteRequest & {draft?: boolean}, options: GithubPrOptions = {}): Promise<GithubPrWriteResult> {return writePr('create', projectRoot, request, options);}
export function updateGithubPr(projectRoot: string, request: GithubPrWriteRequest & {number: number}, options: GithubPrOptions = {}): Promise<GithubPrWriteResult> {return writePr('update', projectRoot, request, options);}

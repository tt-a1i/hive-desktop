import { randomUUID } from 'node:crypto';
import { lstat } from 'node:fs/promises';
import { join } from 'node:path';
import { checkoutGitBranch } from './git-branches-stash.js';
import { getGitWorkspaceStatus, GitWorkspaceError } from './git-workspace.js';
import {
  gitRemoteArgs, readGitRemote, remoteCommand, remoteError, remoteVersion,
  remoteWorkspaceVersion, type GitRemoteContext,
} from './git-remotes.js';
import {
  githubRepositoryFromRemote, parseJson, prUrl, withGh,
  type GithubPrOptions,
} from './github-prs.js';

const commitOid = /^[a-f0-9]{40}(?:[a-f0-9]{24})?$/;

export interface GithubPrReviewTarget {
  remote: string;
  reference: string;
}

export interface GithubPrReviewSeed {
  title: string;
  prompt: string;
}

export function isGithubPrReviewSeed(value: unknown): value is GithubPrReviewSeed {
  if (!value || typeof value !== 'object') return false;
  const seed = value as Partial<GithubPrReviewSeed>;
  return typeof seed.title === 'string' && seed.title.length <= 512 && /^PR #[1-9]\d*/.test(seed.title)
    && typeof seed.prompt === 'string' && seed.prompt.length <= 4096 && seed.prompt.startsWith('Review GitHub pull request #');
}

function pullNumber(reference: string, repository: ReturnType<typeof githubRepositoryFromRemote>): number {
  const input = reference.trim();
  if (input.length > 512) throw remoteError('INVALID_PR_REFERENCE', 'Enter a GitHub pull request URL or positive number');
  if (!/^#?[1-9]\d*$/.test(input)) {
    // Web links copied from the Files or Commits tab resolve to the same PR.
    return prUrl(input.replace(/\/(?:files|commits|checks)\/?$|\/$/, ''), repository).number;
  }
  const number = Number(input.replace(/^#/, ''));
  if (!Number.isSafeInteger(number)) throw remoteError('INVALID_PR_REFERENCE', 'Enter a GitHub pull request URL or positive number');
  return number;
}

async function cleanCheckout(context: GitRemoteContext, options: GithubPrOptions) {
  const {workspace, git} = context;
  const status = await getGitWorkspaceStatus(workspace.repositoryRoot, {
    gitExecutable: git, ...(options.timeoutMs !== undefined && {timeoutMs: Math.min(options.timeoutMs, 30_000)}), limit: 1,
  }).catch((error: unknown) => {
    if (error instanceof GitWorkspaceError && error.code === 'UNSUPPORTED_FILTER') return undefined;
    throw error;
  });
  // Repository-wide status includes files outside a nested workspace.
  if (!status?.complete || status.total || status.headOid !== workspace.headOid || status.branch !== workspace.branch) return false;
  const operation = await Promise.all(
    ['MERGE_HEAD', 'rebase-apply', 'rebase-merge', 'CHERRY_PICK_HEAD', 'REVERT_HEAD', 'sequencer', 'BISECT_START'].map(async state => {
      try { await lstat(join(workspace.gitDir, state)); return true; }
      catch (error) { if ((error as NodeJS.ErrnoException).code !== 'ENOENT') throw error; return false; }
    }),
  );
  if (operation.some(Boolean)) return false;
  const run = (args: string[], accepted = [0]) => remoteCommand(git, gitRemoteArgs(args),
    workspace.repositoryRoot, options.timeoutMs, false, accepted);
  const index = (await run(['ls-files', '-v', '-z'])).stdout;
  // Git status intentionally omits assume-unchanged and skip-worktree edits.
  if (/(?:^|\x00)[a-zS] /.test(index)) return false;
  return (await run(['diff', '--quiet', '--ignore-submodules=none', '--no-ext-diff', '--no-textconv', '--'], [0, 1])).code === 0;
}

async function resolveReview(projectRoot: string, target: GithubPrReviewTarget, options: GithubPrOptions) {
  const context = await readGitRemote(projectRoot, target.remote, options);
  const {workspace} = context;
  const repository = githubRepositoryFromRemote(context.url);
  const number = pullNumber(target.reference, repository);
  const response = await withGh(projectRoot, options, async (gh, cwd) => parseJson(
    (await remoteCommand(gh, ['api', '--hostname', repository.host, '--method', 'GET',
      `repos/${repository.fullName}/pulls/${number}`], cwd, options.timeoutMs, true)).stdout,
  ));
  const identity = prUrl(response?.html_url, repository);
  if (identity.number !== number || response.number !== number || typeof response.base?.repo?.full_name !== 'string'
    || response.base.repo.full_name.toLowerCase() !== repository.fullName.toLowerCase()) {
    throw remoteError('WRONG_GITHUB_REPOSITORY', 'GitHub returned a different repository or pull request number');
  }
  if (typeof response.title !== 'string' || typeof response.base.ref !== 'string' || typeof response.head?.ref !== 'string'
    || typeof response.base.sha !== 'string' || !commitOid.test(response.base.sha)
    || typeof response.head.sha !== 'string' || !commitOid.test(response.head.sha)
    || !['open', 'closed'].includes(response.state) || typeof response.merged !== 'boolean') {
    throw remoteError('INVALID_GITHUB_RESPONSE', 'GitHub did not confirm the pull request branches and commits');
  }
  const title = [...response.title.replace(/[\x00-\x1f\x7f]/g, ' ').trim()].slice(0, 225).join('');
  const preview = {
    ...identity, repository, title, baseBranch: response.base.ref as string, headBranch: response.head.ref as string,
    baseOid: response.base.sha as string, headOid: response.head.sha as string,
    state: (response.merged ? 'merged' : response.state) as 'open' | 'closed' | 'merged',
    currentHead: workspace.headOid, currentBranch: workspace.branch, currentClean: await cleanCheckout(context, options),
  };
  await assertReviewWorkspaceUnchanged(projectRoot, context, options);
  return {context, preview: {...preview, version: remoteVersion([preview, workspace, remoteVersion(context.url)])}};
}

async function assertReviewWorkspaceUnchanged(projectRoot: string, context: GitRemoteContext, options: GithubPrOptions) {
  const after = await remoteWorkspaceVersion(projectRoot, context.git, options.timeoutMs);
  if (JSON.stringify(after) !== JSON.stringify(context.workspace)) {
    throw remoteError('STALE_PREVIEW', 'The local repository changed. Preview the pull request again.');
  }
}

export async function previewGithubPrReview(projectRoot: string, target: GithubPrReviewTarget, options: GithubPrOptions = {}) {
  return (await resolveReview(projectRoot, target, options)).preview;
}

export async function prepareGithubPrReview(
  projectRoot: string, target: GithubPrReviewTarget & {expectedVersion: string; checkout: 'current' | 'worktree'},
  options: GithubPrOptions = {},
) {
  if (!/^[a-f0-9]{64}$/.test(target.expectedVersion)) {
    throw remoteError('STALE_PREVIEW', 'Preview the pull request before opening a review.');
  }
  const {context, preview} = await resolveReview(projectRoot, target, options);
  if (preview.version !== target.expectedVersion) throw remoteError('STALE_PREVIEW', 'The pull request or checkout changed. Preview it again.');
  if (target.checkout === 'current' && !preview.currentClean) {
    throw remoteError('DIRTY_CHECKOUT', 'The project checkout has uncommitted changes. Use an independent worktree or clean the checkout first.');
  }
  const {git, workspace} = context;
  const temporaryRef = `refs/hive/pr-review/${randomUUID()}`;
  const run = (args: string[], accepted = [0]) =>
    remoteCommand(git, gitRemoteArgs(args), workspace.repositoryRoot, options.timeoutMs, false, accepted);
  try {
    // A base repository exposes the PR head even when the author used a fork.
    // An isolated destination lets us verify which head Git actually fetched.
    await run(['-c', `core.hooksPath=${process.platform === 'win32' ? 'NUL' : '/dev/null'}`,
      '-c', 'protocol.allow=never', '-c', 'protocol.https.allow=always', '-c', 'protocol.ssh.allow=always',
      '-c', 'fetch.writeCommitGraph=false', 'fetch', '--no-write-fetch-head', '--no-tags', '--no-prune',
      '--no-recurse-submodules', '--no-auto-maintenance', '--refmap=', '--', context.remote,
      `refs/pull/${preview.number}/head:${temporaryRef}`, preview.baseOid]);
    const fetchedHead = (await run(['rev-parse', '--verify', `${temporaryRef}^{commit}`])).stdout.trim();
    const fetchedBase = (await run(['rev-parse', '--verify', `${preview.baseOid}^{commit}`])).stdout.trim();
    if (fetchedHead !== preview.headOid || fetchedBase !== preview.baseOid) {
      throw remoteError('STALE_PREVIEW', 'The pull request head changed during fetch. Preview it again.');
    }
    await assertReviewWorkspaceUnchanged(projectRoot, context, options);
    if (target.checkout === 'current' && !await cleanCheckout(context, options)) {
      throw remoteError('DIRTY_CHECKOUT', 'The project checkout changed during fetch. Preview it again.');
    }
    const seed: GithubPrReviewSeed = {
      title: `PR #${preview.number}${preview.title ? ` · ${preview.title}` : ''}`,
      prompt: [
        `Review GitHub pull request #${preview.number} (${preview.state}) in ${preview.repository.fullName}.`,
        preview.url,
        `Base commit: ${preview.baseOid}`,
        `Head commit: ${preview.headOid}`,
        '',
        'Review these commits even if the local branch has moved. Start with the PR diff and read relevant code. Report actionable findings with file and line references, ordered by severity.',
        `Compare the stated commits with git diff ${preview.baseOid}...${preview.headOid}. For a merged pull request or an unavailable local comparison, use the PR diff on GitHub.`,
        'Keep the review in this conversation. Do not edit files or post to GitHub unless I ask.',
      ].join('\n'),
    };
    return {context, preview, seed};
  } finally {
    await run(['-c', `core.hooksPath=${process.platform === 'win32' ? 'NUL' : '/dev/null'}`,
      'update-ref', '-d', temporaryRef]);
  }
}

export async function checkoutGithubPrReview(
  projectRoot: string, review: Awaited<ReturnType<typeof prepareGithubPrReview>>, branch: string, options: GithubPrOptions = {},
) {
  const {preview, context} = review;
  const {workspace, git} = context;
  if (!await cleanCheckout(context, options)) {
    throw remoteError('DIRTY_CHECKOUT', 'The project checkout has uncommitted changes or an unfinished Git operation. Use an independent worktree or clean the checkout first.');
  }
  await assertReviewWorkspaceUnchanged(projectRoot, context, options);
  if (workspace.headOid === preview.headOid) return;
  if (workspace.projectPrefix) {
    const prefix = workspace.projectPrefix.replace(/\/$/, '');
    const directory = await remoteCommand(git, gitRemoteArgs(['cat-file', '-t', `${preview.headOid}:${prefix}`]),
      workspace.repositoryRoot, options.timeoutMs, false, [0, 128]);
    if (directory.code !== 0 || directory.stdout.trim() !== 'tree') {
      throw remoteError('PROJECT_NOT_IN_PR', 'The project directory is missing from the pull request head');
    }
  }
  const checkedOut = await checkoutGitBranch(projectRoot, {branch, create: true, startPoint: preview.headOid},
    {gitExecutable: git, expectedWorkspace: workspace});
  if (checkedOut.headOid !== preview.headOid) {
    throw remoteError('CHECKOUT_CHANGED', 'The checkout changed during branch creation. Verify the project branch before retrying.');
  }
}

import { execFile, spawn } from 'node:child_process';
import { getGitWorkspaceVersion, GitWorkspaceError, type GitReadOptions, type GitWorkspaceVersion } from './git-workspace.js';

const OUTPUT_LIMIT = 4 * 1024 * 1024;
const NULL_FILE = process.platform === 'win32' ? 'NUL' : '/dev/null';
const VALID_STASH_REF = /^stash@\{\d+\}$/;
const VALID_REMOTE_NAME = /^[A-Za-z0-9._/-]{1,128}$/;

export interface GitLocalBranch {
  name: string;
  oid: string;
  isCurrent: boolean;
  upstream?: string;
  ahead: number;
  behind: number;
  subject?: string;
}

export interface GitRemoteBranch {
  name: string;
  oid: string;
}

export interface GitBranchesSnapshot {
  current: string | null;
  detached: boolean;
  headOid: string | null;
  local: GitLocalBranch[];
  remote: GitRemoteBranch[];
}

export interface GitStashEntry {
  ref: string;
  index: number;
  oid: string;
  message: string;
  relativeTime: string;
}

export interface GitBranchActionResult {
  ok: true;
  branch: string;
  headOid: string | null;
  created: boolean;
}

export interface GitSyncActionResult {
  ok: true;
  operation: 'fetch' | 'pull';
  summary: string;
  headOid: string | null;
}

export interface GitStashActionResult {
  ok: true;
  operation: 'push' | 'pop' | 'drop';
  summary: string;
}

export interface GitCommitSummary {
  oid: string;
  shortOid: string;
  author: string;
  relativeTime: string;
  subject: string;
}

export interface GitCommitDiffResult {
  oid: string;
  patch: string;
  truncated: boolean;
}

export interface GitDiscardResult {
  ok: true;
  path: string;
  summary: string;
}

const VALID_COMMIT_OID = /^[0-9a-fA-F]{7,40}$/;
const MAX_COMMIT_DIFF_CHARS = 256 * 1024;

function gitEnv(allowNetwork: boolean): NodeJS.ProcessEnv {
  const env = { ...process.env };
  const config = {
    GIT_CONFIG_GLOBAL: env.GIT_CONFIG_GLOBAL,
    GIT_CONFIG_SYSTEM: env.GIT_CONFIG_SYSTEM,
    GIT_CONFIG_NOSYSTEM: env.GIT_CONFIG_NOSYSTEM,
    ...(allowNetwork && env.GIT_SSH_COMMAND ? { GIT_SSH_COMMAND: env.GIT_SSH_COMMAND } : {}),
    ...(allowNetwork && env.SSH_AUTH_SOCK ? { SSH_AUTH_SOCK: env.SSH_AUTH_SOCK } : {}),
  };
  for (const key of Object.keys(env)) {
    if (key.startsWith('GIT_')) delete env[key];
  }
  return {
    ...env,
    ...config,
    LC_ALL: 'C',
    GIT_TERMINAL_PROMPT: '0',
    GIT_OPTIONAL_LOCKS: '0',
    GIT_PAGER: 'cat',
    ...(!allowNetwork && {
      GIT_NO_LAZY_FETCH: '1',
      GIT_ALLOW_PROTOCOL: '',
      GIT_PROTOCOL_FROM_USER: '0',
    }),
  };
}

async function runGit(
  workspace: GitWorkspaceVersion,
  args: string[],
  options: GitReadOptions & { allowNetwork?: boolean; defaultTimeoutMs?: number } = {},
): Promise<{ stdout: string; stderr: string }> {
  const executable = options.gitExecutable ?? 'git';
  const timeout = options.timeoutMs ?? options.defaultTimeoutMs ?? (options.allowNetwork ? 30_000 : 10_000);
  const clampedTimeout = Math.min(Math.max(1000, Math.floor(timeout)), 60_000);
  const allowNetwork = options.allowNetwork === true;

  return new Promise((resolveResult, reject) => {
    let timedOut = false;
    let limitExceeded = false;
    let outputBytes = 0;
    let startFault: NodeJS.ErrnoException | undefined;
    let timer: ReturnType<typeof setTimeout> | undefined;
    let escalation: ReturnType<typeof setTimeout> | undefined;
    const stdout: Buffer[] = [];
    const stderr: Buffer[] = [];

    const gitArgs = [
      '--no-pager',
      '-c', 'core.fsmonitor=false',
      '-c', `core.hooksPath=${NULL_FILE}`,
      '-c', 'maintenance.auto=false',
      '-c', 'gc.auto=0',
      ...args,
    ];
    const owner = process.platform === 'darwin' ? process.env.HIVE_DESKTOP_SHELL_OWNER : undefined;
    const child = spawn(
      owner ?? executable,
      owner ? ['--hive-owned-cli', String(process.pid), executable, ...gitArgs] : gitArgs,
      {
        cwd: workspace.repositoryRoot,
        env: gitEnv(allowNetwork),
        detached: process.platform !== 'win32',
        windowsHide: true,
        stdio: 'pipe',
      },
    );

    const stop = (signal: NodeJS.Signals) => {
      if (!child.pid) return;
      if (owner) {
        child.kill('SIGTERM');
        return;
      }
      if (process.platform === 'win32') {
        execFile('taskkill', ['/PID', String(child.pid), '/T', '/F'], { windowsHide: true }, () => {});
        return;
      }
      try {
        process.kill(-child.pid, signal);
      } catch {
        child.kill(signal);
      }
    };

    const terminate = () => {
      stop('SIGTERM');
      if (!owner) {
        clearTimeout(escalation);
        escalation = setTimeout(() => stop('SIGKILL'), 250);
        escalation.unref();
      }
    };

    const pushChunk = (parts: Buffer[], data: Buffer) => {
      const remaining = OUTPUT_LIMIT - outputBytes;
      outputBytes += data.length;
      if (remaining > 0) parts.push(data.subarray(0, remaining));
      if (outputBytes > OUTPUT_LIMIT && !limitExceeded) {
        limitExceeded = true;
        terminate();
      }
    };

    child.stdout.on('data', (chunk: Buffer) => pushChunk(stdout, chunk));
    child.stderr.on('data', (chunk: Buffer) => pushChunk(stderr, chunk));
    child.once('error', fault => { startFault = fault; });

    child.once('close', code => {
      clearTimeout(timer);
      clearTimeout(escalation);
      const outText = Buffer.concat(stdout).toString('utf8');
      const errText = Buffer.concat(stderr).toString('utf8').slice(0, 1500).replace(/[\x00-\x08\x0b-\x1f\x7f]/g, '').trim();
      if (!startFault && !timedOut && !limitExceeded && code === 0) {
        resolveResult({ stdout: outText, stderr: errText });
        return;
      }
      const kind = timedOut
        ? 'GIT_TIMEOUT'
        : limitExceeded
          ? 'OUTPUT_LIMIT'
          : startFault?.code === 'ENOENT'
            ? 'GIT_NOT_FOUND'
            : 'GIT_OPERATION_FAILED';
      reject(new GitWorkspaceError(kind, errText || outText.trim() || `Git 操作${timedOut ? '超时' : '失败'}`));
    });

    timer = setTimeout(() => {
      timedOut = true;
      terminate();
    }, clampedTimeout);
    timer.unref();
    child.stdin.end();
  });
}

function parseTrackCounts(track: string): { ahead: number; behind: number } {
  const aheadMatch = track.match(/ahead\s+(\d+)/i);
  const behindMatch = track.match(/behind\s+(\d+)/i);
  return {
    ahead: aheadMatch ? Number.parseInt(aheadMatch[1]!, 10) : 0,
    behind: behindMatch ? Number.parseInt(behindMatch[1]!, 10) : 0,
  };
}

export async function listGitBranches(
  projectRoot: string,
  options: GitReadOptions = {},
): Promise<GitBranchesSnapshot> {
  const workspace = await getGitWorkspaceVersion(projectRoot, options);
  const localOut = await runGit(
    workspace,
    [
      'for-each-ref',
      '--sort=-HEAD',
      '--sort=-committerdate',
      '--format=%(HEAD)%00%(refname:short)%00%(objectname:short)%00%(upstream:short)%00%(upstream:track,nobracket)%00%(contents:subject)',
      'refs/heads',
    ],
    options,
  );
  const local: GitLocalBranch[] = [];
  let current: string | null = null;
  for (const line of localOut.stdout.split('\n')) {
    if (!line.trim()) continue;
    const parts = line.split('\0');
    const headMark = parts[0] ?? '';
    const name = parts[1] ?? '';
    const oid = parts[2] ?? '';
    const upstream = parts[3]?.trim() || undefined;
    const track = parts[4] ?? '';
    const subject = parts[5]?.trim() || undefined;
    if (!name) continue;
    const isCurrent = headMark === '*';
    if (isCurrent) current = name;
    const { ahead, behind } = parseTrackCounts(track);
    local.push({
      name,
      oid,
      isCurrent,
      ...(upstream && { upstream }),
      ahead,
      behind,
      ...(subject && { subject }),
    });
  }

  const remoteOut = await runGit(
    workspace,
    [
      'for-each-ref',
      '--sort=-committerdate',
      '--format=%(refname:short)%00%(objectname:short)',
      'refs/remotes',
    ],
    options,
  );
  const remote: GitRemoteBranch[] = [];
  for (const line of remoteOut.stdout.split('\n')) {
    if (!line.trim()) continue;
    const [name, oid] = line.split('\0');
    if (!name || name.endsWith('/HEAD')) continue;
    remote.push({ name, oid: oid ?? '' });
  }

  return {
    current,
    detached: current === null && workspace.headOid !== null,
    headOid: workspace.headOid,
    local,
    remote: remote.slice(0, 200),
  };
}

export async function checkoutGitBranch(
  projectRoot: string,
  request: { branch: string; create?: boolean; startPoint?: string },
  options: GitReadOptions = {},
): Promise<GitBranchActionResult> {
  const branch = request.branch.trim();
  if (!branch || branch.startsWith('-') || branch.length > 255) {
    throw new GitWorkspaceError('INVALID_ARGUMENT', '分支名称无效');
  }
  const workspace = await getGitWorkspaceVersion(projectRoot, options);
  // Validate branch name syntax via Git itself
  await runGit(workspace, ['check-ref-format', '--branch', branch], options);

  if (request.create) {
    const startPoint = request.startPoint?.trim();
    if (startPoint && startPoint.startsWith('-')) {
      throw new GitWorkspaceError('INVALID_ARGUMENT', '起始提交引用无效');
    }
    const args = ['switch', '-c', branch, ...(startPoint ? [startPoint] : [])];
    await runGit(workspace, args, options);
  } else {
    await runGit(workspace, ['switch', branch], options);
  }

  const updated = await getGitWorkspaceVersion(projectRoot, options);
  return {
    ok: true,
    branch,
    headOid: updated.headOid,
    created: request.create === true,
  };
}

export async function fetchGitRemote(
  projectRoot: string,
  request: { remote?: string; prune?: boolean } = {},
  options: GitReadOptions = {},
): Promise<GitSyncActionResult> {
  const remote = request.remote?.trim();
  if (remote && (!VALID_REMOTE_NAME.test(remote) || remote.startsWith('-'))) {
    throw new GitWorkspaceError('INVALID_ARGUMENT', '远端名称无效');
  }
  const workspace = await getGitWorkspaceVersion(projectRoot, options);
  const args = ['fetch', ...(request.prune !== false ? ['--prune'] : []), ...(remote ? [remote] : ['--all'])];
  const res = await runGit(workspace, args, { ...options, allowNetwork: true });
  const updated = await getGitWorkspaceVersion(projectRoot, options);
  const summary = res.stderr || res.stdout.trim() || '已完成远端拉取（Fetch）';
  return {
    ok: true,
    operation: 'fetch',
    summary,
    headOid: updated.headOid,
  };
}

export async function pullGitBranch(
  projectRoot: string,
  request: { rebase?: boolean } = {},
  options: GitReadOptions = {},
): Promise<GitSyncActionResult> {
  const workspace = await getGitWorkspaceVersion(projectRoot, options);
  const args = request.rebase ? ['pull', '--rebase'] : ['pull', '--ff-only'];
  const res = await runGit(workspace, args, { ...options, allowNetwork: true });
  const updated = await getGitWorkspaceVersion(projectRoot, options);
  const summary = res.stdout.trim() || res.stderr || '当前分支已是最新';
  return {
    ok: true,
    operation: 'pull',
    summary,
    headOid: updated.headOid,
  };
}

export async function listGitStashes(
  projectRoot: string,
  options: GitReadOptions = {},
): Promise<{ stashes: GitStashEntry[] }> {
  const workspace = await getGitWorkspaceVersion(projectRoot, options);
  const out = await runGit(
    workspace,
    ['stash', 'list', '--format=%gd%x00%h%x00%gs%x00%cr'],
    options,
  );
  const stashes: GitStashEntry[] = [];
  let idx = 0;
  for (const line of out.stdout.split('\n')) {
    if (!line.trim()) continue;
    const [ref, oid, message, relativeTime] = line.split('\0');
    if (!ref) continue;
    stashes.push({
      ref,
      index: idx++,
      oid: oid ?? '',
      message: message ?? '',
      relativeTime: relativeTime ?? '',
    });
  }
  return { stashes };
}

export async function pushGitStash(
  projectRoot: string,
  request: { message?: string; includeUntracked?: boolean } = {},
  options: GitReadOptions = {},
): Promise<GitStashActionResult> {
  const workspace = await getGitWorkspaceVersion(projectRoot, options);
  const msg = request.message?.trim();
  const args = [
    'stash',
    'push',
    ...(request.includeUntracked !== false ? ['--include-untracked'] : []),
    ...(msg ? ['-m', msg.slice(0, 200)] : []),
  ];
  const res = await runGit(workspace, args, options);
  const summary = res.stdout.trim() || res.stderr || '已暂存当前改动到 Stash';
  return {
    ok: true,
    operation: 'push',
    summary,
  };
}

export async function popGitStash(
  projectRoot: string,
  request: { ref: string },
  options: GitReadOptions = {},
): Promise<GitStashActionResult> {
  const ref = request.ref.trim();
  if (!VALID_STASH_REF.test(ref)) {
    throw new GitWorkspaceError('INVALID_ARGUMENT', '无效的 Stash 引用');
  }
  const workspace = await getGitWorkspaceVersion(projectRoot, options);
  const res = await runGit(workspace, ['stash', 'pop', ref], options);
  const summary = res.stdout.trim() || res.stderr || `已弹出 ${ref}`;
  return {
    ok: true,
    operation: 'pop',
    summary,
  };
}

export async function dropGitStash(
  projectRoot: string,
  request: { ref: string },
  options: GitReadOptions = {},
): Promise<GitStashActionResult> {
  const ref = request.ref.trim();
  if (!VALID_STASH_REF.test(ref)) {
    throw new GitWorkspaceError('INVALID_ARGUMENT', '无效的 Stash 引用');
  }
  const workspace = await getGitWorkspaceVersion(projectRoot, options);
  const res = await runGit(workspace, ['stash', 'drop', ref], options);
  const summary = res.stdout.trim() || res.stderr || `已删除 ${ref}`;
  return {
    ok: true,
    operation: 'drop',
    summary,
  };
}

function assertSafeRelativePath(filePath: string): string {
  const normalized = filePath.replace(/\\/g, '/').trim();
  if (
    !normalized ||
    normalized.startsWith('/') ||
    normalized.startsWith('-') ||
    /^[A-Za-z]:/.test(normalized) ||
    normalized.includes('\0') ||
    normalized.split('/').some(part => !part || part === '.' || part === '..' || part.toLowerCase() === '.git')
  ) {
    throw new GitWorkspaceError('INVALID_ARGUMENT', '文件路径不合法或超出项目范围');
  }
  return normalized;
}

export async function discardGitFile(
  projectRoot: string,
  request: { path: string },
  options: GitReadOptions = {},
): Promise<GitDiscardResult> {
  const relPath = assertSafeRelativePath(request.path);
  const workspace = await getGitWorkspaceVersion(projectRoot, options);
  const repoRelPath = workspace.projectPrefix + relPath;
  await runGit(workspace, ['checkout', '--', repoRelPath], options);
  return {
    ok: true,
    path: relPath,
    summary: `已放弃 ${relPath} 的未暂存修改`,
  };
}

export async function listGitCommits(
  projectRoot: string,
  request: { limit?: number } = {},
  options: GitReadOptions = {},
): Promise<{ commits: GitCommitSummary[] }> {
  const limit = Math.min(Math.max(1, Math.floor(request.limit ?? 30)), 100);
  const workspace = await getGitWorkspaceVersion(projectRoot, options);
  if (!workspace.headOid) {
    return { commits: [] };
  }
  const out = await runGit(
    workspace,
    ['log', `-n`, String(limit), '--format=%H%x00%h%x00%an%x00%cr%x00%s'],
    options,
  );
  const commits: GitCommitSummary[] = [];
  for (const line of out.stdout.split('\n')) {
    if (!line.trim()) continue;
    const [oid, shortOid, author, relativeTime, subject] = line.split('\0');
    if (!oid) continue;
    commits.push({
      oid,
      shortOid: shortOid ?? oid.slice(0, 7),
      author: author ?? '',
      relativeTime: relativeTime ?? '',
      subject: subject ?? '',
    });
  }
  return { commits };
}

export async function getGitCommitDiff(
  projectRoot: string,
  request: { oid: string },
  options: GitReadOptions = {},
): Promise<GitCommitDiffResult> {
  const oid = request.oid.trim();
  if (!VALID_COMMIT_OID.test(oid)) {
    throw new GitWorkspaceError('INVALID_ARGUMENT', '无效的提交哈希');
  }
  const workspace = await getGitWorkspaceVersion(projectRoot, options);
  const out = await runGit(
    workspace,
    ['show', '--stat', '--patch', '--unified=3', '--no-ext-diff', '--no-textconv', oid],
    options,
  );
  const truncated = out.stdout.length > MAX_COMMIT_DIFF_CHARS;
  const patch = truncated
    ? `${out.stdout.slice(0, MAX_COMMIT_DIFF_CHARS)}\n… [提交补丁已截断至 256 KB 以保持流畅]`
    : out.stdout;
  return { oid, patch, truncated };
}

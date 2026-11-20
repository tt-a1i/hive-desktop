import { lstat, realpath } from 'node:fs/promises';
import { homedir } from 'node:os';
import { isAbsolute, join } from 'node:path';

const uuid = /^[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}$/i;

/**
 * Recognize the first local path selected by the pinned SDK 0.3.317.
 * Its alternate project directories, worktrees and long-path search remain
 * available through the official reader, without a seekable source here.
 */
export async function resolveClaudeTranscript(sessionId: string, cwd: string, env: NodeJS.ProcessEnv, retainedWorktreeCwd = false): Promise<string | undefined> {
  if (!uuid.test(sessionId) || !isAbsolute(cwd) || env.CLAUDE_CODE_PROJECT_DIR_NAME) return;
  if (process.platform === 'win32' && Object.keys(env).some(key =>
    ['CLAUDE_CONFIG_DIR', 'CLAUDE_CODE_PROJECT_DIR_NAME', 'USERPROFILE'].includes(key.toUpperCase()) && key !== key.toUpperCase())) return;
  const userHome = (process.platform === 'win32' ? env.USERPROFILE : env.HOME) ?? homedir();
  const config = (env.CLAUDE_CONFIG_DIR ?? join(userHome, '.claude')).normalize('NFC');
  if (!isAbsolute(config)) return;
  try {
    // A released record keeps the canonical cwd captured by the managed
    // worktree. History can outlive that directory and must ignore replacements.
    const realCwd = retainedWorktreeCwd ? cwd : await realpath(cwd);
    const nativeCwd = process.platform === 'darwin' ? realCwd.normalize('NFC') : realCwd;
    const project = nativeCwd.replace(/[^a-zA-Z0-9]/g, '-');
    if (project.length > 200) return;
    const path = join(config, 'projects', project, `${sessionId}.jsonl`);
    const info = await lstat(path);
    if (!info.isFile() || info.size === 0) return;
    const resolved = await realpath(path);
    if (await realpath(config) !== config || (process.platform === 'darwin' ? resolved.normalize('NFC') : resolved) !== path) return;
    return path;
  } catch (error) {
    if (['ENOENT', 'ENOTDIR', 'EACCES', 'EPERM'].includes((error as NodeJS.ErrnoException).code ?? '')) return;
    throw error;
  }
}

/** Check the exact imported input on both sides of SDK reads. Appends keep its identity. */
export async function claudeTranscriptIdentity(path: string, sessionId: string, cwd: string, env: NodeJS.ProcessEnv, retainedWorktreeCwd = false): Promise<string> {
  if (await resolveClaudeTranscript(sessionId, cwd, env, retainedWorktreeCwd) !== path)
    throw new Error('The imported Claude conversation’s original transcript is unavailable');
  const info = await lstat(path, {bigint: true});
  if (!info.isFile() || info.size === 0n)
    throw new Error('The imported Claude conversation’s original transcript is unavailable');
  return `${info.dev}:${info.ino}`;
}

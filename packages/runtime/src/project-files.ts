import { execFile } from 'node:child_process';
import { constants } from 'node:fs';
import { lstat, open, readdir, realpath, stat } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { WORKSPACE_UPLOAD_MAX_BYTES } from '@tt-a1i/hive/dist/src/server/upload-limits.js';
import { isAbsolute, join, relative, resolve, sep } from 'node:path';
import { promisify } from 'node:util';

const exec = promisify(execFile);
const PAGE_SIZE = 100;
const EXCLUDED = new Set(['.git', 'node_modules', 'vendor', 'target', 'dist', 'build', '.next', '.venv', 'venv', '__pycache__']);

function inside(root: string, path: string): boolean {
  const rel = relative(root, path);
  return rel !== '' && rel !== '..' && !rel.startsWith(`..${sep}`) && !isAbsolute(rel);
}
function validRelative(path: string): boolean {
  return !!path && !path.includes('\0') && !isAbsolute(path) && !/^[A-Za-z]:/.test(path)
    && !path.split(/[\\/]/).some(part => !part || part === '.' || part === '..' || part === '.git');
}
async function rootDirectory(root: string): Promise<string> {
  const canonical = await realpath(root);
  if (!(await stat(canonical)).isDirectory()) throw new Error('Project root must be a directory');
  return canonical;
}

/** Resolves the current file target. Callers must independently secure any later open
 * against replacement races; a path alone cannot guarantee that later access is safe. */
export async function resolveProjectFile(projectRoot: string, relativePath: string): Promise<string> {
  if (typeof relativePath !== 'string' || !validRelative(relativePath)) throw new TypeError('Invalid project file path');
  const root = await rootDirectory(projectRoot);
  const path = resolve(root, relativePath);
  if (!inside(root, path)) throw new Error('File is outside project');
  const canonical = await realpath(path);
  if (!inside(root, canonical) || !validRelative(relative(root, canonical)) || !(await stat(canonical)).isFile()) throw new Error('File is outside project or not a regular file');
  return canonical;
}

async function gitFiles(root: string): Promise<string[] | null> {
  const options = { cwd: root, maxBuffer: 64 * 1024 * 1024, env: { ...process.env, LC_ALL: 'C', GIT_OPTIONAL_LOCKS: '0', GIT_CONFIG_NOSYSTEM: '1', GIT_CONFIG_GLOBAL: process.platform === 'win32' ? 'NUL' : '/dev/null' } };
  const prefix = ['-c', 'core.fsmonitor=false', '-c', 'core.hooksPath=/dev/null'];
  try {
    await exec('git', [...prefix, 'rev-parse', '--is-inside-work-tree'], options);
  } catch (error) {
    const e = error as { code?: string | number; stderr?: string };
    if (e.code === 'ENOENT' || (e.code === 128 && e.stderr?.includes('not a git repository'))) return null;
    throw error;
  }
  const { stdout } = await exec('git', [...prefix, 'ls-files', '--cached', '--others', '--exclude-standard', '-z', '--', '.'], options);
  return stdout.split('\0').filter(Boolean);
}
async function walk(root: string, current = ''): Promise<string[]> {
  const result: string[] = [];
  for (const entry of await readdir(join(root, current), { withFileTypes: true })) {
    if (EXCLUDED.has(entry.name) || entry.isSymbolicLink()) continue;
    const path = join(current, entry.name);
    if (entry.isDirectory()) result.push(...await walk(root, path));
    else if (entry.isFile()) result.push(path);
  }
  return result;
}

/** Search is uncached; each request observes the current file list and ignore rules. */
export async function searchProjectFiles(projectRoot: string, query: string, offset = 0): Promise<{ paths: string[]; total: number }> {
  if (typeof query !== 'string' || query.length > 200 || /[\0\r\n]/.test(query)) throw new TypeError('Invalid project file query');
  if (!Number.isSafeInteger(offset) || offset < 0) throw new TypeError('Invalid project file offset');
  const root = await rootDirectory(projectRoot);
  const candidates = await gitFiles(root) ?? await walk(root);
  const matches: string[] = [];
  const needle = query.toLowerCase();
  for (const candidate of new Set(candidates)) {
    const path = candidate.split(sep).join('/');
    if (!validRelative(path) || !path.toLowerCase().includes(needle)) continue;
    try {
      // Never traverse symlink directories; ignore stale deleted Git index entries.
      const parts = path.split('/');
      let parent = root;
      let skip = false;
      for (const part of parts) {
        parent = join(parent, part);
        const info = await lstat(parent);
        if (info.isSymbolicLink()) { skip = true; break; }
      }
      if (!skip && (await stat(parent)).isFile()) matches.push(path);
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== 'ENOENT') throw error;
    }
  }
  matches.sort((a, b) => a < b ? -1 : a > b ? 1 : 0);
  return { paths: matches.slice(offset, offset + PAGE_SIZE), total: matches.length };
}

/** Return a bounded UTF-8 snapshot, never a path for a later unverified read. */
export async function readProjectFile(projectRoot: string, relativePath: string): Promise<{ relativePath: string; bytes: Buffer; sha256: string }> {
  const root = await rootDirectory(projectRoot);
  const canonical = await resolveProjectFile(root, relativePath);
  const source = resolve(root, relativePath);
  const handle = await open(canonical, constants.O_RDONLY | (constants.O_NOFOLLOW ?? 0));
  try {
    const before = await handle.stat();
    const sourceNow = await realpath(source);
    const current = await stat(sourceNow);
    if (!inside(root, sourceNow) || sourceNow !== canonical || !before.isFile() || before.dev !== current.dev || before.ino !== current.ino)
      throw new Error('Project file changed while opening');
    if (before.size > WORKSPACE_UPLOAD_MAX_BYTES) throw new RangeError('Project file exceeds upload limit');
    const bytes = Buffer.alloc(before.size);
    let offset = 0;
    while (offset < bytes.length) {
      const { bytesRead } = await handle.read(bytes, offset, bytes.length - offset, offset);
      if (!bytesRead) throw new Error('Project file changed while reading');
      offset += bytesRead;
    }
    const after = await handle.stat();
    const finalPath = await realpath(source);
    const finalStat = await stat(finalPath);
    if (!inside(root, finalPath) || finalPath !== canonical || before.dev !== after.dev || before.ino !== after.ino || before.size !== after.size || before.mtimeMs !== after.mtimeMs || after.dev !== finalStat.dev || after.ino !== finalStat.ino || after.size !== finalStat.size || after.mtimeMs !== finalStat.mtimeMs)
      throw new Error('Project file changed while reading');
    if (bytes.includes(0)) throw new TypeError('Project file is not text');
    try { new TextDecoder('utf-8', { fatal: true }).decode(bytes); } catch { throw new TypeError('Project file is not UTF-8 text'); }
    return { relativePath: relativePath.split(sep).join('/'), bytes, sha256: createHash('sha256').update(bytes).digest('hex') };
  } finally { await handle.close(); }
}

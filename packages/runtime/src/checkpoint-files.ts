import { constants } from 'node:fs';
import { lstat, open, readlink, realpath } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { isAbsolute, join, relative, sep } from 'node:path';

export class CodeCheckpointError extends Error {
  constructor(readonly code: string, message: string, options?: ErrorOptions) {
    super(`[CHECKPOINT_${code}] ${message}`, options); this.name = 'CodeCheckpointError';
  }
}
export const checkpointHash = (value: Buffer | string): string => createHash('sha256').update(value).digest('hex');
export function checkpointPath(value: string): void {
  if (!value || value.includes('\0') || isAbsolute(value) || /^[a-z]:/i.test(value)
    || value.split(process.platform === 'win32' ? /[\\/]/ : /\//).some(part => !part || part === '.' || part === '..' || part.toLowerCase() === '.git'))
    throw new CodeCheckpointError('UNSAFE_PATH', 'Checkpoint paths must be relative files outside Git metadata');
}
export function checkpointInside(root: string, path: string): boolean {
  const value = relative(root, path); return !value || value !== '..' && !value.startsWith(`..${sep}`) && !isAbsolute(value);
}
export interface CheckpointEntry {
  path: string; kind: 'file' | 'symlink'; mode: '100644' | '100755' | '120000';
  permissions?: number; bytes: number; sha256: string; gitOid?: string;
}
export async function readCheckpointRegular(path: string, maximum: number): Promise<Buffer> {
  const beforePath = await lstat(path);
  if (!beforePath.isFile() || beforePath.isSymbolicLink()) throw new CodeCheckpointError('UNSAFE_FILE', 'Checkpoint content must be a regular file');
  const handle = await open(path, constants.O_RDONLY | (constants.O_NOFOLLOW ?? 0));
  try {
    const before = await handle.stat();
    if (!before.isFile() || before.dev !== beforePath.dev || before.ino !== beforePath.ino) throw new CodeCheckpointError('CONCURRENT_CHANGE', 'File identity changed during checkpoint read');
    if (before.size > maximum) throw new CodeCheckpointError('FILE_LIMIT', 'A checkpoint file exceeds its declared byte limit');
    const data = Buffer.alloc(before.size + 1); let length = 0;
    while (length < data.length) { const value = await handle.read(data, length, data.length - length, null); if (!value.bytesRead) break; length += value.bytesRead; }
    const after = await handle.stat(), target = await lstat(path);
    if (length !== before.size || before.size !== after.size || before.dev !== target.dev || before.ino !== target.ino
      || before.mtimeMs !== after.mtimeMs || before.ctimeMs !== after.ctimeMs || target.mtimeMs !== after.mtimeMs || target.ctimeMs !== after.ctimeMs)
      throw new CodeCheckpointError('CONCURRENT_CHANGE', 'File changed during checkpoint read');
    return data.subarray(0, length);
  } finally { await handle.close(); }
}
export async function checkpointDiskEntry(root: string, path: string, maximum: number): Promise<{entry: CheckpointEntry; data: Buffer; identity: string} | undefined> {
  checkpointPath(path); let current = root;
  const parts = path.split('/'); let observed = false;
  try {
    for (const part of parts.slice(0, -1)) {
      current = join(current, part); const value = await lstat(current);
      if (!value.isDirectory() || value.isSymbolicLink()) throw new CodeCheckpointError('UNSAFE_PATH', 'A checkpoint file parent is not a real directory');
    }
    current = join(root, path); const before = await lstat(current); observed = true;
    let data: Buffer, kind: CheckpointEntry['kind'], mode: CheckpointEntry['mode'];
    if (before.isSymbolicLink()) { data = await readlink(current, {encoding: 'buffer'}); kind = 'symlink'; mode = '120000'; }
    else if (before.isFile()) {
      if (before.mode & 0o7000) throw new CodeCheckpointError('UNSUPPORTED_MODE', 'Special permission bits are not supported by code checkpoints');
      if (!checkpointInside(root, await realpath(current))) throw new CodeCheckpointError('UNSAFE_PATH', 'Checkpoint content escaped its execution directory');
      data = await readCheckpointRegular(current, maximum); kind = 'file'; mode = before.mode & 0o111 ? '100755' : '100644';
    } else throw new CodeCheckpointError('UNSUPPORTED_FILE', 'Directories, nested repositories and special files cannot replace checkpoint files');
    if (data.length > maximum) throw new CodeCheckpointError('FILE_LIMIT', 'A checkpoint file exceeds its declared byte limit');
    const after = await lstat(current);
    if (before.dev !== after.dev || before.ino !== after.ino || before.mode !== after.mode || before.size !== after.size || before.mtimeMs !== after.mtimeMs || before.ctimeMs !== after.ctimeMs
      || kind === 'symlink' && !(await readlink(current, {encoding: 'buffer'})).equals(data))
      throw new CodeCheckpointError('CONCURRENT_CHANGE', 'File changed during checkpoint capture');
    const entry: CheckpointEntry = {path, kind, mode, ...(kind === 'file' && {permissions: before.mode & 0o777}), bytes: data.length, sha256: checkpointHash(data)};
    return {entry, data, identity: JSON.stringify([before.dev, before.ino, before.mode, before.size, before.mtimeMs, before.ctimeMs])};
  } catch (error) { if ((error as NodeJS.ErrnoException).code === 'ENOENT') {if(observed)throw new CodeCheckpointError('CONCURRENT_CHANGE','A file disappeared during checkpoint capture');return;} throw error; }
}

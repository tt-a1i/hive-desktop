import { constants } from 'node:fs';
import { lstat, open, opendir, readdir, readlink, realpath } from 'node:fs/promises';
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

export interface CheckpointInventoryEntry {path: string; kind: 'file' | 'symlink' | 'directory' | 'special'; ignored: boolean}
/** Git omits some filesystem nodes (notably FIFOs/sockets) from ls-files --others.
 * Enumerate without following links; prune ignored directories before visiting children. */
export async function checkpointFilesystemInventory(root: string, tracked: ReadonlySet<string>,
  classifyIgnored: (paths: string[]) => Promise<boolean[]>, maximum: number): Promise<CheckpointInventoryEntry[]> {
  const inventory: CheckpointInventoryEntry[] = [], directories = [''], trackedDirectories = new Set<string>(); let visited = 0;
  for (const path of tracked) {const parts=path.split('/');for(let end=1;end<parts.length;end++)trackedDirectories.add(parts.slice(0,end).join('/'));}
  try {
    for (let at = 0; at < directories.length; at++) {
      const relativeDirectory = directories[at]!, path = relativeDirectory ? join(root, relativeDirectory) : root;
      const before = await lstat(path);
      if (!before.isDirectory() || before.isSymbolicLink() || await realpath(path) !== path)
        throw new CodeCheckpointError('CONCURRENT_CHANGE', 'A filesystem inventory directory was replaced or linked');
      const candidates: {path: string; kind: CheckpointInventoryEntry['kind']}[] = [];
      let names: string[] = [];
      const directory = await opendir(path);
      for await (const entry of directory) {
        if (entry.name.toLowerCase() === '.git') continue;
        if (++visited > maximum) throw new CodeCheckpointError('ENTRY_LIMIT', 'Filesystem inventory exceeds the checkpoint entry limit');
        names.push(entry.name);
      }
      // opendir has only a string encoding contract on supported Node versions.
      // Verify the rare ambiguous decoding case with raw names after bounded enumeration.
      if (names.some(name=>name.includes('\uFFFD'))) {
        const raw = await readdir(path, {encoding:'buffer'});
        if (raw.length > maximum + 1) throw new CodeCheckpointError('ENTRY_LIMIT', 'Filesystem inventory exceeds the checkpoint entry limit');
        let decoded: string[];
        try {decoded = raw.map(name=>new TextDecoder('utf-8',{fatal:true,ignoreBOM:true}).decode(name)).filter(name=>name.toLowerCase()!=='.git');}
        catch {throw new CodeCheckpointError('UNSUPPORTED_PATH_ENCODING','Checkpoint filesystem paths must be valid UTF-8');}
        if (JSON.stringify([...names].sort()) !== JSON.stringify([...decoded].sort()))
          throw new CodeCheckpointError('CONCURRENT_CHANGE','Filesystem names changed during checkpoint inventory');
        names = decoded;
      }
      for (const name of names) {
        const local = relativeDirectory ? relativeDirectory + '/' + name : name; checkpointPath(local);
        const info = await lstat(join(root, local));
        const kind = info.isSymbolicLink() ? 'symlink' : info.isFile() ? 'file' : info.isDirectory() ? 'directory' : 'special';
        if (kind === 'special' && ['.gitignore','.gitattributes'].includes(name.toLowerCase()))
          throw new CodeCheckpointError('UNSUPPORTED_SPECIAL_FILE','Special Git rule files cannot be safely read for checkpoint classification');
        candidates.push({path: local, kind});
      }
      // check-ignore may read a candidate directory's rules even when the
      // directory itself is ignored. Inspect those names without opening them.
      for (const candidate of candidates) if (candidate.kind === 'directory') {
        for (const name of ['.gitignore', '.gitattributes']) {
          try {
            const info = await lstat(join(root, candidate.path, name));
            if (!info.isFile() && !info.isSymbolicLink() && !info.isDirectory())
              throw new CodeCheckpointError('UNSUPPORTED_SPECIAL_FILE','Special Git rule files cannot be safely read for checkpoint classification');
          } catch (error) {if ((error as NodeJS.ErrnoException).code !== 'ENOENT') throw error;}
        }
      }
      const after = await lstat(path);
      if (before.dev !== after.dev || before.ino !== after.ino || before.mtimeMs !== after.mtimeMs || before.ctimeMs !== after.ctimeMs || await realpath(path) !== path)
        throw new CodeCheckpointError('CONCURRENT_CHANGE', 'Filesystem inventory changed while enumerating a directory');
      candidates.sort((a,b)=>a.path<b.path?-1:a.path>b.path?1:0);
      const ignored = await classifyIgnored(candidates.map(entry=>entry.path+(entry.kind==='directory'?'/':'')));
      if (ignored.length !== candidates.length) throw new CodeCheckpointError('INVALID_GIT_OUTPUT', 'Git ignore classification is incomplete');
      for (let index = 0; index < candidates.length; index++) {
        const entry = candidates[index]!, excluded = ignored[index]! && !tracked.has(entry.path) && !(entry.kind==='directory'&&trackedDirectories.has(entry.path));
        inventory.push({...entry, ignored: excluded});
        if (excluded) continue;
        if (entry.kind === 'special') throw new CodeCheckpointError('UNSUPPORTED_SPECIAL_FILE', 'Non-ignored FIFOs, sockets and device nodes cannot be captured');
        if (entry.kind === 'directory') directories.push(entry.path);
      }
    }
  } catch (error) {
    if (['ENOENT','ENOTDIR'].includes((error as NodeJS.ErrnoException).code ?? ''))
      throw new CodeCheckpointError('CONCURRENT_CHANGE', 'Filesystem inventory changed during checkpoint capture', {cause:error});
    throw error;
  }
  return inventory.sort((a,b)=>a.path<b.path?-1:a.path>b.path?1:0);
}

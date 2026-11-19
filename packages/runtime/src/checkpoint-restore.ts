import { execFile } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { link, lstat, mkdir, open, realpath, rename, rmdir, symlink, unlink } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { promisify } from 'node:util';
import { type CodeCheckpoints, type CodeCheckpointManifest, type CodeCheckpointReader, type CodeCheckpointPreview } from './code-checkpoints.js';
import { checkpointDiskEntry, checkpointHash, checkpointPath, CodeCheckpointError, readCheckpointRegular, type CheckpointEntry } from './checkpoint-files.js';
import { getGitWorkspaceVersion } from './git-workspace.js';
import { readWorktreeIntent, validateManagedWorktree, type ManagedWorktree } from './git-worktrees.js';

type RestoreInput = {
  dataRoot: string;
  workspaceId: string;
  sessionId: string;
  operationId: string;
  checkpoints: CodeCheckpoints;
  future: CodeCheckpointReader;
  target: CodeCheckpointReader;
};
type FileChange = { path: string; before?: CheckpointEntry; after?: CheckpointEntry };
type DiskNode = {kind: 'missing' | 'directory'} | {kind: 'blocked'; parent: string} | {kind: 'entry'; entry: CheckpointEntry};
const git = promisify(execFile);
const uuid = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;

function conflict(message: string): never {
  throw new CodeCheckpointError('RESTORE_CONFLICT', message);
}
const missing = (error: unknown) => (error as NodeJS.ErrnoException).code === 'ENOENT';
const same = (a?: CheckpointEntry, b?: CheckpointEntry) => !a || !b ? a === b
  : a.path === b.path && a.kind === b.kind && a.mode === b.mode && a.permissions === b.permissions
    && a.bytes === b.bytes && a.sha256 === b.sha256;

async function inspect(root: string, path: string, maximum: number): Promise<DiskNode> {
  checkpointPath(path);
  if (await realpath(root) !== root) conflict('Restore directory was replaced');
  const parts = path.split('/');
  let directory = root;
  for (const [i, part] of parts.slice(0, -1).entries()) {
    directory = join(directory, part);
    try {
      const info = await lstat(directory);
      if (!info.isDirectory() || info.isSymbolicLink()) return {kind: 'blocked', parent: parts.slice(0, i + 1).join('/')};
      if (await realpath(directory) !== directory) conflict(`Worktree directory changed: ${path}`);
    } catch (error) { if (missing(error)) return {kind: 'missing'}; throw error; }
  }
  try {
    const file = await lstat(join(root, path));
    if (file.isDirectory() && !file.isSymbolicLink()) return {kind: 'directory'};
    const value = await checkpointDiskEntry(root, path, maximum);
    if (!value) conflict(`Worktree path changed during restore: ${path}`);
    return {kind: 'entry', entry: value.entry};
  } catch (error) { if (missing(error)) return {kind: 'missing'}; throw error; }
}

async function ensureParents(root: string, path: string) {
  let current = root;
  for (const part of path.split('/').slice(0, -1)) {
    current = join(current, part);
    try { await mkdir(current, {mode: 0o755}); await syncDirectory(dirname(current)); }
    catch (error) { if ((error as NodeJS.ErrnoException).code !== 'EEXIST') throw error; }
    if (await realpath(current) !== current || !(await lstat(current)).isDirectory()) conflict(`Worktree parent changed: ${path}`);
  }
}

function fileChanges(future: CodeCheckpointManifest, target: CodeCheckpointManifest) {
  const paths = new Map<string, FileChange>();
  const aliases = new Map<string, string>();
  for (const [manifest, key] of [[future, 'before'], [target, 'after']] as const) {
    for (const entry of manifest.disk) {
      checkpointPath(entry.path);
      let path = '';
      for (const segment of entry.path.split('/')) {
        path += path ? `/${segment}` : segment;
        if (process.platform === 'win32' || process.platform === 'darwin') {
          const alias = path.normalize('NFC').toLowerCase();
          const original = aliases.get(alias);
          if (original && original !== path) conflict(`Case or Unicode spelling collision: ${path}`);
          aliases.set(alias, path);
        }
      }
      const change = paths.get(entry.path) ?? {path: entry.path};
      change[key] = entry;
      paths.set(entry.path, change);
    }
  }
  return [...paths.values()].filter(change => !same(change.before, change.after));
}

/** A restore may only target the registered worktree and its original Git index. */
function validateSnapshot(input: RestoreInput, owned: ManagedWorktree, manifest: CodeCheckpointManifest) {
  const workspace = manifest.preview.workspace;
  if (manifest.workspaceId !== input.workspaceId || manifest.sessionId !== input.sessionId
    || owned.projectPrefix || owned.executionPath !== owned.managedWorktreeRoot
    || workspace.projectRoot !== owned.executionPath || workspace.repositoryRoot !== owned.managedWorktreeRoot
    || workspace.projectPrefix || workspace.commonDir !== owned.repoCommonDir || workspace.gitDir !== owned.gitDir
    || !workspace.linkedWorktree || workspace.branch !== owned.branch || !workspace.headOid || !owned.gitDir
    || workspace.indexPath !== join(owned.gitDir, 'index')) conflict('A restore requires two checkpoints from this exact owned worktree');
}

function assertExclusionsSafe(changes: FileChange[], preview: CodeCheckpointPreview) {
  const excluded = new Set(preview.exclusions.map(path => path.replace(/\/$/, '')));
  const desired = new Set(changes.filter(change => change.after).map(change => change.path));
  const overlapsAncestor = (path: string, paths: Set<string>) => {
    for (;;) {
      if (paths.has(path)) return true;
      const split = path.lastIndexOf('/');
      if (split < 0) return false;
      path = path.slice(0, split);
    }
  };
  for (const path of desired) if (overlapsAncestor(path, excluded)) {
    conflict(`A checkpoint path would replace excluded content: ${path}`);
  }
  for (const path of excluded) if (overlapsAncestor(path, desired)) {
    conflict(`A checkpoint path would replace excluded content: ${path}`);
  }
}

async function syncDirectory(path: string) {
  if (process.platform === 'win32') return;
  const handle = await open(path, 'r');
  try { await handle.sync(); } finally { await handle.close(); }
}

async function restoreClaim(directory: string, stamp: string, mayStart: () => Promise<void>) {
  const claim = join(directory, 'claim.json');
  const read = async () => (await readCheckpointRegular(claim, 4096)).toString('utf8');
  try {
    if (await realpath(directory) !== directory || await read() !== stamp) conflict('Restore claim belongs to a different operation');
    return;
  } catch (error) { if (!missing(error)) throw error; }
  await mayStart();
  try { await mkdir(directory, {mode: 0o700}); }
  catch (error) { if ((error as NodeJS.ErrnoException).code !== 'EEXIST') throw error; }
  if (await realpath(directory) !== directory) conflict('Restore storage was replaced');
  const temporary = join(directory, randomUUID());
  try {
    const fd = await open(temporary, 'wx', 0o600);
    try { await fd.writeFile(stamp); await fd.sync(); } finally { await fd.close(); }
    try { await link(temporary, claim); }
    catch (error) { if ((error as NodeJS.ErrnoException).code !== 'EEXIST' || await read() !== stamp) throw error; }
    await syncDirectory(directory);
    await syncDirectory(dirname(directory));
  } finally { await unlink(temporary).catch(error => {if (!missing(error)) throw error;}); }
}

async function gitRef(root: string, args: string[], input?: string) {
  const env = Object.fromEntries(Object.entries(process.env).filter(([key]) => !/^GIT_/i.test(key)));
  const nil = process.platform === 'win32' ? 'NUL' : '/dev/null';
  try {
    const command = git('git', ['--no-pager', '-c', `core.hooksPath=${nil}`, '-c', 'core.fsmonitor=false', ...args], {
      cwd: root, env: {...env, GIT_CONFIG_NOSYSTEM: '1', GIT_CONFIG_GLOBAL: nil, GIT_TERMINAL_PROMPT: '0', GIT_NO_LAZY_FETCH: '1', GIT_NO_REPLACE_OBJECTS: '1'},
      windowsHide: true, timeout: 8000, maxBuffer: input ? 2 * 1024 * 1024 : 8192,
    });
    if (input) {
      command.child.stdin!.on('error', () => { command.child.kill(); });
      command.child.stdin!.end(input);
    }
    return (await command).stdout.trim();
  } catch (error) { throw new CodeCheckpointError('RESTORE_GIT_FAILED', 'Git could not verify objects or update the owned branch', {cause: error}); }
}

async function statIfPresent(path: string) {
  try { return await lstat(path); }
  catch (error) { if (!missing(error)) throw error; }
}

async function ownsIndexLock(stage: string, lock: string) {
  const a = await statIfPresent(stage), b = await statIfPresent(lock);
  if (!b) return false;
  if (!a?.isFile() || !b.isFile() || a.ino !== b.ino || a.dev !== b.dev) conflict('Git index is locked by another operation');
  return true;
}

async function installIndex(path: string, current: string, desired: CodeCheckpointReader, operationId: string) {
  const revision = desired.manifest.rawIndex;
  const targetHash = revision?.sha256 ?? 'missing';
  const lock = `${path}.lock`;
  const stage = join(dirname(path), `hive-restore-index-${operationId}`);
  let ownedLock = await ownsIndexLock(stage, lock);
  if (targetHash !== current && !await statIfPresent(stage)) {
    const temporary = join(dirname(path), randomUUID());
    const file = await open(temporary, 'wx', 0o600);
    try {
      try { await file.writeFile(revision ? await desired.read(revision) : Buffer.alloc(0)); await file.sync(); }
      finally { await file.close(); }
      await link(temporary, stage);
      await syncDirectory(dirname(path));
    } finally { await unlink(temporary); }
  }
  try {
    if (targetHash !== current && !ownedLock) {
      await link(stage, lock);
      ownedLock = true;
    }
    if (!ownedLock) return;
    let hash = 'missing';
    try { hash = checkpointHash(await readCheckpointRegular(path, 64 * 1024 * 1024)); }
    catch (error) { if (!missing(error)) throw error; }
    if (hash !== current) conflict('Git index changed while preparing the restore');
    if (targetHash !== current) {
      const bytes = await readCheckpointRegular(lock, revision?.bytes ?? 0);
      if (revision ? bytes.length !== revision.bytes || checkpointHash(bytes) !== revision.sha256 : bytes.length !== 0) conflict('The restore index staging file changed');
      if (revision) { await rename(lock, path); ownedLock = false; }
      else await unlink(path);
    }
    await syncDirectory(dirname(path));
  } finally {
    if (ownedLock && await ownsIndexLock(stage, lock)) await unlink(lock);
    if (await statIfPresent(stage)) await unlink(stage);
  }
}

export async function inspectCheckpointRestore(input: RestoreInput) {
  if (!uuid.test(input.operationId)) conflict('Restore operation identity is invalid');
  const owned = (await readWorktreeIntent(input.dataRoot, input.sessionId)).record;
  const root = await validateManagedWorktree(input.dataRoot, owned);
  const future = input.future.manifest, target = input.target.manifest;
  validateSnapshot(input, owned, future);
  validateSnapshot(input, owned, target);
  const changes = fileChanges(future, target);
  assertExclusionsSafe(changes, future.preview);
  const before = future.preview.workspace, after = target.preview.workspace;
  const checkGit = async () => {
    await validateManagedWorktree(input.dataRoot, owned);
    const state = await getGitWorkspaceVersion(root);
    if (state.gitDir !== before.gitDir || state.commonDir !== before.commonDir || state.indexPath !== before.indexPath
      || state.branch !== owned.branch || state.configFingerprint !== before.configFingerprint
      || (state.headOid !== before.headOid && state.headOid !== after.headOid)
      || (state.indexFingerprint !== before.indexFingerprint && state.indexFingerprint !== after.indexFingerprint)) {
      conflict('HEAD, index or repository configuration changed outside the restore');
    }
    for (const name of ['MERGE_HEAD', 'CHERRY_PICK_HEAD', 'REBASE_HEAD', 'rebase-merge', 'rebase-apply', 'sequencer']) {
      if (await statIfPresent(join(state.gitDir, name))) conflict('A Git merge, cherry-pick or rebase is in progress');
    }
    await ownsIndexLock(join(state.gitDir, `hive-restore-index-${input.operationId}`), `${state.indexPath}.lock`);
    return state;
  };
  await checkGit();
  if (await gitRef(root, ['cat-file', '-t', after.headOid!]) !== 'commit') conflict('Target HEAD is no longer available');
  const objects = [...new Set(target.index.map(entry => entry.gitOid))];
  if (objects.some(oid => !oid)) conflict('Target index references an invalid Git object');
  if (objects.length) {
    const types = await gitRef(root, ['cat-file', '--batch-check=%(objectname) %(objecttype)'], objects.join('\n') + '\n');
    if (types !== objects.map(oid => `${oid} blob`).join('\n')) conflict('Target index references an unavailable Git blob');
  }
  return {owned, root, changes, checkGit};
}

/** Apply or explicitly resume the code portion of a confirmed rewind. The
 * caller owns the execution lease and keeps the conversation gated throughout. */
export async function restoreCheckpoint(input: RestoreInput): Promise<void> {
  const {owned, root, changes, checkGit} = await inspectCheckpointRestore(input);
  const future = input.future.manifest, target = input.target.manifest;
  const after = target.preview.workspace;
  const byPath = new Map(changes.map(change => [change.path, change]));
  const retiredDirectories = new Set<string>();
  for (const change of changes) if (change.before) {
    let parent = change.path;
    while (parent.includes('/')) { parent = parent.slice(0, parent.lastIndexOf('/')); retiredDirectories.add(parent); }
  }
  const archive = join(dirname(root), `restore-${input.operationId}`);
  await restoreClaim(archive, JSON.stringify({version: 1, owner: owned.ownerToken, future: future.checkpointId, target: target.checkpointId}), async () => {
    const current = await input.checkpoints.preview();
    if (!current.canCapture || current.version !== future.preview.version) conflict('Worktree changed since the verified backup');
  });

  const max = Math.max(future.preview.limits.fileBytes, target.preview.limits.fileBytes);
  const stashName = (path: string) => `before-${checkpointHash(path)}`;
  for (const change of changes) {
    const current = await inspect(root, change.path, max);
    const held = await inspect(archive, stashName(change.path), max);
    const archivedBefore = held.kind === 'entry' && change.before && same({...held.entry, path: change.path}, change.before);
    if (held.kind !== 'missing' && !archivedBefore) conflict(`Retired file differs from its backup: ${change.path}`);
    if (current.kind === 'entry' && !same(current.entry, archivedBefore ? change.after : change.before ?? change.after)) {
      conflict(`File changed outside the restore: ${change.path}`);
    }
    if (!archivedBefore && change.before && current.kind !== 'entry') conflict(`A backed-up file is no longer at its original path: ${change.path}`);
    if (current.kind === 'directory' && change.after && !retiredDirectories.has(change.path)) conflict(`An unrelated directory blocks a restored file: ${change.path}`);
    if (current.kind === 'blocked') {
      const parent = byPath.get(current.parent);
      if (!parent?.before && !parent?.after) conflict(`Unrelated file blocks a restore path: ${change.path}`);
    }
  }
  // Remove changed paths before publishing any target file. The retired bytes
  // stay in the operation directory, even if a later check fails.
  for (const change of changes) {
    if (!change.before) continue;
    const name = stashName(change.path);
    if ((await inspect(archive, name, max)).kind !== 'missing') continue;
    const observed = await inspect(root, change.path, max);
    if (observed.kind !== 'entry' || !same(observed.entry, change.before)) conflict(`File changed before retirement: ${change.path}`);
    await rename(join(root, change.path), join(archive, name));
    const retired = await inspect(archive, name, max);
    if (retired.kind !== 'entry' || !same({...retired.entry, path: change.path}, change.before)) conflict(`File changed during retirement: ${change.path}`);
    await syncDirectory(dirname(join(root, change.path)));
    await syncDirectory(archive);
  }
  for (const {path, after: desired} of changes) {
    if (!desired) continue;
    let observed = await inspect(root, path, max);
    if (observed.kind === 'entry' && same(observed.entry, desired)) continue;
    if (observed.kind === 'directory') {
      // Only remove known directory parents of retired files, and only if
      // empty. External or ignored contents are left where they were.
      const descendants = [...retiredDirectories].filter(dir => dir === path || dir.startsWith(`${path}/`));
      for (const dir of descendants.sort((a, b) => b.length - a.length)) {
        const candidate = await inspect(root, dir, max);
        if (candidate.kind === 'directory') await rmdir(join(root, dir));
      }
      observed = await inspect(root, path, max);
    }
    if (observed.kind !== 'missing') conflict(`Cannot publish restored file: ${path}`);
    await ensureParents(root, path);
    const file = join(root, path), bytes = await input.target.read(desired);
    if (desired.kind === 'symlink') await symlink(bytes, file);
    else {
      const staged = join(archive, randomUUID());
      const permissions = desired.permissions ?? (desired.mode === '100755' ? 0o755 : 0o644);
      const fd = await open(staged, 'wx', permissions);
      try {
        try { await fd.writeFile(bytes); await fd.chmod(permissions); await fd.sync(); } finally { await fd.close(); }
        await link(staged, file); // No-replace, including when an editor raced the preflight.
      } finally { await unlink(staged); }
    }
    await syncDirectory(dirname(file));
  }

  const state = await checkGit();
  await installIndex(state.indexPath, state.indexFingerprint, input.target, input.operationId);
  if (state.headOid !== after.headOid) await gitRef(root, ['update-ref', '-m', 'Hive checkpoint restore', `refs/heads/${owned.branch}`, after.headOid!, state.headOid!]);
  const final = await checkGit();
  if (final.headOid !== after.headOid || final.indexFingerprint !== after.indexFingerprint || !await input.checkpoints.matchesLayout(target)) {
    conflict('The restored HEAD, index or files differ from the confirmed checkpoint');
  }
}

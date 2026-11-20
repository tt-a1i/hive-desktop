import { randomUUID } from 'node:crypto';
import { link, lstat, mkdir, open, readFile, readdir, realpath, rm } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import type { WorkspaceSummary } from './hive-runtime-contract.js';

type HiveRequest = (method: string, path: string, body?: unknown) => Promise<unknown>;
export const isScratchId = (value: unknown): value is string => typeof value === 'string'
  && /^[a-f0-9]{8}-[a-f0-9]{4}-4[a-f0-9]{3}-[89ab][a-f0-9]{3}-[a-f0-9]{12}$/.test(value);

export function scratchConversationPath(workspacePath: string, workspaceId: string, sessionId: string): string {
  if (!isScratchId(workspaceId) || !isScratchId(sessionId)) throw new Error('Invalid Scratch conversation identity');
  return join(workspacePath, 'sessions', workspaceId, sessionId);
}

export async function isScratchWorkspace(dataDir: string, workspacePath: string): Promise<boolean> {
  return workspacePath === join(await realpath(dataDir), 'scratch');
}

const missing = (error: unknown) => (error as NodeJS.ErrnoException).code === 'ENOENT';

async function ensureDirectory(path: string): Promise<void> {
  try { await mkdir(path, { mode: 0o700 }); }
  catch (error) {
    if ((error as NodeJS.ErrnoException).code !== 'EEXIST') throw error;
  }
}

async function directoryIdentity(path: string): Promise<string> {
  const metadata = await lstat(path, {bigint: true});
  if (!metadata.isDirectory() || await realpath(path) !== path) throw new Error('Scratch folder is not an owned directory');
  return `${metadata.dev}:${metadata.ino}:${metadata.birthtimeNs}`;
}

async function readReceipt(file: string): Promise<string | undefined> {
  try {
    if (!(await lstat(file)).isFile() || await realpath(file) !== file) throw new Error('Invalid Scratch folder ownership receipt');
    return await readFile(file, 'utf8');
  } catch (error) {
    if (missing(error)) return;
    throw error;
  }
}

async function scratchLocation(dataDir: string, workspace: WorkspaceSummary, sessionId: string) {
  const root = await realpath(dataDir);
  if (workspace.path !== join(root, 'scratch')) throw new Error('Scratch workspace folder is outside the desktop data folder');
  await directoryIdentity(workspace.path);
  const path = scratchConversationPath(workspace.path, workspace.id, sessionId);
  const workspaceReceipts = join(root, 'scratch-ownership', workspace.id);
  return {path, workspaceReceipts, file: join(workspaceReceipts, `${sessionId}.json`)};
}

const receiptFor = (workspaceId: string, sessionId: string, identity: string) =>
  JSON.stringify({version: 1, workspaceId, sessionId, identity});

export async function validateScratchConversation(dataDir: string, workspace: WorkspaceSummary, sessionId: string): Promise<string> {
  const {path, file} = await scratchLocation(dataDir, workspace, sessionId);
  const receipt = await readReceipt(file);
  if (receipt === undefined || receipt !== receiptFor(workspace.id, sessionId, await directoryIdentity(path))) {
    throw new Error('Scratch conversation folder is missing or was replaced');
  }
  return path;
}

/** Call under the workspace creation lock, before publishing a conversation record. */
export async function createScratchConversation(dataDir: string, workspace: WorkspaceSummary, sessionId: string): Promise<string> {
  const {path, workspaceReceipts, file} = await scratchLocation(dataDir, workspace, sessionId);
  for (const directory of [dirname(dirname(path)), dirname(path), dirname(workspaceReceipts), workspaceReceipts]) {
    await ensureDirectory(directory);
    await directoryIdentity(directory);
  }
  if (await readReceipt(file) !== undefined) return validateScratchConversation(dataDir, workspace, sessionId);

  await ensureDirectory(path);
  const identity = await directoryIdentity(path);
  // A quit between mkdir and receipt publication can only leave an empty folder.
  // Refuse any unclaimed folder that already contains files.
  if ((await readdir(path)).length !== 0) throw new Error('Unclaimed Scratch conversation folder already contains files');
  const temp = join(workspaceReceipts, `${randomUUID()}.tmp`);
  try {
    const handle = await open(temp, 'wx', 0o600);
    try {
      await handle.writeFile(receiptFor(workspace.id, sessionId, identity));
      await handle.sync();
    } finally { await handle.close(); }
    try { await link(temp, file); }
    catch (error) {
      if ((error as NodeJS.ErrnoException).code !== 'EEXIST') throw error;
    }
  } finally { await rm(temp, {force: true}); }
  return validateScratchConversation(dataDir, workspace, sessionId);
}

async function ensureOutsideGit(directory: string): Promise<void> {
  for (let folder = directory; ; folder = dirname(folder)) {
    try {
      await lstat(join(folder, '.git'));
      throw new Error('The Scratch workspace must be outside a Git working tree.');
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== 'ENOENT') throw error;
    }
    if (dirname(folder) === folder) return;
  }
}

export async function scratchWorkspace(dataDir: string, requestHive: HiveRequest): Promise<WorkspaceSummary> {
  const root = await realpath(dataDir);
  await ensureOutsideGit(root);
  const path = join(root, 'scratch');
  await ensureDirectory(path);
  if (!(await lstat(path)).isDirectory() || await realpath(path) !== path) {
    throw new Error('The Scratch workspace folder must be a directory inside the desktop data folder.');
  }
  await ensureOutsideGit(path);

  const find = async () => (await requestHive('GET', '/api/workspaces') as WorkspaceSummary[])
    .find(workspace => workspace.path === path);
  const existing = await find();
  if (existing) return existing;
  try {
    const workspace = await requestHive('POST', '/api/workspaces', {
      path, name: 'Scratch', autostart_orchestrator: false,
    }) as WorkspaceSummary;
    if (workspace.path !== path) throw new Error('Hive returned a different Scratch workspace folder.');
    return workspace;
  } catch (error) {
    // Hive can commit the workspace before a lost response, or another request may win.
    const recovered = await find();
    if (!recovered) throw error;
    return recovered;
  }
}

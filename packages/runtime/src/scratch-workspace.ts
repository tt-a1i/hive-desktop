import { lstat, mkdir, realpath } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import type { WorkspaceSummary } from './hive-runtime-contract.js';

type HiveRequest = (method: string, path: string, body?: unknown) => Promise<unknown>;

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
  try { await mkdir(path, { mode: 0o700 }); }
  catch (error) {
    if ((error as NodeJS.ErrnoException).code !== 'EEXIST') throw error;
  }
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

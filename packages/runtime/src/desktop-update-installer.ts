import { spawn, type ChildProcess } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { once } from 'node:events';
import { createReadStream } from 'node:fs';
import { lstat, mkdir, readFile, realpath, rename, rm, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { pathToFileURL } from 'node:url';

export type DesktopInstallPlan = {
  installation: string;
  dataDirectory: string;
  fromVersion: string;
  toVersion: string;
  oldExecutableSha256: string;
  newExecutableSha256: string;
};

export function desktopPackagePaths(root: string) {
  const contents = process.platform === 'darwin' ? join(root, 'Contents') : root;
  const resources = join(contents, process.platform === 'darwin' ? 'Resources' : 'resources');
  return {
    resources,
    executable: join(contents, process.platform === 'darwin' ? 'MacOS/hive-desktop' : 'hive-desktop.exe'),
    node: join(resources, process.platform === 'darwin' ? 'node/bin/node' : 'node/node.exe'),
    runtime: join(resources, 'runtime/dist/main.js'),
  };
}

export async function hashDesktopFile(path: string, signal?: AbortSignal): Promise<string> {
  const hash = createHash('sha256');
  for await (const chunk of createReadStream(path, { signal })) hash.update(chunk);
  return hash.digest('hex');
}

function isLive(pid: number): boolean {
  try { process.kill(pid, 0); return true; }
  catch (error) {
    if ((error as NodeJS.ErrnoException).code === 'ESRCH') return false;
    throw error;
  }
}

async function move(source: string, target: string) {
  const deadline = Date.now() + 4000;
  for (;;) {
    try { await rename(source, target); return; }
    catch (error) {
      if (process.platform !== 'win32' || !['EPERM', 'EACCES', 'EBUSY'].includes((error as NodeJS.ErrnoException).code ?? '') || Date.now() >= deadline) throw error;
      await delay(100);
    }
  }
}

async function atomicJson(path: string, value: unknown) {
  const temporary = `${path}.${randomUUID()}.part`;
  try {
    await writeFile(temporary, JSON.stringify(value), { flag: 'wx', mode: 0o600, flush: true });
    await move(temporary, path);
  } finally { await rm(temporary, { force: true }); }
}

async function checkExecutable(root: string, expected: string) {
  const { executable } = desktopPackagePaths(root);
  if (!(await lstat(executable)).isFile() || await realpath(executable) !== executable || await hashDesktopFile(executable) !== expected) {
    throw new Error('Desktop executable changed before installation');
  }
}

function launch(root: string, dataDirectory: string, readiness?: string): ChildProcess {
  const env: NodeJS.ProcessEnv = { ...process.env, HIVE_DESKTOP_DATA_DIR: dataDirectory };
  delete env.HIVE_DESKTOP_RUNTIME;
  delete env.HIVE_DESKTOP_NODE;
  delete env.HIVE_DESKTOP_UPDATE_READY;
  if (readiness) env.HIVE_DESKTOP_UPDATE_READY = readiness;
  return spawn(desktopPackagePaths(root).executable, [], { cwd: root, env, stdio: 'ignore', detached: true });
}

async function waitForReadiness(child: ChildProcess, file: string, version: string) {
  const deadline = Date.now() + 30_000;
  while (Date.now() < deadline) {
    if (child.exitCode !== null || child.signalCode !== null) throw new Error(`Updated desktop exited before startup completed (${child.exitCode ?? child.signalCode})`);
    try {
      const value = JSON.parse(await readFile(file, 'utf8'));
      if (value.pid !== child.pid || value.version !== version) throw new Error('Updated desktop reported an unexpected startup identity');
      if (child.exitCode !== null || child.signalCode !== null) throw new Error('Updated desktop exited during startup');
      return;
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== 'ENOENT') throw error;
    }
    await delay(100);
  }
  throw new Error('Updated desktop did not finish startup');
}

async function stopCandidate(child: ChildProcess) {
  if (child.exitCode !== null || child.signalCode !== null) return;
  const stopped = new AbortController();
  try {
    const exited = once(child, 'exit', { signal: stopped.signal });
    child.kill('SIGKILL');
    if (!await Promise.race([exited.then(() => true), delay(6000, false, { signal: stopped.signal })])) {
      throw new Error('Unable to confirm the updated desktop shut down');
    }
  } finally { stopped.abort(); }
}

export function installLockPath(installation: string): string {
  const key = createHash('sha256').update(process.platform === 'win32' ? installation.toUpperCase() : installation).digest('hex').slice(0, 24);
  return join(dirname(installation), `.hive-desktop-update-${key}.lock`);
}

/** Run only after the original desktop and its sidecar have exited. */
export async function installDesktopUpdate(plan: DesktopInstallPlan, transaction: string) {
  const { installation, dataDirectory, fromVersion, toVersion } = plan;
  const next = join(transaction, 'next');
  const previous = join(transaction, 'previous');
  const readiness = join(transaction, 'ready.json');
  const journal = join(transaction, 'install-status.json');
  const receipt = join(dataDirectory, 'desktop-updates', 'install-result.json');
  if (dirname(transaction) !== dirname(installation) || transaction === installation || await realpath(transaction) !== transaction) {
    throw new Error('Desktop update transaction must be beside the installation');
  }
  const lock = installLockPath(installation);
  await mkdir(lock, { mode: 0o700 });
  let movedOld = false;
  let movedNew = false;
  let installed = false;
  let candidate: ChildProcess | undefined;
  const record = (state: string, error?: string) => atomicJson(journal, { state, fromVersion, toVersion, ...(error && {error}) });
  try {
    await checkExecutable(installation, plan.oldExecutableSha256);
    await checkExecutable(next, plan.newExecutableSha256);
    await record('replacing');
    await move(installation, previous);
    movedOld = true;
    await move(next, installation);
    movedNew = true;
    await record('starting');
    candidate = launch(installation, dataDirectory, readiness);
    await once(candidate, 'spawn');
    await waitForReadiness(candidate, readiness, toVersion);
    await record('installed');
    await atomicJson(receipt, { state: 'installed', version: toVersion, requestedVersion: toVersion });
    candidate.unref();
    installed = true;
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    try {
      if (candidate?.pid) await stopCandidate(candidate);
      if (movedNew) await move(installation, next);
      if (movedOld) await move(previous, installation);
      await record(movedOld ? 'rolledBack' : 'failed', message);
      await atomicJson(receipt, { state: movedOld ? 'rolledBack' : 'failed', version: fromVersion, requestedVersion: toVersion, error: message });
      // Before replacement, launch only the verified original and never an unexpected occupant.
      await checkExecutable(installation, plan.oldExecutableSha256);
      const original = launch(installation, dataDirectory);
      await once(original, 'spawn');
      original.unref();
    } catch (recoveryError) {
      const detail = recoveryError instanceof Error ? recoveryError.message : String(recoveryError);
      await record('recoveryRequired', `${message}\n${detail}`);
      throw recoveryError;
    }
  } finally {
    if (installed) await rm(previous, { recursive: true, force: true }).catch(() => {});
    await rm(lock, { recursive: true, force: true });
  }
}

async function main() {
  const transaction = await realpath(dirname(process.argv[1]!));
  const parent = Number(process.argv[2]);
  if (!Number.isSafeInteger(parent) || parent <= 1) throw new Error('Invalid original desktop process identity');
  const plan = JSON.parse(await readFile(join(transaction, 'install-plan.json'), 'utf8')) as DesktopInstallPlan;
  const deadline = Date.now() + 30_000;
  while (isLive(parent)) {
    if (Date.now() >= deadline) throw new Error('Original desktop is still running');
    await delay(100);
  }
  await installDesktopUpdate(plan, transaction);
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  await main().catch(error => { console.error(error); process.exitCode = 1; });
}

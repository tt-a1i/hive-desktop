import { spawn, type ChildProcess } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { once } from 'node:events';
import { createReadStream } from 'node:fs';
import { lstat, readFile, realpath, rename, rm, writeFile } from 'node:fs/promises';
import { basename, dirname, isAbsolute, join, resolve } from 'node:path';
import { DatabaseSync } from 'node:sqlite';
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

type Journal = {
  state: 'replacing' | 'starting' | 'installed' | 'rolledBack' | 'failed' | 'recoveryRequired';
  fromVersion: string;
  toVersion: string;
  candidatePid?: number | undefined;
  error?: string | undefined;
};

function missing(error: unknown) {
  return (error as NodeJS.ErrnoException).code === 'ENOENT';
}

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

async function exists(path: string) {
  return lstat(path).then(() => true, error => {
    if (missing(error)) return false;
    throw error;
  });
}

function launch(root: string, dataDirectory: string, readiness?: string): ChildProcess {
  const env: NodeJS.ProcessEnv = { ...process.env, HIVE_DESKTOP_DATA_DIR: dataDirectory };
  delete env.HIVE_DESKTOP_RUNTIME;
  delete env.HIVE_DESKTOP_NODE;
  delete env.HIVE_DESKTOP_UPDATE_READY;
  delete env.HIVE_DESKTOP_UPDATE_AUTHORIZE;
  if (readiness) {
    env.HIVE_DESKTOP_UPDATE_READY = readiness;
    env.HIVE_DESKTOP_UPDATE_AUTHORIZE = '1';
  }
  return spawn(desktopPackagePaths(root).executable, [], { cwd: root, env, stdio: 'ignore', detached: true });
}

async function waitForReadiness(child: ChildProcess, file: string, version: string) {
  const deadline = Date.now() + 30_000;
  while (Date.now() < deadline) {
    if (child.exitCode !== null || child.signalCode !== null) throw new Error(`Updated desktop exited before startup completed (${child.exitCode ?? child.signalCode})`);
    try {
      await checkReadiness(file, child.pid!, version);
      if (child.exitCode !== null || child.signalCode !== null) throw new Error('Updated desktop exited during startup');
      return;
    } catch (error) {
      if (!missing(error)) throw error;
    }
    await delay(100);
  }
  throw new Error('Updated desktop did not finish startup');
}

async function checkReadiness(file: string, pid: number, version: string) {
  const value = JSON.parse(await readFile(file, 'utf8'));
  if (value.pid !== pid || value.version !== version) throw new Error('Updated desktop reported an unexpected startup identity');
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

function installLockPath(installation: string): string {
  const key = createHash('sha256').update(process.platform === 'win32' ? installation.toUpperCase() : installation).digest('hex').slice(0, 24);
  return join(dirname(installation), `.hive-desktop-update-${key}.sqlite`);
}

function recoveryEntry(transaction: string) {
  const suffix = /^\.hive-desktop-install-([a-zA-Z0-9]+)$/.exec(basename(transaction))?.[1];
  if (!suffix) return undefined;
  return join(dirname(transaction), `Recover Hive Desktop ${suffix}.${process.platform === 'win32' ? 'cmd' : 'command'}`);
}

export async function prepareDesktopRecoveryEntry(transaction: string) {
  const entry = recoveryEntry(transaction);
  if (!entry) throw new Error('Invalid desktop update directory name');
  const folder = basename(transaction);
  const script = process.platform === 'win32'
    ? `@echo off\r\necho Close all Hive Desktop windows before continuing. Press Ctrl+C to cancel.\r\npause\r\n"%~dp0${folder}\\node.exe" "%~dp0${folder}\\installer.mjs" --recover\r\npause\r\n`
    : `#!/bin/sh\ncd "$(dirname "$0")/${folder}" || exit 1\nprintf '%s' 'Close all Hive Desktop windows, then press Return to continue. ' \nread -r answer || exit 1\nexec ./node ./installer.mjs --recover\n`;
  await writeFile(entry, script, { flag: 'wx', mode: 0o700, flush: true });
  return entry;
}

async function clearRecoveryEntry(transaction: string) {
  const entry = recoveryEntry(transaction);
  if (entry) await rm(entry, { force: true }).catch(() => {});
}

async function transactionPaths(plan: DesktopInstallPlan, transaction: string) {
  if (!isAbsolute(plan.installation) || !isAbsolute(plan.dataDirectory)
    || dirname(transaction) !== dirname(plan.installation) || transaction === plan.installation || await realpath(transaction) !== transaction
    || !/^[a-f0-9]{64}$/.test(plan.oldExecutableSha256) || !/^[a-f0-9]{64}$/.test(plan.newExecutableSha256)) {
    throw new Error('Desktop update transaction must be beside the installation');
  }
  return {
    next: join(transaction, 'next'),
    previous: join(transaction, 'previous'),
    readiness: join(transaction, 'ready.json'),
    journal: join(transaction, 'install-status.json'),
    receipt: join(plan.dataDirectory, 'desktop-updates', 'install-result.json'),
  };
}

async function withInstallLock<T>(installation: string, action: () => Promise<T>) {
  // SQLite holds the lock through OS file handles. Killing the installer releases it.
  // Keep the lock file in place so concurrent processes always open the same file.
  const lock = new DatabaseSync(installLockPath(installation));
  try {
    try { lock.exec('PRAGMA busy_timeout=0; BEGIN IMMEDIATE'); }
    catch (error) {
      if ((error as { errcode?: number }).errcode === 5) throw new Error('Another desktop installer is running', { cause: error });
      throw error;
    }
    return await action();
  } finally {
    lock.close();
  }
}

/** Run only after the original desktop and its sidecar have exited. */
export async function installDesktopUpdate(plan: DesktopInstallPlan, transaction: string) {
  const paths = await transactionPaths(plan, transaction);
  await withInstallLock(plan.installation, () => installLocked(plan, transaction, paths));
}

type TransactionPaths = Awaited<ReturnType<typeof transactionPaths>>;

async function installLocked(plan: DesktopInstallPlan, transaction: string, paths: TransactionPaths) {
  const { installation, dataDirectory, fromVersion, toVersion } = plan;
  const { next, previous, readiness, journal, receipt } = paths;
  if (await exists(journal)) throw new Error('This desktop update has already started. Use the recovery entry to inspect it.');
  let movedOld = false;
  let movedNew = false;
  let startedHealthy = false;
  let candidate: ChildProcess | undefined;
  const record = (state: Journal['state'], error?: string) =>
    atomicJson(journal, { state, fromVersion, toVersion, candidatePid: candidate?.pid, error } satisfies Journal);
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
    // The candidate waits for this persisted PID before opening its data directory.
    await record('starting');
    await waitForReadiness(candidate, readiness, toVersion);
    startedHealthy = true;
    candidate.unref();
    await record('installed');
    await atomicJson(receipt, { state: 'installed', version: toVersion, requestedVersion: toVersion });
    await rm(previous, { recursive: true, force: true }).catch(() => {});
    await clearRecoveryEntry(transaction);
  } catch (error) {
    // Once the application has accepted the workspace, keep it running even
    // if writing the final receipt fails. Recovery can finish that commit.
    if (startedHealthy) throw error;
    const message = error instanceof Error ? error.message : String(error);
    try {
      if (candidate?.pid) await stopCandidate(candidate);
      if (movedNew) await move(installation, next);
      if (movedOld) await move(previous, installation);
      // Before replacement, launch only the verified original and never an unexpected occupant.
      await checkExecutable(installation, plan.oldExecutableSha256);
      await atomicJson(receipt, { state: movedOld ? 'rolledBack' : 'failed', version: fromVersion, requestedVersion: toVersion, error: message });
      await record(movedOld ? 'rolledBack' : 'failed', message);
      const original = launch(installation, dataDirectory);
      await once(original, 'spawn');
      original.unref();
      await clearRecoveryEntry(transaction);
    } catch (recoveryError) {
      const detail = recoveryError instanceof Error ? recoveryError.message : String(recoveryError);
      await record('recoveryRequired', `${message}\n${detail}`);
      throw recoveryError;
    }
  }
}

/** Recover only the package associated with this private transaction directory. */
export async function recoverDesktopUpdate(plan: DesktopInstallPlan, transaction: string) {
  const paths = await transactionPaths(plan, transaction);
  const { installation, dataDirectory, fromVersion, toVersion } = plan;
  const { next, previous, readiness, journal, receipt } = paths;
  return withInstallLock(installation, async () => {
    let state: Journal;
    try { state = JSON.parse(await readFile(journal, 'utf8')) as Journal; }
    catch (error) {
      if (missing(error)) return 'Nothing to recover. The installation was not changed.';
      throw error;
    }
    if (state.fromVersion !== fromVersion || state.toVersion !== toVersion
      || !['replacing', 'starting', 'installed', 'rolledBack', 'failed', 'recoveryRequired'].includes(state.state)
      || (state.candidatePid !== undefined && (!Number.isSafeInteger(state.candidatePid) || state.candidatePid <= 1))) {
      throw new Error('Invalid desktop update journal');
    }
    if (state.state === 'rolledBack' || state.state === 'failed') {
      await checkExecutable(installation, plan.oldExecutableSha256);
      return 'The original installation is already available.';
    }

    const candidateReady = state.candidatePid !== undefined && await checkReadiness(readiness, state.candidatePid, toVersion).then(
      () => true, error => {
        if (missing(error)) return false;
        throw error;
      },
    );
    if (state.state === 'installed' || (candidateReady && await exists(previous) && !await exists(next))) {
      await checkExecutable(installation, plan.newExecutableSha256);
      await atomicJson(journal, { ...state, state: 'installed', error: undefined });
      await atomicJson(receipt, { state: 'installed', version: toVersion, requestedVersion: toVersion });
      await rm(previous, { recursive: true, force: true }).catch(() => {});
      return `Desktop ${toVersion} is installed. You can open it from the normal location.`;
    }
    if (state.candidatePid !== undefined && isLive(state.candidatePid)) {
      throw new Error('Close the updated Hive Desktop window, then run recovery again.');
    }

    if (await exists(previous)) {
      await checkExecutable(previous, plan.oldExecutableSha256);
      if (await exists(installation)) {
        if (await exists(next)) throw new Error('Unexpected package layout. Recovery kept both installations untouched.');
        await checkExecutable(installation, plan.newExecutableSha256);
        await move(installation, next);
      }
      await move(previous, installation);
    }
    await checkExecutable(installation, plan.oldExecutableSha256);
    const message = 'Desktop installation was interrupted.';
    await atomicJson(receipt, { state: 'rolledBack', version: fromVersion, requestedVersion: toVersion, error: message });
    await atomicJson(journal, { ...state, state: 'rolledBack', error: message });
    return `Desktop ${fromVersion} was recovered. You can open it from the normal location.`;
  });
}

async function main() {
  const transaction = await realpath(dirname(process.argv[1]!));
  const plan = JSON.parse(await readFile(join(transaction, 'install-plan.json'), 'utf8')) as DesktopInstallPlan;
  if (process.argv[2] === '--recover') {
    console.log(await recoverDesktopUpdate(plan, transaction));
    return;
  }
  const parent = Number(process.argv[2]);
  if (!Number.isSafeInteger(parent) || parent <= 1) throw new Error('Invalid original desktop process identity');
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

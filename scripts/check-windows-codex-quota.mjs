import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { once } from 'node:events';
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { delimiter, dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { promisify } from 'node:util';
import { createWindowsFixtureProcesses } from './lib/windows-native-fixture.mjs';

const argument = (name) => {
  const index = process.argv.indexOf(name);
  assert(index >= 0 && process.argv[index + 1], `Missing ${name}`);
  return resolve(process.argv[index + 1]);
};
const bundle = argument('--bundle');
const receiptPath = argument('--receipt');
const node = join(bundle, 'resources', 'node', 'node.exe');
const desktopExe = join(bundle, 'hive-desktop.exe');
const peer = fileURLToPath(new URL('./fixtures/windows-codex-quota-peer.cjs', import.meta.url));
const guard = fileURLToPath(new URL('./fixtures/windows-image-network-guard.cjs', import.meta.url));
const ui = fileURLToPath(new URL('./windows-codex-quota-ui.ps1', import.meta.url));
const powershell = join(
  process.env.SystemRoot,
  'System32',
  'WindowsPowerShell',
  'v1.0',
  'powershell.exe',
);
const execFileAsync = promisify(execFile);
const { start, deadline, stopAll } = createWindowsFixtureProcesses();
const result = { status: 'failed', checks: {}, error: null, cleanupErrors: [] };
let root;
let observedOwned = [];
let desktopLog = '';

async function until(check, label, milliseconds = 30000, signal) {
  const end = Date.now() + milliseconds;
  do {
    signal?.throwIfAborted();
    if (await check()) return;
    await new Promise((resolveDelay) => setTimeout(resolveDelay, 100));
  } while (Date.now() < end);
  throw Error(`Timed out: ${label}`);
}
async function untilOrDriverExit(check, label, driver) {
  const controller = new AbortController();
  try {
    return await Promise.race([
      until(check, label, 120000, controller.signal),
      driver.done.then(([code, signal]) => {
        throw Error(`UI driver exited before ${label}: code=${code}, signal=${signal}`);
      }),
    ]);
  } finally {
    controller.abort();
  }
}
async function exists(path) {
  try {
    await readFile(path);
    return true;
  } catch (error) {
    if (error.code === 'ENOENT') return false;
    throw error;
  }
}
async function observerCommand(command) {
  try {
    return await execFileAsync(powershell, ['-NoProfile', '-NonInteractive', '-Command', command], {
      windowsHide: true,
      timeout: 15000,
      maxBuffer: 4 * 1024 * 1024,
    });
  } catch (error) {
    const stderr = String(error.stderr ?? '')
      .slice(0, 3000)
      .replace(/PRIVATE_FIXTURE_[A-Z_]+/g, '[redacted]');
    const failure = new Error(`Process observer failed: ${stderr || error.message}`);
    Object.assign(failure, {
      code: error.code ?? null,
      killed: error.killed ?? null,
      signal: error.signal ?? null,
    });
    throw failure;
  }
}
async function processes() {
  const command =
    'Get-CimInstance -ClassName Win32_Process -Property ProcessId,ParentProcessId,Name,CreationDate | Select-Object ProcessId,ParentProcessId,Name,CreationDate | ConvertTo-Json -Compress';
  const { stdout } = await observerCommand(command);
  const list = JSON.parse(stdout);
  return Array.isArray(list) ? list : [list];
}
async function prepareObserver() {
  result.checks.observerWarmup = [];
  for (let attempt = 1; attempt <= 2; attempt++) {
    const started = Date.now();
    try {
      const snapshot = await processes();
      assert(
        snapshot.some((item) => item.ProcessId === process.pid),
        'Process observer must see the fixture before launching the CLI',
      );
      result.checks.observerWarmup.push({
        attempt,
        elapsedMs: Date.now() - started,
        status: 'ready',
      });
      return;
    } catch (error) {
      const timeout = error.code === null && error.killed === true && error.signal === 'SIGTERM';
      result.checks.observerWarmup.push({
        attempt,
        elapsedMs: Date.now() - started,
        status: timeout ? 'timeout' : 'error',
        error: String(error),
        failure: {
          code: error.code ?? null,
          killed: error.killed ?? null,
          signal: error.signal ?? null,
        },
      });
      if (!(timeout && attempt === 1)) throw error;
    }
  }
}
async function processSnapshot(peerPid, helperPid, unrelatedPid, desktopPid) {
  for (const pid of [peerPid, helperPid, unrelatedPid, desktopPid]) {
    assert(Number.isSafeInteger(pid) && pid > 0);
  }
  // Collect lifecycle fields once. Inspect a command line only for a Node
  // process already established to be on this fixture's ancestor chain.
  const command = `
    $all = @(Get-CimInstance -ClassName Win32_Process -Property ProcessId,ParentProcessId,Name,CreationDate)
    $chain = @(); $current = ${peerPid}; $hostFound = $false
    for ($i = 0; $i -lt 10 -and $current -gt 0 -and $current -ne ${desktopPid}; $i++) {
      $process = $all | Where-Object { $_.ProcessId -eq $current } | Select-Object -First 1
      if (-not $process) { break }
      $chain += $process | Select-Object ProcessId,ParentProcessId,Name,CreationDate
      if ($i -gt 0 -and $process.Name -ieq 'node.exe') {
        $details = Get-CimInstance -ClassName Win32_Process -Filter "ProcessId = $current" -Property ProcessId,CommandLine
        if ($details.CommandLine -like '*owned-windows-cli-host.js*') { $hostFound = $true; break }
      }
      $current = $process.ParentProcessId
    }
    $helper = $all | Where-Object { $_.ProcessId -eq ${helperPid} } | Select-Object ProcessId,ParentProcessId,Name,CreationDate
    $other = $all | Where-Object { $_.ProcessId -eq ${unrelatedPid} } | Select-Object ProcessId,ParentProcessId,Name,CreationDate
    @{ chain = $chain; helper = $helper; other = $other; hostFound = $hostFound } | ConvertTo-Json -Depth 5 -Compress
  `;
  const { stdout } = await observerCommand(command);
  return JSON.parse(stdout);
}
function sameProcess(a, b) {
  return a.ProcessId === b.ProcessId && a.CreationDate === b.CreationDate;
}

try {
  assert.equal(process.platform, 'win32', 'Requires an interactive Windows desktop');
  await mkdir(dirname(receiptPath), { recursive: true });
  root = await mkdtemp(join(tmpdir(), 'hive win quota '));
  const dirs = Object.fromEntries(
    ['bin', 'state', 'pi', 'codex-home', 'profile', 'appdata', 'localappdata', 'tmp'].map(
      (name) => [name, join(root, name)],
    ),
  );
  await Promise.all(Object.values(dirs).map((path) => mkdir(path)));
  await writeFile(join(dirs.state, 'desktop-language.json'), JSON.stringify('en'));
  const shim = join(dirs.bin, 'codex.cmd');
  await writeFile(shim, `@echo off\r\n"${node}" "${peer}" %*\r\n`);
  const env = {
    SystemRoot: process.env.SystemRoot,
    WINDIR: process.env.SystemRoot,
    ComSpec: join(process.env.SystemRoot, 'System32', 'cmd.exe'),
    PSModulePath: join(process.env.SystemRoot, 'System32', 'WindowsPowerShell', 'v1.0', 'Modules'),
    PATHEXT: '.COM;.EXE;.BAT;.CMD',
    PATH: [dirs.bin, join(process.env.SystemRoot, 'System32')].join(delimiter),
    TEMP: dirs.tmp,
    TMP: dirs.tmp,
    USERPROFILE: dirs.profile,
    HOME: dirs.profile,
    APPDATA: dirs.appdata,
    LOCALAPPDATA: dirs.localappdata,
    CODEX_HOME: dirs['codex-home'],
    PI_CODING_AGENT_DIR: dirs.pi,
    PI_OFFLINE: '1',
    HIVE_DESKTOP_DATA_DIR: dirs.state,
    HIVE_QUOTA_FIXTURE: root,
    NODE_OPTIONS: `--require="${guard.replaceAll('\\', '/')}"`,
    HIVE_DESKTOP_TRACE_RPC: '1',
  };
  // The observer is a test tool, not part of the isolated application or CLI.
  // Match the inherited runner environment used by the native CLI checks.
  await prepareObserver();
  const unrelated = start(node, ['-e', 'setInterval(()=>{},1000)'], {
    env,
    stdio: 'ignore',
    windowsHide: true,
  });
  await once(unrelated.child, 'spawn');
  const desktop = start(desktopExe, [], { env, cwd: root, stdio: ['ignore', 'ignore', 'pipe'] });
  desktop.child.stderr.setEncoding('utf8');
  desktop.child.stderr.on('data', (chunk) => {
    desktopLog += chunk;
  });
  const uiReceipt = join(root, 'ui.json');
  const driver = start(
    powershell,
    [
      '-NoProfile',
      '-ExecutionPolicy',
      'Bypass',
      '-File',
      ui,
      '-AppProcessId',
      String(desktop.child.pid),
      '-Executable',
      desktopExe,
      '-FixtureRoot',
      root,
      '-OutputPath',
      uiReceipt,
    ],
    { env, stdio: 'inherit', windowsHide: true },
  );

  await untilOrDriverExit(
    () => exists(join(root, 'quota-ready')),
    'controlled quota request',
    driver,
  );
  const pids = JSON.parse(await readFile(join(root, 'peer-pids.json'), 'utf8'));
  const snapshot = await processSnapshot(
    pids.peer,
    pids.helper,
    unrelated.child.pid,
    desktop.child.pid,
  );
  const chain = snapshot.chain;
  const helper = snapshot.helper;
  const other = snapshot.other;
  assert(
    Array.isArray(chain) && chain.length && helper && other,
    'fixture peer, helper and unrelated process must be visible',
  );
  assert(snapshot.hostFound, 'Windows Job host must be in the fixture ancestor chain');
  assert.equal(chain[0].ProcessId, pids.peer);
  assert.equal(helper.ParentProcessId, pids.peer);
  const owned = [...chain, helper];
  observedOwned = owned;
  result.checks.processesBefore = owned;
  await writeFile(join(root, 'release'), 'release');
  await untilOrDriverExit(() => exists(join(root, 'ui-observed')), 'quota result in UI', driver);
  await until(async () => {
    const after = await processes();
    return owned.every((old) => !after.some((item) => sameProcess(old, item)));
  }, 'owned probe process cleanup');
  const after = await processes();
  assert(
    after.some((item) => sameProcess(other, item)),
    'unrelated process must survive probe cleanup',
  );
  result.checks.ownedProcessesExited = true;
  result.checks.unrelatedProcessAlive = true;
  const requests = (await readFile(join(root, 'requests.jsonl'), 'utf8'))
    .trim()
    .split('\n')
    .map((line) => JSON.parse(line));
  assert.deepEqual(
    requests.map((item) => item.method),
    ['initialize', 'initialized', 'account/read', 'account/rateLimits/read'],
  );
  assert.deepEqual(requests[2].params, { refreshToken: false });
  assert.deepEqual(requests[3].params, { excludeResetCreditDetails: true });
  result.checks.readOnlyProtocol = true;
  await writeFile(join(root, 'close-ui'), 'close');
  assert.deepEqual(await deadline(driver.done, 90000, 'UI did not exit'), [0, null]);
  result.checks.ui = JSON.parse(await readFile(uiReceipt, 'utf8'));
  assert.equal(result.checks.ui.status, 'passed');
  assert.deepEqual(await deadline(desktop.done, 30000, 'desktop did not exit'), [0, null]);
  assert.match(desktopLog, /"shutdown"/);
  assert.match(desktopLog, /reply 0 ok/);
  assert(
    !desktopLog.includes('PRIVATE_FIXTURE_'),
    'private fixture data must not enter desktop logs',
  );
  result.checks.desktopExit = { code: 0, backendAcknowledged: true };
  result.status = 'passed';
} catch (error) {
  result.error = String(error?.stack ?? error).replace(/PRIVATE_FIXTURE_[A-Z_]+/g, '[redacted]');
  process.exitCode = 1;
} finally {
  result.cleanupErrors.push(...(await stopAll()));
  if (observedOwned.length) {
    try {
      await until(
        async () => {
          const remaining = await processes();
          return observedOwned.every((old) => !remaining.some((item) => sameProcess(old, item)));
        },
        'owned probe processes after cleanup',
        10000,
      );
      result.checks.ownedProcessesGoneAfterCleanup = true;
    } catch (error) {
      result.cleanupErrors.push(String(error));
    }
  }
  if (root) {
    try {
      const log = await readFile(join(root, 'requests.jsonl'), 'utf8');
      result.checks.fixtureMethods = log
        .trim()
        .split('\n')
        .filter(Boolean)
        .map((line) => JSON.parse(line).method);
    } catch (error) {
      if (error.code !== 'ENOENT') result.checks.fixtureLogError = String(error);
    }
    try {
      // Keep UI diagnostics even when the driver failed before its success signal.
      if (!result.checks.ui)
        result.checks.ui = JSON.parse(await readFile(join(root, 'ui.json'), 'utf8'));
    } catch {}
    try {
      await rm(root, { recursive: true, force: true });
    } catch (error) {
      result.cleanupErrors.push(String(error));
    }
  }
  if (result.cleanupErrors.length) {
    result.status = 'failed';
    process.exitCode = 1;
  }
  try {
    await mkdir(dirname(receiptPath), { recursive: true });
    await writeFile(receiptPath, JSON.stringify(result, null, 2) + '\n');
    await writeFile(
      join(dirname(receiptPath), 'windows-codex-quota-desktop.log'),
      desktopLog.replace(/PRIVATE_FIXTURE_[A-Z_]+/g, '[redacted]'),
    );
  } catch (error) {
    console.error(error);
    process.exitCode = 1;
  }
}

import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { digest } from './fixtures/windows-image-fixture.mjs';

function argument(name) {
  const index = process.argv.indexOf(name);
  assert.ok(index >= 0 && process.argv[index + 1], `Missing ${name}`);
  return resolve(process.argv[index + 1]);
}

const bundle = argument('--bundle');
const receiptPath = argument('--receipt');
const resources = join(bundle, 'resources');
const executable = join(bundle, 'hive-desktop.exe');
const seedScript = fileURLToPath(new URL('./fixtures/windows-image-seed.mjs', import.meta.url));
const guard = fileURLToPath(new URL('./fixtures/windows-image-network-guard.cjs', import.meta.url));
const uiScript = fileURLToPath(new URL('./windows-history-image-ui.ps1', import.meta.url));
const output = { status: 'failed', fixture: {}, error: null, cleanupErrors: [] };
let root;
let desktop;
let desktopClosed;
let stderr = '';
const running = new Map();

function start(command, args, options) {
  const child = spawn(command, args, options);
  const closed = once(child, 'close').finally(() => running.delete(child));
  running.set(child, closed);
  // Attach a handler immediately, including for a failed spawn.
  closed.catch(() => {});
  return { child, closed };
}

async function run(command, args, options) {
  const { closed } = start(command, args, options);
  const result = await deadline(closed, 120_000, `${command} timed out`);
  assert.deepEqual(result, [0, null], `${command} failed`);
}

async function stopOwned(child) {
  if (!child.pid || child.exitCode !== null || child.signalCode !== null) return;
  if (process.platform === 'win32') {
    const killer = spawn('taskkill.exe', ['/PID', String(child.pid), '/T', '/F'], { stdio: 'ignore' });
    const [code] = await once(killer, 'close');
    if (code !== 0 && child.exitCode === null && child.signalCode === null) {
      throw Error(`Could not terminate fixture process ${child.pid}`);
    }
  } else {
    child.kill();
  }
}

async function deadline(promise, milliseconds, label) {
  let timer;
  try {
    return await Promise.race([
      promise,
      new Promise((_, reject) => { timer = setTimeout(() => reject(Error(label)), milliseconds); }),
    ]);
  } finally {
    clearTimeout(timer);
  }
}

try {
  assert.equal(process.platform, 'win32', 'Requires an interactive Windows desktop');
  await mkdir(dirname(receiptPath), { recursive: true });
  root = await mkdtemp(join(tmpdir(), 'hive-win-image-memory-'));
  // Node parses backslashes in quoted NODE_OPTIONS as escapes on Windows.
  // Forward slashes also work in Windows paths, including paths with spaces.
  const guardOptionPath = guard.replaceAll('\\', '/');
  const env = { ...process.env, NODE_OPTIONS: `--require="${guardOptionPath}"`, PI_OFFLINE: '1' };
  delete env.HIVE_DESKTOP_NODE;
  delete env.HIVE_DESKTOP_RUNTIME;
  // Preparation runs in its own process so that its Hive server and descendants
  // can be reaped even if runtime construction or cleanup gets stuck.
  const seed = start(process.execPath, [seedScript, root, resources], { env, stdio: 'inherit' });
  const seedResult = await deadline(seed.closed, 120_000, 'fixture preparation timed out');
  assert.deepEqual(seedResult, [0, null], 'fixture preparation failed');
  const fixture = JSON.parse(await readFile(join(root, 'seed.json'), 'utf8'));
  output.fixture = {
    rows: 48,
    images: fixture.images,
    historyBefore: fixture.historyBefore,
    sourceBytesVerified: true,
    executableSha256: digest(await readFile(executable)),
  };
  env.HIVE_DESKTOP_DATA_DIR = join(root, 'state');
  env.PI_CODING_AGENT_DIR = join(root, 'offline-pi');
  env.HIVE_DESKTOP_TRACE_RPC = '1';
  const launched = start(executable, [], { cwd: join(root, 'project'), env, stdio: ['ignore', 'ignore', 'pipe'] });
  desktop = launched.child;
  desktopClosed = launched.closed;
  desktop.stderr.setEncoding('utf8');
  desktop.stderr.on('data', data => { stderr += data; });
  const uiReceipt = join(root, 'ui.json');
  const ui = start('powershell.exe', [
    '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', uiScript,
    '-AppProcessId', String(desktop.pid), '-Executable', executable,
    '-OutputPath', uiReceipt, '-EvidenceDirectory', dirname(receiptPath),
  ], { stdio: 'inherit' });
  let uiResult;
  try {
    uiResult = await deadline(ui.closed, 240_000, 'Windows UI fixture timed out');
  } finally {
    try {
      output.window = JSON.parse(await readFile(uiReceipt, 'utf8'));
    } catch {
      // PowerShell stderr and a failure screenshot remain available.
    }
  }
  assert.deepEqual(uiResult, [0, null], 'Windows UI fixture failed');
  const exit = await deadline(desktopClosed, 30_000, 'desktop did not exit');
  assert.deepEqual(exit, [0, null]);
  assert.match(stderr, /reply 0 ok/);
  assert.match(stderr, /"shutdown"/);
  output.shutdown = { exitCode: exit[0], acknowledged: true };
  await run(process.execPath, [seedScript, root, resources, 'verify'], { env, stdio: 'inherit' });
  const verified = JSON.parse(await readFile(join(root, 'verified.json'), 'utf8'));
  output.fixture.historyAfter = verified.historyAfter;
  output.fixture.sourceBytesVerifiedAfterExit = true;
  output.status = 'passed';
} catch (error) {
  output.error = String(error?.stack ?? error);
  process.exitCode = 1;
} finally {
  for (const [child, closed] of [...running]) {
    try {
      await stopOwned(child);
      await deadline(closed.catch(() => {}), 15_000, `fixture process ${child.pid} did not close`);
    } catch (error) {
      output.cleanupErrors.push(String(error));
    }
  }
  try {
    await mkdir(dirname(receiptPath), { recursive: true });
    await writeFile(join(dirname(receiptPath), 'windows-image-desktop.log'), stderr);
  } catch (error) {
    output.cleanupErrors.push(String(error));
  }
  if (root) {
    try {
      await rm(root, { recursive: true, force: true });
    } catch (error) {
      output.cleanupErrors.push(String(error));
    }
  }
  if (output.cleanupErrors.length) {
    output.status = 'failed';
    process.exitCode = 1;
  }
  try {
    await mkdir(dirname(receiptPath), { recursive: true });
    await writeFile(receiptPath, JSON.stringify(output, null, 2) + '\n');
  } catch (error) {
    console.error('Could not write receipt:', error);
    process.exitCode = 1;
  }
}

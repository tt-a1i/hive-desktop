import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
const argument = (name) => {
  const i = process.argv.indexOf(name);
  assert(i >= 0 && process.argv[i + 1], `Missing ${name}`);
  return resolve(process.argv[i + 1]);
};
const bundle = argument('--bundle'),
  receiptPath = argument('--receipt'),
  resources = join(bundle, 'resources');
const executable = join(bundle, 'hive-desktop.exe'),
  node = join(resources, 'node', 'node.exe');
const seed = fileURLToPath(new URL('./fixtures/windows-reference-seed.mjs', import.meta.url));
const guard = fileURLToPath(new URL('./fixtures/windows-image-network-guard.cjs', import.meta.url));
const ui = fileURLToPath(new URL('./windows-reference-ui.ps1', import.meta.url));
let root;
const children = new Map();
const logs = [];
const result = { status: 'failed', stages: {}, error: null, cleanupErrors: [] };
async function deadline(promise, ms, label) {
  let timer;
  try {
    return await Promise.race([
      promise,
      new Promise((_, reject) => {
        timer = setTimeout(() => reject(Error(label)), ms);
      }),
    ]);
  } finally {
    clearTimeout(timer);
  }
}
function start(command, args, options) {
  const child = spawn(command, args, options);
  const done = once(child, 'close').finally(() => children.delete(child));
  children.set(child, done);
  done.catch(() => {});
  return { child, done };
}
async function run(command, args, options, label, timeout = 120000) {
  const { done } = start(command, args, options);
  assert.deepEqual(await deadline(done, timeout, label), [0, null], `${label} failed`);
}
try {
  assert.equal(process.platform, 'win32', 'Requires an interactive Windows desktop');
  await mkdir(dirname(receiptPath), { recursive: true });
  root = await mkdtemp(join(tmpdir(), 'hive-win-reference-'));
  const env = {
    ...process.env,
    PI_OFFLINE: '1',
    HIVE_DESKTOP_DATA_DIR: join(root, 'state'),
    PI_CODING_AGENT_DIR: join(root, 'offline-pi'),
    NODE_OPTIONS: `--require="${guard.replaceAll('\\', '/')}"`,
    HIVE_DESKTOP_TRACE_RPC: '1',
  };
  delete env.HIVE_DESKTOP_NODE;
  delete env.HIVE_DESKTOP_RUNTIME;
  await run(node, [seed, root, resources], { env, stdio: 'inherit' }, 'seed');
  const fixture = JSON.parse(await readFile(join(root, 'reference-seed.json'), 'utf8'));
  result.fixture = {
    sessionId: fixture.sessionId,
    sha256: fixture.sha256,
    relativePath: fixture.relativePath,
  };
  for (const phase of ['select', 'reopen']) {
    const desktop = start(executable, [], {
      cwd: fixture.project,
      env,
      stdio: ['ignore', 'ignore', 'pipe'],
    });
    const log = { phase, stderr: '' };
    logs.push(log);
    desktop.child.stderr.setEncoding('utf8');
    desktop.child.stderr.on('data', (chunk) => {
      log.stderr += chunk;
    });
    const uiReceipt = join(root, `reference-ui-${phase}.json`);
    const powershell = start(
      'powershell.exe',
      [
        '-NoProfile',
        '-ExecutionPolicy',
        'Bypass',
        '-File',
        ui,
        '-AppProcessId',
        String(desktop.child.pid),
        '-Executable',
        executable,
        '-Phase',
        phase,
        '-Sha256',
        fixture.sha256,
        '-Worktree',
        fixture.worktree,
        '-OutputPath',
        uiReceipt,
        '-EvidenceDirectory',
        dirname(receiptPath),
      ],
      { stdio: 'inherit' },
    );
    let uiExit;
    try {
      uiExit = await deadline(powershell.done, 120000, `${phase} UI timed out`);
    } finally {
      try {
        result.stages[phase] = JSON.parse(await readFile(uiReceipt, 'utf8'));
      } catch {}
    }
    assert.deepEqual(uiExit, [0, null], `${phase} UI failed`);
    assert.deepEqual(await deadline(desktop.done, 30000, `${phase} desktop exit`), [0, null]);
    assert.match(log.stderr, /"shutdown"/, 'desktop must acknowledge normal shutdown');
    await run(
      node,
      [seed, root, resources, phase === 'select' ? 'before' : 'after'],
      { env, stdio: 'inherit' },
      `${phase} disk verification`,
    );
    result.stages[`${phase}Disk`] = JSON.parse(
      await readFile(
        join(root, `reference-${phase === 'select' ? 'before' : 'after'}.json`),
        'utf8',
      ),
    );
  }
  result.status = 'passed';
} catch (error) {
  result.error = String(error?.stack ?? error);
  process.exitCode = 1;
} finally {
  for (const [child, done] of [...children]) {
    try {
      if (child.pid && child.exitCode === null && child.signalCode === null) {
        const killer = spawn('taskkill.exe', ['/PID', String(child.pid), '/T', '/F'], {
          stdio: 'ignore',
        });
        const [code] = await once(killer, 'close');
        if (code !== 0 && child.exitCode === null && child.signalCode === null)
          throw Error(`Could not stop fixture ${child.pid}`);
      }
      await deadline(
        done.catch(() => {}),
        15000,
        'fixture cleanup',
      );
    } catch (error) {
      result.cleanupErrors.push(String(error));
    }
  }
  if (root) {
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
      join(dirname(receiptPath), 'windows-project-reference-desktop.log'),
      logs.map((log) => `${log.phase}\n${log.stderr}`).join('\n'),
    );
  } catch (error) {
    console.error(error);
    process.exitCode = 1;
  }
}

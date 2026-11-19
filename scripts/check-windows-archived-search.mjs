import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

function argument(name) {
  const index = process.argv.indexOf(name);
  assert.ok(index >= 0 && process.argv[index + 1], `Missing ${name}`);
  return resolve(process.argv[index + 1]);
}
const bundle = argument('--bundle');
const receipt = argument('--receipt');
const resources = join(bundle, 'resources');
const executable = join(bundle, 'hive-desktop.exe');
const node = join(resources, 'node', 'node.exe');
const handoff = process.argv.includes('--handoff-range');
const reading = process.argv.includes('--reading-position');
assert.ok(!(handoff && reading), 'Select at most one scenario');
const seedName = reading ? './fixtures/windows-reading-position-seed.mjs' :
  handoff ? './fixtures/windows-handoff-range-seed.mjs' : './fixtures/windows-archived-search-seed.mjs';
const seedScript = fileURLToPath(new URL(seedName, import.meta.url));
const uiScript = fileURLToPath(new URL('./windows-archived-search-ui.ps1', import.meta.url));
const guard = fileURLToPath(new URL('./fixtures/windows-image-network-guard.cjs', import.meta.url));
const output = { status: 'failed', error: null, cleanupErrors: [] };
const children = new Map();
let root;
let env;
let stderr = '';
function start(command, args, options) {
  const child = spawn(command, args, options);
  const closed = once(child, 'close').finally(() => children.delete(child));
  children.set(child, closed);
  closed.catch(() => {});
  return { child, closed };
}
async function deadline(promise, ms, label) {
  let timer;
  try { return await Promise.race([promise, new Promise((_, reject) => { timer = setTimeout(() => reject(Error(label)), ms); })]); }
  finally { clearTimeout(timer); }
}
async function run(command, args, options, label) {
  const { closed } = start(command, args, options);
  assert.deepEqual(await deadline(closed, 120_000, `${label} timed out`), [0, null], `${label} failed`);
}
try {
  assert.equal(process.platform, 'win32', 'Requires an interactive Windows desktop');
  await mkdir(dirname(receipt), { recursive: true });
  root = await mkdtemp(join(tmpdir(), 'hive-win-archived-search-'));
  env = { ...process.env, HOME: root, USERPROFILE: root, PI_OFFLINE: '1',
    HIVE_DESKTOP_DATA_DIR: join(root, 'state'), PI_CODING_AGENT_DIR: join(root, 'offline-pi'),
    NODE_OPTIONS: `--require="${guard.replaceAll('\\', '/')}"`, HIVE_DESKTOP_TRACE_RPC: '1',
    GIT_CONFIG_NOSYSTEM: '1', GIT_CONFIG_GLOBAL: 'NUL', GIT_CONFIG_SYSTEM: 'NUL', GIT_TERMINAL_PROMPT: '0' };
  for (const key of Object.keys(env)) {
    if (key.startsWith('GH_') || key.startsWith('GITHUB_')) delete env[key];
  }
  delete env.HIVE_DESKTOP_NODE;
  delete env.HIVE_DESKTOP_RUNTIME;
  await run(node, [seedScript, root, resources], { env, stdio: 'inherit' }, 'fixture setup');
  const seed = JSON.parse(await readFile(join(root, 'archived-seed.json'), 'utf8'));
  output.fixture = { sessionId: seed.sessionId, messageId: seed.messageId, historySha256: seed.historySha256,
    agentRunsBefore: seed.agentRunsBefore, execution: seed.execution };
  const phases = reading ? ['first', 'cold'] : ['single'];
  if (reading) output.readingPhases = {};
  for (const phase of phases) {
    const desktop = start(executable, [], { cwd: join(root, 'project'), env, stdio: ['ignore', 'ignore', 'pipe'] });
    let phaseStderr = '';
    desktop.child.stderr.setEncoding('utf8');
    desktop.child.stderr.on('data', chunk => { stderr += chunk; phaseStderr += chunk; });
    const uiReceipt = join(root, `archived-ui-${phase}.json`);
    const scenarioArgs = reading ? ['-Scenario', 'reading-position', '-Phase', phase,
      '-StateDirectory', join(root, 'state'), '-SessionId', seed.sessionId, '-WorkspaceId', seed.workspaceId] :
      handoff ? ['-Scenario', 'handoff-range', '-SessionId', seed.sessionId, '-OriginAgentId', seed.originAgentId] : [];
    const ui = start('powershell.exe', ['-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', uiScript,
      '-AppProcessId', String(desktop.child.pid), '-Executable', executable,
      '-Worktree', seed.worktree, ...scenarioArgs, '-OutputPath', uiReceipt, '-EvidenceDirectory', dirname(receipt)], { stdio: 'inherit' });
    let uiExit;
    let windowReceipt;
    try { uiExit = await deadline(ui.closed, 120_000, `UIA ${phase} timed out`); }
    finally {
      try {
        windowReceipt = JSON.parse(await readFile(uiReceipt, 'utf8'));
        if (reading) output.readingPhases[phase] = windowReceipt;
        else output.window = windowReceipt;
      } catch {}
    }
    assert.deepEqual(uiExit, [0, null], `UIA ${phase} failed`);
    assert.equal(windowReceipt?.status, 'passed');
    assert.equal(windowReceipt?.processesExited, true);
    assert.deepEqual(await deadline(desktop.closed, 30_000, `Desktop ${phase} exit timed out`), [0, null]);
    if (handoff) {
      assert.match(phaseStderr, /"chat.handoff.preview"/);
      assert.doesNotMatch(phaseStderr, /"chat.handoff.apply"|"chat.handoff.recover"/);
    } else if (!reading) { assert.match(phaseStderr, /"chat.search"/); }
    assert.match(phaseStderr, /"chat.open"/);
    assert.match(phaseStderr, /send 0 "shutdown"\s*\[hive rpc\] reply 0 ok/);
    assert.doesNotMatch(phaseStderr, /"chat\.(?:prompt|activate)"|"pi\.auth\.(?!providers")/);
  }
  output.isolatedPiProviderCatalogRead = /"pi\.auth\.providers"/.test(stderr);
  await run(node, [seedScript, root, resources, 'verify'], { env, stdio: 'inherit' }, 'disk verification');
  output.disk = JSON.parse(await readFile(join(root, 'archived-disk.json'), 'utf8'));
  output.shutdownAcknowledged = true;
  output.status = 'passed';
} catch (error) {
  output.error = String(error?.stack ?? error);
  process.exitCode = 1;
} finally {
  for (const [child, closed] of [...children]) {
    try {
      if (child.pid && child.exitCode === null && child.signalCode === null) {
        const killer = spawn('taskkill.exe', ['/PID', String(child.pid), '/T', '/F'], { stdio: 'ignore' });
        const [code] = await once(killer, 'close');
        if (code !== 0 && child.exitCode === null && child.signalCode === null) throw Error(`Could not stop fixture ${child.pid}`);
      }
      await deadline(closed.catch(() => {}), 15_000, 'Fixture cleanup timed out');
    } catch (error) { output.cleanupErrors.push(String(error)); }
  }
  if (root && env && output.fixture && !output.disk) {
    try {
      await run(node, [seedScript, root, resources, 'verify'], { env, stdio: 'inherit' }, 'failure disk verification');
      output.disk = JSON.parse(await readFile(join(root, 'archived-disk.json'), 'utf8'));
    } catch (error) {
      output.diskVerificationError = String(error?.stack ?? error);
    }
  }
  if (root) {
    try { await rm(root, { recursive: true, force: true }); }
    catch (error) { output.cleanupErrors.push(String(error)); }
  }
  if (output.cleanupErrors.length) { output.status = 'failed'; process.exitCode = 1; }
  try {
    await mkdir(dirname(receipt), { recursive: true });
    await writeFile(receipt, JSON.stringify(output, null, 2) + '\n');
    await writeFile(join(dirname(receipt), 'windows-archived-search-desktop.log'), stderr);
  } catch (error) { console.error(error); process.exitCode = 1; }
}

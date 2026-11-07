/** Actual Pi shell operations + production macOS helper. No model or network. */
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { mkdtemp, readFile, writeFile, rm, realpath, mkdir } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

if (process.platform !== 'darwin') throw new Error('macOS parent-exit check requires macOS');
const sdkEntry = process.env.HIVE_SHELL_CHECK_RUNTIME
  ? pathToFileURL(join(process.env.HIVE_SHELL_CHECK_RUNTIME, 'node_modules/@earendil-works/pi-coding-agent/dist/index.js'))
  : new URL('../packages/runtime/node_modules/@earendil-works/pi-coding-agent/dist/index.js', import.meta.url);
const { createLocalBashOperations } = await import(sdkEntry);
const { createLocalShellOperations } = await import(new URL('./core/tools/bash.js', sdkEntry));
const helper = resolve(process.env.HIVE_DESKTOP_SHELL_OWNER || process.argv[2] || 'apps/desktop/target/debug/process-supervisor-check');
process.env.HIVE_DESKTOP_SHELL_OWNER = helper;
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
function alive(pid) {
  try { process.kill(pid, 0); return true; }
  catch (error) { if (error.code === 'ESRCH') return false; throw error; }
}
function killGroup(pid) {
  try { process.kill(-pid, 'SIGKILL'); }
  catch (error) { if (error.code !== 'ESRCH') throw error; }
}
async function until(check, description) {
  const deadline = Date.now() + 8000;
  do { if (await check()) return; await delay(20); } while (Date.now() < deadline);
  throw new Error('Timed out: ' + description);
}
async function pids(root) {
  try {
    const text = await readFile(join(root, 'pids'), 'utf8');
    const ids = text.trim().split(/\s+/).map(Number);
    return ids.length === 3 && ids.every(id => Number.isSafeInteger(id) && id > 1) ? ids : undefined;
  } catch (error) { if (error.code !== 'ENOENT') throw error; }
}
const command = '/bin/sleep 90 & printf "%s %s %s\\n" "$PPID" "$$" "$!" > pids; wait';
if (process.argv.includes('--child')) {
  await createLocalBashOperations().exec(command, process.argv.at(-1), { onData() {} });
  process.exit(0);
}
const root = await mkdtemp(join(tmpdir(), 'hive-pi-shell-owner-'));
const receipts = [];
let output = '';
const result = await createLocalBashOperations().exec('printf "%s|%s" "$HIVE_SHELL_CHECK" "$PWD"; printf "|stderr" >&2; exit 7', root, {
  env: { ...process.env, HIVE_SHELL_CHECK: '中文工具' }, onData: data => { output += data.toString(); },
});
assert.equal(result.exitCode, 7);
assert.ok(output.includes('中文工具|' + await realpath(root)), output);
assert.ok(output.includes('|stderr'));
receipts.push('stdout, stderr, cwd, environment and exit code');

assert.equal((await createLocalBashOperations().exec('kill -TERM $$', root, { onData() {} })).exitCode, 143);
let stdinOutput = '';
const stdinShell = createLocalShellOperations('sh', () => ({ shell: '/bin/sh', args: ['-s'], commandTransport: 'stdin' }));
assert.equal((await stdinShell.exec('printf "stdin:中文"', root, { onData(data) { stdinOutput += data.toString(); } })).exitCode, 0);
assert.equal(stdinOutput, 'stdin:中文');
receipts.push('signal exit code and stdin command transport');

for (const mode of ['abort', 'timeout']) {
  await rm(join(root, 'pids'), { force: true });
  const controller = new AbortController();
  const operation = createLocalBashOperations().exec(command, root, {
    onData() {}, ...(mode === 'abort' ? { signal: controller.signal } : { timeout: 2 }),
  });
  const rejected = assert.rejects(operation, error => error instanceof Error &&
    (mode === 'abort' ? error.message === 'aborted' : error.message === 'timeout:2'));
  let ids;
  try {
    await until(async () => Boolean(ids = await pids(root)), mode + ' readiness');
    assert.ok(ids.every(alive));
    if (mode === 'abort') controller.abort();
    await rejected;
    await until(() => ids.every(pid => !alive(pid)), mode + ' group exit');
    receipts.push(mode + ': helper, shell and grandchild exited');
  } finally { if (ids) killGroup(ids[0]); }
}
for (const mode of ['node-only', 'node-group']) {
  await rm(join(root, 'pids'), { force: true });
  const child = spawn(process.execPath, [fileURLToPath(import.meta.url), helper, '--child', root], {
    detached: true, stdio: ['ignore', 'ignore', 'inherit'], env: process.env,
  });
  const exit = once(child, 'exit');
  const unrelated = spawn('/bin/sleep', ['90']);
  const unrelatedExit = once(unrelated, 'exit');
  let ids;
  try {
    await until(async () => Boolean(ids = await pids(root)), mode + ' readiness');
    assert.ok(ids.every(alive));
    process.kill(mode === 'node-only' ? child.pid : -child.pid, 'SIGKILL');
    await exit;
    await until(() => ids.every(pid => !alive(pid)), mode + ' cleanup');
    assert.ok(alive(unrelated.pid), 'unrelated process must survive');
    receipts.push({ mode, node: child.pid, helper: ids[0], shell: ids[1], grandchild: ids[2], allExited: true, unrelatedSurvived: true });
  } finally {
    if (ids) killGroup(ids[0]);
    if (child.exitCode === null && child.signalCode === null) killGroup(child.pid);
    unrelated.kill('SIGKILL'); await unrelatedExit;
  }
}
if (process.argv.includes('--native')) {
  const nativeRoot = join(root, 'native');
  await mkdir(nativeRoot);
  const runtime = process.env.HIVE_SHELL_CHECK_RUNTIME;
  assert.ok(runtime, '--native requires HIVE_SHELL_CHECK_RUNTIME pointing to the deployed runtime');
  const wrapper = join(nativeRoot, 'entry.mjs');
  const nodeFile = join(nativeRoot, 'node.json');
  await writeFile(wrapper, `
    import {writeFile} from 'node:fs/promises';
    import {createLocalBashOperations} from ${JSON.stringify(sdkEntry.href)};
    await import(${JSON.stringify(pathToFileURL(join(runtime, 'dist/main.js')).href)});
    await writeFile(${JSON.stringify(nodeFile)}, JSON.stringify({pid:process.pid,parent:process.ppid,owner:process.env.HIVE_DESKTOP_SHELL_OWNER}));
    await createLocalBashOperations().exec(${JSON.stringify(command)}, ${JSON.stringify(nativeRoot)}, {onData(){}});
  `);
  const env = { ...process.env, HIVE_DESKTOP_RUNTIME: wrapper, HIVE_DESKTOP_DATA_DIR: join(nativeRoot, 'state') };
  delete env.HIVE_DESKTOP_SHELL_OWNER;
  const app = spawn(helper, [], { stdio: ['ignore', 'ignore', 'pipe'], env });
  const appExit = once(app, 'exit');
  let stderr = '';
  app.stderr.on('data', data => { stderr += data.toString(); });
  let ids, node;
  try {
    await until(async () => Boolean(ids = await pids(nativeRoot)), 'native launched shell: ' + stderr);
    node = JSON.parse(await readFile(nodeFile, 'utf8'));
    assert.equal(node.parent, app.pid);
    assert.equal(await realpath(node.owner), await realpath(helper));
    assert.ok(ids.every(alive));
    process.kill(node.pid, 'SIGKILL');
    await until(() => ids.every(pid => !alive(pid)), 'native launched shell group cleanup');
    assert.ok(alive(app.pid), 'desktop window process must survive backend crash');
    receipts.push({ mode: 'native-runtime-launch', app: app.pid, node: node.pid, helper: ids[0], shell: ids[1], grandchild: ids[2], allExited: true, appSurvived: true });
  } finally {
    if (ids) killGroup(ids[0]);
    if (!node) {
      try { node = JSON.parse(await readFile(nodeFile, 'utf8')); }
      catch (error) { if (error.code !== 'ENOENT') throw error; }
    }
    if (node && alive(node.pid)) process.kill(node.pid, 'SIGKILL');
    if (app.exitCode === null && app.signalCode === null) app.kill('SIGTERM');
    await appExit;
  }
}
// An accidental non-detached invocation must fail before starting the shell.
const marker = join(root, 'must-not-start');
const invalid = spawn(helper, ['--hive-owned-shell', String(process.pid), '/bin/sh', '-c', 'touch "$1"', 'sh', marker], { stdio: 'pipe' });
const [code] = await once(invalid, 'exit');
assert.equal(code, 1);
await assert.rejects(readFile(marker), error => error.code === 'ENOENT');
receipts.push('non-detached invocation rejected before execution');
await writeFile(join(root, 'receipt.json'), JSON.stringify(receipts, null, 2));
console.log(JSON.stringify({ ok: true, receipt: join(root, 'receipt.json'), checks: receipts }, null, 2));

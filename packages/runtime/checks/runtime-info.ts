import assert from 'node:assert/strict';
import { spawn, execFileSync } from 'node:child_process';
import { once } from 'node:events';
import { copyFile, mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { createInterface } from 'node:readline';
import { fileURLToPath, pathToFileURL } from 'node:url';

const runtimeRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const require = createRequire(pathToFileURL(join(runtimeRoot, 'dist/main.js')));
const packageInfo = JSON.parse(await readFile(join(runtimeRoot, 'package.json'), 'utf8'));
const hiveInfo = JSON.parse(await readFile(require.resolve('@tt-a1i/hive/package.json'), 'utf8'));
assert.equal(packageInfo.name, '@hive-desktop/runtime');
assert.equal(hiveInfo.name, '@tt-a1i/hive');
const dir = await mkdtemp(join(tmpdir(), 'hive-runtime-info-'));
const data = join(dir, 'data');
const pi = join(dir, 'pi');
await mkdir(pi);
const child = spawn(process.execPath, [join(runtimeRoot, 'dist/main.js'), '--data-dir', data], {
  cwd: dir, env: { ...process.env, PI_CODING_AGENT_DIR: pi, PI_OFFLINE: '1' }, stdio: ['pipe', 'pipe', 'pipe'],
});
const closed = once(child, 'close');
let stderr = '';
child.stderr.on('data', chunk => { stderr += chunk; });
const lines = createInterface({ input: child.stdout });
let next = 0;
const pending = new Map<number, { resolve: (value: any) => void; reject: (error: Error) => void }>();
lines.on('line', line => { const frame = JSON.parse(line); pending.get(frame.id)?.resolve(frame); });
child.on('close', () => { for (const request of pending.values()) request.reject(new Error(`Runtime closed: ${stderr}`)); });
async function rpc(method: string) {
  const id = ++next;
  let timer: ReturnType<typeof setTimeout> | undefined;
  try {
    return await new Promise<any>((resolve, reject) => {
      pending.set(id, { resolve, reject });
      timer = setTimeout(() => reject(new Error(`Timed out: ${method}; ${stderr}`)), 30000);
      child.stdin.write(JSON.stringify({ id, method, params: {} }) + '\n');
    });
  } finally { if (timer) clearTimeout(timer); pending.delete(id); }
}
try {
  const response = await rpc('initialize');
  assert.equal(response.error, undefined);
  const result = response.result;
  assert.deepEqual(Object.keys(result).sort(), ['dataDir', 'hivePort', 'navigation', 'piAgentDir', 'remoteCommands', 'runtimeInfo', 'version'].sort());
  assert.equal(result.dataDir, data);
  assert.equal(result.piAgentDir, pi);
  assert.ok(Number.isInteger(result.hivePort) && result.hivePort > 0);
  assert.ok('navigation' in result && 'remoteCommands' in result);
  assert.equal(result.version, packageInfo.version);
  assert.deepEqual(result.runtimeInfo, { version: packageInfo.version, nodeVersion: process.version,
    hiveVersion: hiveInfo.version, platform: process.platform, arch: process.arch });
  assert.deepEqual((await rpc('initialize')).result.runtimeInfo, result.runtimeInfo);
  assert.deepEqual((await rpc('shutdown')).result, { ok: true });
  assert.equal((await closed)[0], 0);

  // Execute the actual compiled metadata reader in an isolated package with
  // mismatched Hive metadata and missing, invalid or mismatched runtime metadata.
  const isolated = join(dir, 'isolated');
  await mkdir(join(isolated, 'dist'), { recursive: true });
  const module = join(isolated, 'dist/runtime-info.mjs');
  await copyFile(join(runtimeRoot, 'dist/runtime-info.js'), module);
  const fakeHive = join(isolated, 'node_modules/@tt-a1i/hive');
  await mkdir(fakeHive, { recursive: true });
  await writeFile(join(fakeHive, 'package.json'), JSON.stringify({ name: 'wrong-package', version: '9' }));
  for (const metadata of [undefined, 'not json', JSON.stringify({ name: 'other', version: '9' }), JSON.stringify({ name: '@hive-desktop/runtime', version: '' })]) {
    if (metadata === undefined) await rm(join(isolated, 'package.json'), { force: true });
    else await writeFile(join(isolated, 'package.json'), metadata);
    const output = execFileSync(process.execPath, ['--input-type=module', '-e',
      `import {captureRuntimeInfo} from ${JSON.stringify(pathToFileURL(module).href)}; console.log(JSON.stringify(captureRuntimeInfo()))`], { cwd: dir, encoding: 'utf8' });
    assert.deepEqual(JSON.parse(output), { version: null, nodeVersion: process.version, hiveVersion: null, platform: process.platform, arch: process.arch });
  }
  console.log('RUNTIME_INFO_OK: compiled JSONL initialize and shutdown from unrelated cwd; exact process and package metadata; invalid/missing metadata tolerated. No inference.');
} finally {
  if (child.exitCode === null && child.signalCode === null) { child.kill(); await closed; }
  lines.close();
  await rm(dir, { recursive: true, force: true, maxRetries: 5 });
}

/** Production desktop JSONL, real Hive and isolated Pi configuration; no vendor requests. */
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createInterface } from 'node:readline';

const directory = await mkdtemp(join(tmpdir(), 'hive-auth-wire-'));
const agentDir = join(directory, 'pi');
await mkdir(agentDir);
const settings = JSON.stringify({ defaultProvider: 'openai', defaultModel: 'gpt-6.1-terra' });
await writeFile(join(agentDir, 'settings.json'), settings);
const child = spawn(process.execPath, ['dist/main.js'], { cwd: process.cwd(),
  env: { ...process.env, PI_CODING_AGENT_DIR: agentDir, PI_OFFLINE: '1', HIVE_DESKTOP_DATA_DIR: join(directory, 'desktop') },
  stdio: ['pipe', 'pipe', 'pipe'],
});
const exited = once(child, 'exit');
let stderr = '';
child.stderr.on('data', chunk => { stderr += String(chunk); });
let sequence = 0;
const pending = new Map<number, { resolve(value: any): void; reject(error: Error): void }>();
const lines = createInterface({ input: child.stdout });
lines.on('line', line => {
  const response = JSON.parse(line);
  const request = pending.get(response.id);
  if (request) { pending.delete(response.id); request.resolve(response); }
});
child.on('exit', () => {
  for (const request of pending.values()) request.reject(new Error('Runtime exited before reply: ' + stderr));
  pending.clear();
});
function rpc(method: string, params = {}) {
  return new Promise<any>((resolve, reject) => {
    const id = ++sequence;
    const timeout = setTimeout(() => { pending.delete(id); reject(new Error('Runtime auth request timed out: ' + method)); }, 10_000);
    pending.set(id, { resolve: value => { clearTimeout(timeout); resolve(value); }, reject: error => { clearTimeout(timeout); reject(error); } });
    child.stdin.write(JSON.stringify({ id, method, params }) + '\n');
  });
}
try {
  assert.ok((await rpc('initialize')).result.hivePort);
  const catalog = await rpc('pi.auth.providers');
  assert.equal(catalog.error, undefined);
  assert.equal(catalog.result.agentDir, agentDir);
  assert.ok(catalog.result.providers.some((provider: any) => provider.id === 'openai' && provider.storedType === null));
  assert.deepEqual(catalog.result.providers.find((provider: any) => provider.id === 'anthropic').methods.map((method: any) => method.type), ['oauth', 'api_key']);
  assert.equal(catalog.result.login, null);
  assert.equal((await rpc('pi.auth.status')).result, null);
  assert.ok((await rpc('pi.auth.start', { providerId: 'invalid-provider' })).error);
  assert.ok((await rpc('pi.auth.respond', { loginId: 'stale', promptId: 'stale', value: 'unconsumed' })).error);
  assert.ok((await rpc('pi.auth.cancel', { loginId: 'stale' })).error);
  assert.ok((await rpc('pi.auth.start', { providerId: 'anthropic', authType: 'invalid' })).error);
  const start = await rpc('pi.auth.start', { providerId: 'anthropic', authType: 'api_key' });
  assert.equal(start.result.authType, 'api_key');
  async function statusUntil(predicate: (state: any) => boolean) {
    const end = Date.now() + 5000;
    for (;;) {
      const status = await rpc('pi.auth.status');
      assert.equal(status.error, undefined);
      if (predicate(status.result)) return status.result;
      assert.ok(Date.now() < end, 'Native API configuration step timed out');
      await new Promise(resolve => setTimeout(resolve, 10));
    }
  }
  const prompt = await statusUntil(state => state?.prompt?.type === 'secret');
  assert.ok((await rpc('pi.auth.respond', { loginId: prompt.id, promptId: prompt.prompt.id, value: '' })).error);
  assert.ok((await rpc('pi.auth.respond', { loginId: prompt.id, promptId: prompt.prompt.id, value: {} })).error);
  assert.equal((await rpc('pi.auth.respond', { loginId: prompt.id, promptId: prompt.prompt.id, value: 'wire-fixture-value' })).error, undefined);
  assert.equal((await statusUntil(state => state?.state !== 'running')).state, 'succeeded');
  const configured = await rpc('pi.auth.providers');
  assert.equal(configured.result.providers.find((provider: any) => provider.id === 'anthropic').storedType, 'api_key');
  assert.ok(!JSON.stringify(configured).includes('wire-fixture-value'));
  const removed = await rpc('pi.auth.logout', { providerId: 'anthropic' });
  assert.equal(removed.error, undefined);
  assert.equal(removed.result.login, null);
  assert.equal(removed.result.providers.find((provider: any) => provider.id === 'anthropic').storedType, null);
  const modelConfig = (await rpc('pi.models.get')).result;
  assert.deepEqual(modelConfig.providers, []);
  const custom = await rpc('pi.models.change', { revision: modelConfig.revision, change: {
    action: 'provider', id: 'wire-custom', name: '本地接口', baseUrl: 'https://example.test/v1', api: 'openai-responses', firstModelId: 'gpt-6.1-terra',
  } });
  assert.equal(custom.error, undefined);
  assert.equal(custom.result.conflict, false);
  assert.equal(custom.result.snapshot.providers[0].models[0].id, 'gpt-6.1-terra');
  assert.equal((await rpc('pi.auth.providers')).result.providers.find((provider: any) => provider.id === 'wire-custom').registered, true);
  const stale = await rpc('pi.models.change', { revision: modelConfig.revision, change: { action: 'removeProvider', id: 'wire-custom' } });
  assert.equal(stale.result.conflict, true);
  const deleted = await rpc('pi.models.change', { revision: custom.result.snapshot.revision, change: { action: 'removeProvider', id: 'wire-custom' } });
  assert.equal(deleted.result.conflict, false);
  assert.deepEqual(deleted.result.snapshot.providers, []);
  assert.equal((await rpc('pi.auth.providers')).result.providers.find((provider: any) => provider.id === 'wire-custom'), undefined);
  assert.equal(await readFile(join(agentDir, 'settings.json'), 'utf8'), settings);
  assert.equal((await rpc('shutdown')).result.ok, true);
  await exited;
  assert.equal(child.exitCode, 0);
  console.log('PI_AUTH_WIRE_OK: production JSONL provider methods, native Anthropic fixture key setup/removal, invalid/stale/empty input, unchanged default model and clean shutdown. No API requests.');
} finally {
  if (child.exitCode === null && child.signalCode === null) { child.kill('SIGKILL'); await exited; }
  lines.close();
  await rm(directory, { recursive: true, force: true });
}

/** Real JSONC files + real SDK catalogs, no vendor/model requests or personal configuration. */
import assert from 'node:assert/strict';
import { chmod, lstat, mkdir, mkdtemp, readFile, readdir, rm, symlink, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { ModelRuntime, readStoredCredential } from '@earendil-works/pi-coding-agent';
import { PiModelConfig } from '../src/pi-model-config.js';
import { PiAuth } from '../src/pi-auth.js';

const dir = await mkdtemp(join(tmpdir(), 'hive-model-config-'));
const path = join(dir, 'pi/models.json');
const authPath = join(dir, 'auth.json');
const service = new PiModelConfig(path);
const createRuntime = () => ModelRuntime.create({ modelsPath: path, authPath, refreshOnCreate: false });
const auth = new PiAuth(createRuntime);
const provider = { action: 'provider', id: 'hive-custom', name: '本地接口', baseUrl: 'https://example.test/v1', api: 'openai-responses', firstModelId: 'gpt-6.1-terra' };
const model = { action: 'model', id: provider.id, modelId: 'gpt-6.1-terra', name: 'Terra 自定义', contextWindow: 262144, maxTokens: 32768, reasoning: true, vision: true };
async function change(value: unknown) { return service.change(service.get().revision, value); }
try {
  assert.equal(service.get().providers.length, 0);
  await assert.rejects(service.change('invalid', provider));
  assert.equal((await change(provider)).conflict, false);
  const sdk = await createRuntime();
  assert.equal(sdk.getModel(provider.id, model.modelId)?.baseUrl, provider.baseUrl);
  assert.equal((await auth.providers()).providers.find(entry => entry.id === provider.id)?.methods[0]?.type, 'api_key');
  await change(model);
  const updated = (await createRuntime()).getModel(provider.id, model.modelId)!;
  assert.equal(updated.contextWindow, 262144);
  assert.equal(updated.maxTokens, 32768);
  assert.equal(updated.reasoning, true);
  assert.deepEqual(updated.input, ['text', 'image']);
  const canonical = JSON.parse(await readFile(path, 'utf8'));
  canonical.providers[provider.id].apiKey = '!touch ' + join(dir, 'COMMAND_MUST_NOT_RUN');
  canonical.providers[provider.id].headers = { 'X-Fixture': 'preserve-header-value' };
  canonical.providers[provider.id].modelOverrides = { [model.modelId]: { name: '覆盖名称', contextWindow: 131072 } };
  canonical.providers[provider.id].models[0].samplingParams = { temperature: 0.25 };
  const decorated = '\uFEFF// keep this comment\r\n' + JSON.stringify(canonical, null, 2).replace(/\n/g, '\r\n').replace(/\r\n}$/, ',\r\n}\r\n');
  await writeFile(path, decorated);
  assert.equal(service.get().providers[0]!.models[0]!.name, '覆盖名称');
  await change({ ...model, name: '改名后', vision: false });
  const preserved = await readFile(path, 'utf8');
  assert.ok(preserved.startsWith('\uFEFF// keep this comment\r\n'));
  assert.ok(preserved.includes('preserve-header-value'));
  assert.ok(preserved.includes('COMMAND_MUST_NOT_RUN'));
  assert.ok(!JSON.stringify(service.get()).includes('preserve-header-value'));
  assert.ok(!JSON.stringify(service.get()).includes('!touch'));
  const native = (await createRuntime()).getModel(provider.id, model.modelId)!;
  assert.equal(native.name, '改名后');
  assert.equal(native.contextWindow, model.contextWindow);
  assert.deepEqual(native.samplingParams, { temperature: 0.25 });
  assert.deepEqual(native.input, ['text']);
  assert.ok(!(await readdir(dir)).includes('COMMAND_MUST_NOT_RUN'));

  const stale = service.get();
  const external = preserved + '// external edit\r\n';
  await writeFile(path, external);
  assert.equal((await service.change(stale.revision, provider)).conflict, true);
  assert.equal(await readFile(path, 'utf8'), external);
  const revision = service.get().revision;
  const concurrent = await Promise.all([service.change(revision, { ...provider, name: '第一次保存' }), service.change(revision, { ...model, name: '不应覆盖' })]);
  assert.deepEqual(concurrent.map(result => result.conflict), [false, true]);
  assert.equal(service.get().providers[0]!.name, '第一次保存');
  assert.equal(service.get().providers[0]!.models[0]!.name, '改名后');
  const beforeInvalid = await readFile(path, 'utf8');
  await assert.rejects(change({ ...model, contextWindow: 0 }));
  await assert.rejects(change({ ...provider, api: 'not-an-api' }));
  await assert.rejects(change({ ...provider, baseUrl: 'file:///tmp/data' }));
  assert.equal(await readFile(path, 'utf8'), beforeInvalid);
  if (process.platform !== 'win32' && process.getuid?.() !== 0) {
    await chmod(path, 0o400);
    try { await assert.rejects(change({ ...provider, name: '拒绝写入' }), { code: 'EACCES' }); }
    finally { await chmod(path, 0o600); }
    assert.equal(await readFile(path, 'utf8'), beforeInvalid);
  }
  const leftovers = (await readdir(join(dir, 'pi'))).filter(name => name.startsWith('.models-'));
  assert.deepEqual(leftovers, []);

  // Native SDK authentication can use the new custom provider, then remains manageable after its removal.
  await auth.start(provider.id, 'api_key');
  const end = Date.now() + 5000;
  while (!auth.status()?.prompt) { assert.ok(Date.now() < end); await new Promise(resolve => setTimeout(resolve, 5)); }
  const login = auth.status()!;
  auth.respond(login.id, login.prompt!.id, 'fixture-custom-key');
  while (auth.status()?.state === 'running') { assert.ok(Date.now() < end); await new Promise(resolve => setTimeout(resolve, 5)); }
  assert.equal(auth.status()!.state, 'succeeded');
  await change({ action: 'removeProvider', id: provider.id });
  const orphan = (await auth.providers()).providers.find(entry => entry.id === provider.id);
  assert.equal(orphan?.registered, false);
  assert.equal(orphan?.storedType, 'api_key');
  await auth.logout(provider.id);
  assert.equal(readStoredCredential(provider.id, authPath), undefined);
  assert.equal((await auth.providers()).providers.find(entry => entry.id === provider.id), undefined);

  await change({ action: 'provider', id: 'openai', name: '内置接口覆盖', baseUrl: 'https://example.test/proxy', api: null, firstModelId: null });
  await change({ ...model, id: 'openai' });
  const inherited = (await createRuntime()).getModel('openai', model.modelId)!;
  assert.equal(inherited.baseUrl, 'https://example.test/proxy');
  assert.equal(inherited.contextWindow, model.contextWindow);
  assert.ok(inherited.api.startsWith('openai-'));
  await change({ action: 'removeModel', id: 'openai', modelId: model.modelId });
  assert.equal(service.get().providers.find(provider => provider.id === 'openai')!.models.length, 0);
  await change({ action: 'removeProvider', id: 'openai' });
  assert.deepEqual(service.get().providers, []);

  await change(provider);
  await change({ ...model, modelId: ' spaced-id ', name: '原名' });
  await change({ ...model, modelId: ' spaced-id ', name: '保留原始 ID' });
  assert.equal(service.get().providers[0]!.models.length, 2);
  assert.equal(service.get().providers[0]!.models.find(model => model.id === ' spaced-id ')!.name, '保留原始 ID');
  await change({ action: 'removeProvider', id: provider.id });

  await writeFile(path, '{bad-json');
  assert.throws(() => service.get());
  await assert.rejects(service.change(revision, provider));
  assert.equal(await readFile(path, 'utf8'), '{bad-json');
  await writeFile(path, '{"providers":{},"providers":{}}');
  assert.throws(() => service.get());
  await writeFile(path, '{"providers":{}}');
  if (process.platform !== 'win32') {
    const target = join(dir, 'managed-models.json');
    await writeFile(target, '{"providers":{}}');
    await rm(path);
    await symlink(target, path);
    await change(provider);
    assert.equal((await lstat(path)).isSymbolicLink(), true);
    assert.ok((await readFile(target, 'utf8')).includes(provider.id));
  }
  await service.close();
  await assert.rejects(change(provider));
  console.log('PI_MODEL_CONFIG_OK: real SDK/custom auth, JSONC+BOM/CRLF/advanced preservation, stale/concurrent writes, validation, readonly file, symlink target and orphan credentials. No model requests.');
} finally { await auth.close(); await service.close(); await rm(dir, { recursive: true, force: true }); }

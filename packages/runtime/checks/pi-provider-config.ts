/** Real SDK API-key prompts, persistence, model availability and removal. All values are synthetic; no API requests. */
import assert from 'node:assert/strict';
import { chmod, mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { ModelRuntime, readStoredCredential } from '@earendil-works/pi-coding-agent';
import { PiAuth } from '../src/pi-auth.js';

const directory = await mkdtemp(join(tmpdir(), 'hive-provider-config-'));
const authPath = join(directory, 'auth.json');
const runtime = await ModelRuntime.create({ authPath, modelsPath: null, refreshOnCreate: false });
const originalAnthropicKey = process.env.ANTHROPIC_API_KEY;
delete process.env.ANTHROPIC_API_KEY;
// Reuse a static SDK model's schema only; no model is executed. Terra may be cache-only in this pinned SDK.
const template = runtime.getModels('openai')[0];
assert.ok(template);
const terra = { ...template, id: 'gpt-6.1-terra', name: 'Terra availability fixture' };
const id = 'hive-config-fixture';
let ambient = false;
let failSync = false;
let holdRefresh = false;
let refreshEntered = false;
let releaseRefresh: (() => void) | undefined;
runtime.registerNativeProvider({
  id, name: '多字段 API 配置', getModels: () => [{ ...terra, provider: id }],
  stream() { throw new Error('Model requests forbidden'); }, streamSimple() { throw new Error('Model requests forbidden'); },
  async refreshModels() {
    if (holdRefresh) { refreshEntered = true; await new Promise<void>(resolve => { releaseRefresh = resolve; }); }
    if (failSync) throw new Error('Fixture catalog synchronization failure');
  },
  auth: { apiKey: {
    name: 'Fixture API configuration',
    async login(interaction) {
      const key = await interaction.prompt({ type: 'secret', message: 'API Key' });
      const account = await interaction.prompt({ type: 'text', message: '账号标识' });
      const optional = await interaction.prompt({ type: 'text', message: '可选字段，留空继续' });
      return { type: 'api_key', key, env: { ACCOUNT: account, OPTIONAL: optional } };
    },
    async resolve({ credential }) {
      return credential?.key ? { auth: { apiKey: credential.key }, env: credential.env, source: 'stored credential' }
        : ambient ? { auth: { apiKey: 'fixture-ambient-value' }, source: 'ambient fixture' } : undefined;
    },
  } },
});
const auth = new PiAuth(async () => runtime);
async function waitFor(predicate: () => boolean) {
  const end = Date.now() + 5000;
  while (!predicate()) { assert.ok(Date.now() < end, 'Provider step timed out'); await new Promise(resolve => setTimeout(resolve, 5)); }
}
async function answer(message: string, value: string) {
  await waitFor(() => auth.status()?.prompt?.message === message);
  const state = auth.status()!;
  return auth.respond(state.id, state.prompt!.id, value);
}
async function configure() {
  const start = await auth.start(id, 'api_key');
  assert.equal(start.authType, 'api_key');
  await waitFor(() => auth.status()?.prompt?.type === 'secret');
  const first = auth.status()!;
  assert.throws(() => auth.respond(first.id, first.prompt!.id, ''));
  await answer('API Key', 'fixture-config-value');
  await answer('账号标识', '本地账号 🐝');
  await answer('可选字段，留空继续', '');
  await waitFor(() => auth.status()?.state !== 'running');
  assert.equal(auth.status()!.state, 'succeeded');
}
try {
  const catalog = await auth.providers();
  assert.deepEqual(catalog.providers.find(provider => provider.id === id)?.methods.map(method => method.type), ['api_key']);
  assert.deepEqual(catalog.providers.find(provider => provider.id === 'anthropic')?.methods.map(method => method.type), ['oauth', 'api_key']);
  await assert.rejects(auth.start(id, 'oauth'));
  await assert.rejects(auth.start(id, 'invalid'));
  await assert.rejects(auth.logout('invalid-provider'));
  await configure();
  const stored = readStoredCredential(id, authPath);
  assert.equal(stored?.type, 'api_key');
  assert.ok(stored?.type === 'api_key');
  assert.equal(stored.key, 'fixture-config-value');
  assert.deepEqual(stored.env, { ACCOUNT: '本地账号 🐝', OPTIONAL: '' });
  assert.ok(runtime.getAvailableSnapshot().some(model => model.provider === id));
  assert.ok(!JSON.stringify(await auth.providers()).includes('fixture-config-value'));
  assert.ok(!JSON.stringify(auth.status()).includes('fixture-config-value'));
  if (process.platform !== 'win32' && process.getuid?.() !== 0) {
    await chmod(authPath, 0o400);
    try {
      await assert.rejects(auth.logout(id), error => {
        const failure = error as Error & { code: string; cause: NodeJS.ErrnoException };
        assert.equal(failure.code, 'auth');
        assert.equal(failure.cause.code, 'EACCES');
        return true;
      });
      assert.deepEqual(readStoredCredential(id, authPath), stored, 'Failed removal must preserve the credential');
      assert.equal(auth.status()!.state, 'succeeded', 'A failed write must not replace the saved-login state');
    } finally { await chmod(authPath, 0o600); }
  }


  const cancelled = await auth.start(id, 'api_key');
  await answer('API Key', 'discarded-fixture-value');
  await waitFor(() => auth.status()?.prompt?.message === '账号标识');
  await assert.rejects(auth.logout(id), 'Removing credentials during setup must fail');
  await auth.cancel(cancelled.id);
  assert.deepEqual(readStoredCredential(id, authPath), stored);
  const removed = await auth.logout(id);
  assert.equal(removed.login, null);
  assert.equal(removed.providers.find(provider => provider.id === id)?.storedType, null);
  assert.equal(readStoredCredential(id, authPath), undefined);
  assert.ok(!runtime.getAvailableSnapshot().some(model => model.provider === id));
  const reopened = await ModelRuntime.create({ authPath, modelsPath: null, refreshOnCreate: false });
  assert.ok(!(await reopened.listCredentials()).some(info => info.providerId === id));

  // The installed Anthropic API-key handler is exercised without a real key or request.
  await auth.start('anthropic', 'api_key');
  await waitFor(() => !!auth.status()?.prompt);
  const native = auth.status()!;
  assert.equal(native.prompt!.type, 'secret');
  auth.respond(native.id, native.prompt!.id, 'fixture-anthropic-value');
  await waitFor(() => auth.status()?.state !== 'running');
  assert.equal(auth.status()!.state, 'succeeded');
  assert.equal(readStoredCredential('anthropic', authPath)?.type, 'api_key');

  await configure();
  ambient = true;
  await auth.logout(id);
  assert.equal(readStoredCredential(id, authPath), undefined);
  assert.equal((await runtime.getAuth(id))?.source, 'ambient fixture');
  assert.ok(runtime.getAvailableSnapshot().some(model => model.provider === id), 'Removal must preserve ambient auth availability');
  ambient = false;
  await configure();
  failSync = true;
  const warning = await auth.logout(id);
  assert.match(warning.message, /凭据已移除/);
  assert.equal(readStoredCredential(id, authPath), undefined);
  failSync = false;

  await configure();
  holdRefresh = true;
  const removal = auth.logout(id);
  await waitFor(() => refreshEntered);
  await assert.rejects(auth.start(id, 'api_key'));
  await assert.rejects(auth.logout('anthropic'));
  let closed = false;
  const close = auth.close().then(() => { closed = true; });
  await new Promise(resolve => setTimeout(resolve, 20));
  assert.equal(closed, false, 'Runtime close must wait for the SDK removal already in progress');
  releaseRefresh!();
  assert.equal((await removal).providers.find(provider => provider.id === id)?.storedType, null);
  await close;
  assert.equal(readStoredCredential(id, authPath), undefined);
  assert.equal(readStoredCredential('anthropic', authPath)?.type, 'api_key', 'Another provider must remain saved');
  console.log('PI_PROVIDER_CONFIG_OK: real SDK/native Anthropic prompts, multi-field/empty text, persistence, cancel, removal, ambient auth, saved-removal warning and shutdown drain. No API/model requests.');
} finally {
  releaseRefresh?.();
  await auth.close();
  if (originalAnthropicKey === undefined) delete process.env.ANTHROPIC_API_KEY;
  else process.env.ANTHROPIC_API_KEY = originalAnthropicKey;
  await rm(directory, { recursive: true, force: true });
}

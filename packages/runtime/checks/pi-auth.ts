/** Real Pi ModelRuntime + locked credential file, synthetic OAuth provider; no model or vendor login requests. */
import assert from 'node:assert/strict';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { ModelRuntime, readStoredCredential } from '@earendil-works/pi-coding-agent';
import { PiAuth } from '../src/pi-auth.js';

const directory = await mkdtemp(join(tmpdir(), 'hive-pi-auth-'));
const authPath = join(directory, 'auth.json');
const modelRuntime = await ModelRuntime.create({ authPath, modelsPath: null, modelsStorePath: join(directory, 'models-store.json'), refreshOnCreate: false });
type Provider = Parameters<ModelRuntime['registerNativeProvider']>[0];
let loginCount = 0;
let callback: (() => void) | undefined;
let failSync = false;
let rejectLogin = false;
const provider: Provider = {
  id: 'hive-oauth-fixture', name: '本地认证夹具',
  getModels: () => [], async refreshModels() { if (failSync) throw new Error('Fixture local catalog failure'); }, stream() { throw new Error('No model requests permitted'); }, streamSimple() { throw new Error('No model requests permitted'); },
  auth: { oauth: {
    name: '测试订阅',
    async login(interaction) {
      loginCount++;
      if (rejectLogin) throw new Error("Fixture authorization denied");
      interaction.notify({ type: 'device_code', userCode: 'HIVE-CODE', verificationUri: 'https://example.test/login' });
      interaction.notify({ type: 'progress', message: '等待授权' });
      const choice = await interaction.prompt({ type: 'select', message: '账号类型', options: [{ id: 'personal', label: '个人' }] });
      assert.equal(choice, 'personal');
      const code = await interaction.prompt({ type: 'manual_code', message: '回填授权码' });
      assert.equal(code, 'local-authorization-code');
      // Callback arrival must clear and reject only this prompt, not cancel the whole login.
      const controller = new AbortController();
      callback = () => controller.abort(new Error('Browser callback won'));
      try { await interaction.prompt({ type: 'manual_code', message: '备用回填', signal: controller.signal }); }
      catch (error) { assert.equal(error, controller.signal.reason); }
      interaction.signal.throwIfAborted();
      return { type: 'oauth', access: 'fixture-access-secret', refresh: 'fixture-refresh-secret', expires: Date.now() + 3600_000 };
    },
    async refresh(credential) { return credential; },
    async toAuth(credential) { return { apiKey: credential.access }; },
  } },
};
modelRuntime.registerNativeProvider(provider);
const auth = new PiAuth(async () => modelRuntime);
async function waitFor(predicate: () => boolean) {
  const until = Date.now() + 5000;
  while (!predicate()) {
    assert.ok(Date.now() < until, 'OAuth state transition timed out');
    await new Promise(resolve => setTimeout(resolve, 10));
  }
}
async function answerThroughCallback() {
  await waitFor(() => auth.status()?.prompt?.type === 'select');
  const first = auth.status()!;
  assert.equal(first.authorization?.type, 'device_code');
  assert.equal(first.notice?.type, 'progress');
  assert.throws(() => auth.respond(first.id, first.prompt!.id, 'nonexistent'));
  auth.respond(first.id, first.prompt!.id, 'personal');
  assert.throws(() => auth.respond(first.id, first.prompt!.id, 'personal'), 'duplicate response must fail');
  await waitFor(() => auth.status()?.prompt?.message === '回填授权码');
  const code = auth.status()!;
  auth.respond(code.id, code.prompt!.id, 'local-authorization-code');
  await waitFor(() => auth.status()?.prompt?.message === '备用回填');
  const pending = auth.status()!;
  callback!();
  await waitFor(() => auth.status()?.state !== 'running');
  assert.throws(() => auth.respond(pending.id, pending.prompt!.id, 'late-code'));
}
try {
  assert.equal((await auth.providers()).providers.find(p => p.id === provider.id)?.storedType, null);
  await assert.rejects(auth.start('does-not-exist'));
  rejectLogin = true;
  await auth.start(provider.id);
  await waitFor(() => auth.status()?.state === 'failed');
  assert.equal(readStoredCredential(provider.id, authPath), undefined);
  rejectLogin = false;
  const start = await auth.start(provider.id);
  await assert.rejects(auth.start(provider.id));
  await answerThroughCallback();
  assert.equal(auth.status()!.id, start.id);
  assert.equal(auth.status()!.state, 'succeeded');
  assert.equal(readStoredCredential(provider.id, authPath)?.type, 'oauth');
  const catalog = await auth.providers();
  assert.equal(catalog.providers.find(p => p.id === provider.id)?.storedType, 'oauth');
  assert.equal(catalog.providers[0]!.id, provider.id, 'Saved accounts should appear first');
  assert.ok(!JSON.stringify(catalog).includes('fixture-access-secret'));
  assert.ok(!JSON.stringify(auth.status()).includes('fixture-refresh-secret'));
  assert.equal(auth.status()!.prompt, undefined);
  assert.equal(auth.status()!.authorization, undefined);
  // Fresh ModelRuntime must observe the SDK's durable credential file.
  const reopened = await ModelRuntime.create({ authPath, modelsPath: null, refreshOnCreate: false });
  assert.ok((await reopened.listCredentials()).some(info => info.providerId === provider.id && info.type === 'oauth'));

  const cancel = await auth.start(provider.id);
  await waitFor(() => !!auth.status()?.prompt);
  await auth.cancel(cancel.id);
  assert.equal(auth.status()!.state, 'cancelled');
  assert.equal(readStoredCredential(provider.id, authPath)?.type, 'oauth', 'cancel must retain an existing credential');
  await assert.rejects(auth.cancel('stale-login'));

  failSync = true;
  await auth.start(provider.id);
  await answerThroughCallback();
  assert.equal(auth.status()!.state, 'succeeded', 'saved credentials must not be reported as lost when availability fails');
  assert.match(auth.status()!.message!, /凭据已保存/);
  failSync = false;
  const last = await auth.start(provider.id);
  await waitFor(() => !!auth.status()?.prompt);
  await auth.close();
  assert.equal(auth.status()!.id, last.id);
  assert.equal(auth.status()!.state, 'cancelled');
  await assert.rejects(auth.start(provider.id));
  assert.equal(loginCount, 5);
  console.log('PI_AUTH_OK: real SDK persistence/reopen, status redaction, prompt callback race, duplicate/stale input, cancel/close and saved-credential sync failure.');
} finally { await auth.close(); await rm(directory, { recursive: true, force: true }); }

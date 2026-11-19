import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { once } from 'node:events';
import { mkdir, mkdtemp, readFile, realpath, rm, stat, writeFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import type { AddressInfo } from 'node:net';
import { tmpdir } from 'node:os';
import { delimiter, join } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { fileURLToPath } from 'node:url';
import { createDesktopRuntime } from '../src/desktop-runtime.js';
import { ChatIndex } from '../src/chat-index.js';

// Owned executable and HTTP server. No vault account or vendor model is contacted.
const root = await realpath(await mkdtemp(join(tmpdir(), 'hive-pi-vault-')));
const data = join(root, 'state');
const agent = join(root, 'pi');
const project = join(root, 'project');
const bin = join(root, 'bin');
const commandState = join(root, 'command.json');
const commandLog = join(root, 'command.jsonl');
const release = join(root, 'release');
const helperPidFile = join(root, 'inherited-output.pid');
const helperSource = `
const fs = require('node:fs');
const release = process.argv[1];
const poll = setInterval(() => { if (fs.existsSync(release)) process.exit(0); }, 20);
setTimeout(() => clearInterval(poll), 30_000);
`;
const modelFile = join(agent, 'models.json');
const keys = ['fixture-'+randomUUID(), 'rotated-'+randomUUID()];
const ref = 'op://Fixture Vault/Test Model/credential';
const provider = 'hive-vault-fixture';
const model = 'fixture-model';
const events: unknown[] = [];
let runtime: Awaited<ReturnType<typeof createDesktopRuntime>> | undefined;
let failure = false;
const requests: { auth: string | undefined; input: string }[] = [];
const server = createServer(async (request, response) => {
  const chunks: Buffer[] = [];
  for await (const chunk of request) chunks.push(Buffer.from(chunk));
  const auth = request.headers.authorization;
  requests.push({ auth, input: Buffer.concat(chunks).toString() });
  const key = auth?.replace(/^Bearer /, '') ?? '';
  if (failure) {
    response.writeHead(401, { 'content-type': 'application/json' });
    response.end(JSON.stringify({ error: { message: `Rejected credential ${key}` } }));
    return;
  }
  if (request.url === '/v1/chat/completions') {
    response.writeHead(200, { 'content-type': 'text/event-stream' });
    const chunk = { id: 'fixture_completion', object: 'chat.completion.chunk', created: 1, model };
    response.write(`data: ${JSON.stringify({...chunk, choices: [{index: 0, delta: {role: 'assistant', content: 'Chat Completions credential accepted'}, finish_reason: null}]})}\n\n`);
    response.write(`data: ${JSON.stringify({...chunk, choices: [{index: 0, delta: {}, finish_reason: 'stop'}]})}\n\n`);
    response.end('data: [DONE]\n\n');
    return;
  }
  const text = `text before ${key} text after`;
  const item = { id: 'fixture_message', type: 'message', role: 'assistant', status: 'completed',
    content: [{ type: 'output_text', text, annotations: [] }] };
  const midpoint = 16 + Math.floor(key.length / 2);
  const output = [
    { type: 'response.created', response: { id: 'fixture_response' } },
    { type: 'response.output_item.added', output_index: 0, item: { ...item, content: [] } },
    ...[text.slice(0, midpoint), text.slice(midpoint)].map(delta => ({
      type: 'response.output_text.delta', output_index: 0, content_index: 0, delta,
    })),
    { type: 'response.output_item.done', output_index: 0, item },
    { type: 'response.completed', response: { id: 'fixture_response', status: 'completed', output: [item],
      usage: { input_tokens: 1, output_tokens: 1, total_tokens: 2 } } },
  ];
  response.writeHead(200, { 'content-type': 'text/event-stream' });
  for (const event of output) response.write(`event: ${event.type}\ndata: ${JSON.stringify(event)}\n\n`);
  response.end();
});
server.listen(0, '127.0.0.1');
await once(server, 'listening');
const endpoint = `http://127.0.0.1:${(server.address() as AddressInfo).port}/v1`;
const settings = JSON.stringify({ defaultProvider: provider, defaultModel: model, defaultProjectTrust: 'always', retry: { enabled: false } });
function models(url: string, api = 'openai-responses') {
  return JSON.stringify({ providers: { [provider]: { api, baseUrl: url,
    models: [{ id: model, name: 'Vault fixture', input: ['text'], contextWindow: 32768, maxTokens: 1024 }] } } });
}
const log = async () => (await readFile(commandLog, 'utf8')).trim().split('\n').filter(Boolean)
  .map(line => JSON.parse(line) as { args: string[]; pid: number });
const command = (mode: string, key = keys[0]) => writeFile(commandState, JSON.stringify({mode,key}));
async function waitFor<T>(poll: () => Promise<T | undefined>): Promise<T> {
  const deadline = Date.now() + 10_000;
  while (Date.now() < deadline) {
    const result = await poll();
    if (result !== undefined) return result;
    await delay(25);
  }
  throw new Error('Timed out waiting for the owned vault fixture');
}

async function waitForExit(pid: number) {
  await waitFor(async () => {
    try { process.kill(pid, 0); }
    catch (error) {
      assert.equal((error as NodeJS.ErrnoException).code, 'ESRCH');
      return true;
    }
    return undefined;
  });
}

try {
  await Promise.all([agent, project, bin].map(path => mkdir(path)));
  await writeFile(modelFile, models(endpoint));
  await writeFile(join(agent, 'settings.json'), settings);
  await writeFile(commandLog, '');
  if (process.platform === 'win32') {
    execFileSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command',
      'Add-Type -TypeDefinition (Get-Content -LiteralPath $env:HIVE_VAULT_FIXTURE_SOURCE -Raw) -ReferencedAssemblies System.dll,System.Web.Extensions.dll -OutputAssembly $env:HIVE_VAULT_FIXTURE_EXE -OutputType ConsoleApplication'],
    {timeout: 30_000, env: {...process.env,
      HIVE_VAULT_FIXTURE_SOURCE: fileURLToPath(new URL('./fixtures/pi-vault-op.cs', import.meta.url)),
      HIVE_VAULT_FIXTURE_EXE: join(bin, 'op.exe'),
    }});
  } else await writeFile(join(bin, 'op'), `#!/usr/bin/env node
const fs = require('node:fs');
const state = JSON.parse(fs.readFileSync(${JSON.stringify(commandState)}));
const args = process.argv.slice(2);
fs.appendFileSync(${JSON.stringify(commandLog)}, JSON.stringify({pid:process.pid,args})+'\\n');
if (state.mode === 'hold') setInterval(()=>{},1000);
else if (state.mode === 'deferred') {
  const interval = setInterval(() => {
    if (!fs.existsSync(${JSON.stringify(release)})) return;
    clearInterval(interval);
    process.stdout.write(state.key);
  }, 20);
}
else if (state.mode === 'inherited-output') {
  const {spawn} = require('node:child_process');
  const helper = spawn(process.execPath, ['-e', ${JSON.stringify(helperSource)}, ${JSON.stringify(release)}], {stdio: ['ignore', 'inherit', 'ignore']});
  fs.writeFileSync(${JSON.stringify(helperPidFile)}, String(helper.pid));
  helper.unref();
  process.stdout.write(state.key);
}
else if (state.mode === 'fail') { process.stderr.write(state.key); process.exitCode = 1; }
else process.stdout.write(state.key);
`, {mode: 0o700});
  process.env.PI_CODING_AGENT_DIR = agent;
  process.env.PI_OFFLINE = '1';
  process.env.PATH = `${bin}${delimiter}${process.env.PATH ?? ''}`;
  runtime = await createDesktopRuntime(data, event => events.push(event));
  const workspace = await runtime.hive.request('POST', '/api/workspaces', { path: project, name: 'Vault test', autostart_orchestrator: false }) as {id:string};
  const chat = await runtime.chats.create(workspace.id, 'pi');
  const open = () => runtime!.chats.open(chat.id);
  const settled = () => waitFor(async () => { const snapshot = await open(); return snapshot.busy ? undefined : snapshot; });
  const initial = await runtime.piVault.view();
  assert.deepEqual(initial.eligible, [{ provider, endpoint }]);
  await assert.rejects(async () => runtime!.piVault.change(initial.revision, initial.configRevision, provider, keys[0]), /op:\/\//);
  const bound = await runtime.piVault.change(initial.revision, initial.configRevision, provider, ref);
  assert.deepEqual(bound.configured, [{ provider, endpoint, stale: false }]);
  assert.ok(!JSON.stringify(bound).includes(ref));
  if (process.platform !== 'win32') assert.equal((await stat(join(data, 'pi-vault.json'))).mode & 0o777, 0o600);
  await assert.rejects(runtime.piVault.change(initial.revision, initial.configRevision, provider, null), /已变化/);
  await runtime.close();
  const index = new ChatIndex(join(data, 'chats'));
  index.save({...index.get(chat.id), model: {provider: 'removed-fixture-provider', id: 'removed-model', name: 'Removed model'}});
  runtime = await createDesktopRuntime(data, event => events.push(event));
  assert.equal((await open()).model, undefined);
  await runtime.chats.refreshPiModels(chat.id);
  await runtime.chats.setModel(chat.id, provider, model);
  assert.equal((await open()).model?.id, model);
  assert.equal(runtime.chats.list(workspace.id).find(item => item.id === chat.id)?.model?.id, model);
  assert.deepEqual(await log(), []);

  await command('fail');
  await runtime.chats.prompt(chat.id, 'Queued vault prompt');
  const failed = await waitFor(async () => { const snapshot = await settled(); return snapshot.deliveryFailure ? snapshot : undefined; });
  assert.equal(failed.pending, 1);
  assert.ok(!JSON.stringify(failed.history).includes('Queued vault prompt'));
  assert.equal(requests.length, 0);
  await command('key');
  await runtime.chats.resume(chat.id);
  const delivered = await waitFor(async () => { const snapshot = await settled(); return snapshot.pending === 0 ? snapshot : undefined; });
  assert.match(JSON.stringify(delivered.history), /text before \[REDACTED\] text after/);
  assert.equal(requests.at(-1)?.auth, `Bearer ${keys[0]}`);

  await command('key', keys[1]);
  failure = true;
  await runtime.chats.prompt(chat.id, 'Check rotation and an HTTP failure');
  const rejected = await waitFor(async () => { const snapshot = await settled(); return snapshot.deliveryFailure ? snapshot : undefined; });
  assert.match(JSON.stringify(rejected.deliveryFailure), /Rejected credential \[REDACTED\]/);
  assert.equal(requests.at(-1)?.auth, `Bearer ${keys[1]}`);
  await runtime.chats.abort(chat.id);

  await command('hold');
  await runtime.chats.prompt(chat.id, 'Cancel while the CLI is running');
  const activePid = await waitFor(async () => (await log()).at(3)?.pid);
  process.kill(activePid, 0);
  await runtime.chats.abort(chat.id);
  assert.throws(() => process.kill(activePid, 0), { code: 'ESRCH' });
  assert.equal((await settled()).pending, 0);
  assert.equal(requests.length, 2);

  await command('deferred');
  await runtime.chats.prompt(chat.id, 'Recheck the destination after credential retrieval');
  await waitFor(async () => (await log()).at(4)?.pid);
  await writeFile(modelFile, models('http://127.0.0.1:1/v1'));
  await writeFile(release, '');
  const retargeted = await waitFor(async () => { const snapshot = await settled(); return snapshot.deliveryFailure ? snapshot : undefined; });
  assert.match(JSON.stringify(retargeted.deliveryFailure), /接口已变更/);
  assert.equal(requests.length, 2);
  await runtime.chats.abort(chat.id);
  const stale = await runtime.piVault.view();
  assert.equal(stale.configured[0]?.stale, true);
  await runtime.chats.prompt(chat.id, 'Never forward an old key after an endpoint edit');
  const refused = await waitFor(async () => { const snapshot = await settled(); return snapshot.deliveryFailure ? snapshot : undefined; });
  assert.match(JSON.stringify(refused.deliveryFailure), /接口已变更/);
  const calls = await log();
  assert.equal(calls.length, 5);
  for (const {args} of calls) assert.deepEqual(args, ['read', '--no-newline', ref]);
  await runtime.chats.abort(chat.id);

  assert.ok(delivered.sessionFile);
  const transcript = await readFile(delivered.sessionFile, 'utf8');
  const store = await readFile(join(data, 'pi-vault.json'), 'utf8');
  assert.ok(store.includes(ref));
  assert.equal(await readFile(join(agent, 'settings.json'), 'utf8'), settings);
  for (const key of keys) {
    assert.ok(!transcript.includes(key));
    assert.ok(!store.includes(key));
    assert.ok(!JSON.stringify(events).includes(key));
    assert.ok(requests.every(request => !request.input.includes(key)));
    assert.ok(!JSON.stringify(await open()).includes(key));
  }
  await writeFile(modelFile, models(endpoint));
  await command('hold');
  await runtime.chats.prompt(chat.id, 'Quit while the CLI is running');
  const closingPid = await waitFor(async () => (await log()).at(5)?.pid);
  process.kill(closingPid, 0);
  await runtime.close();
  assert.throws(() => process.kill(closingPid, 0), { code: 'ESRCH' });
  runtime = await createDesktopRuntime(data, () => {});
  const reopened = await runtime.piVault.view();
  assert.deepEqual(reopened.configured, [{ provider, endpoint, stale: false }]);
  assert.equal((await open()).model?.provider, provider);
  assert.equal((await log()).length, 6);
  const removed = await runtime.piVault.change(reopened.revision, reopened.configRevision, provider, null);
  assert.deepEqual(removed.configured, []);
  assert.deepEqual((await runtime.piVault.view()).configured, []);
  assert.ok((await open()).history.length > 0);

  await writeFile(modelFile, models(endpoint, 'openai-completions'));
  const otherApi = await runtime.piVault.view();
  await runtime.piVault.change(otherApi.revision, otherApi.configRevision, provider, ref);
  const compatProject = join(root, 'completions-project');
  await mkdir(compatProject);
  const compatWorkspace = await runtime.hive.request('POST', '/api/workspaces', {
    path: compatProject, name: 'Chat Completions vault test', autostart_orchestrator: false,
  }) as {id:string};
  const compatChat = await runtime.chats.create(compatWorkspace.id, 'pi');
  await runtime.chats.setModel(compatChat.id, provider, model);
  await command('key');
  failure = false;
  await runtime.chats.prompt(compatChat.id, 'Test a Chat Completions request');
  const completed = await waitFor(async () => {
    const snapshot = await runtime!.chats.open(compatChat.id);
    assert.equal(snapshot.deliveryFailure, undefined, 'Chat Completions delivery must settle successfully');
    return !snapshot.busy && snapshot.pending === 0 ? snapshot : undefined;
  });
  assert.match(JSON.stringify(completed.history), /Chat Completions credential accepted/);
  assert.equal(requests.length, 3);
  assert.equal(requests.at(-1)?.auth, `Bearer ${keys[0]}`);

  await rm(release, {force: true});
  await command('deferred');
  const beforeRevocation = (await log()).length;
  await runtime.chats.prompt(compatChat.id, 'Preserve this input when its reference is removed');
  const revokedPid = await waitFor(async () => (await log()).at(beforeRevocation)?.pid);
  const beforeRemoval = await runtime.piVault.view();
  await runtime.piVault.change(beforeRemoval.revision, beforeRemoval.configRevision, provider, null);
  const revoked = await waitFor(async () => {
    const snapshot = await runtime!.chats.open(compatChat.id);
    return snapshot.busy ? undefined : snapshot;
  });
  assert.equal(requests.length, 3, 'Removing the reference must prevent the suspended read from authorizing a model request');
  assert.throws(() => process.kill(revokedPid, 0), {code: 'ESRCH'});
  assert.equal(revoked.pending, 1);
  assert.match(JSON.stringify(revoked.deliveryFailure), /密码库引用已变化/);
  assert.ok(!JSON.stringify(revoked.history).includes('Preserve this input when its reference is removed'));

  const replacementRef = 'op://Fixture Vault/Replacement Model/credential';
  const unbound = await runtime.piVault.view();
  await runtime.piVault.change(unbound.revision, unbound.configRevision, provider, replacementRef);
  await command('key', keys[1]);
  await runtime.chats.resume(compatChat.id);
  const retried = await waitFor(async () => {
    const snapshot = await runtime!.chats.open(compatChat.id);
    return !snapshot.busy && snapshot.pending === 0 ? snapshot : undefined;
  });
  assert.equal(requests.length, 4);
  assert.equal(requests.at(-1)?.auth, `Bearer ${keys[1]}`);
  assert.deepEqual((await log()).at(-1)?.args, ['read', '--no-newline', replacementRef]);
  assert.match(JSON.stringify(retried.history), /Preserve this input when its reference is removed/);
  for (const key of keys) {
    assert.ok(!JSON.stringify(events).includes(key));
    assert.ok(!JSON.stringify(retried.history).includes(key));
    assert.ok(requests.every(request => !request.input.includes(key)));
  }
  if (process.platform !== 'win32') {
    await rm(release, {force: true});
    await command('inherited-output');
    const beforeInheritedOutput = (await log()).length;
    await runtime.chats.prompt(compatChat.id, 'Quit after the CLI exits with stdout still open');
    const exitedPid = await waitFor(async () => (await log()).at(beforeInheritedOutput)?.pid);
    await waitForExit(exitedPid);
    const helperPid = Number(await readFile(helperPidFile, 'utf8'));
    process.kill(helperPid, 0);
    const closing = runtime.close();
    const timeout = new AbortController();
    try {
      assert.equal(await Promise.race([
        closing.then(() => 'closed'),
        delay(2_000, 'blocked by inherited stdout', {signal: timeout.signal}),
      ]), 'closed');
      await waitForExit(helperPid);
      assert.equal(requests.length, 4);
    } finally {
      timeout.abort();
      await writeFile(release, '');
      await closing;
    }
  }
  console.log('PI_VAULT_OK: CLI lifecycle, SDK loopback, queue recovery, rotation, revocation, retargeting and persisted redaction');
} finally {
  try { await runtime?.close(); }
  finally {
    await new Promise<void>(resolve => server.close(() => resolve()));
    await rm(root, { recursive: true, force: true });
  }
}

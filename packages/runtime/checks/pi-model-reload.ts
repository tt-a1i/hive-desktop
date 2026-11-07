/** Warm native SDK session: removing/restoring its selected model must change send availability. */
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { once } from 'node:events';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { createPiSession } from '../src/agents/pi.js';
import { PiModelConfig } from '../src/pi-model-config.js';

const directory = await mkdtemp(join(tmpdir(), 'hive-model-reload-'));
let requests = 0;
const endpoint = createServer((_, response) => { requests++; response.writeHead(503); response.end(); });
endpoint.listen(0, '127.0.0.1'); await once(endpoint, 'listening');
const address = endpoint.address(); assert.ok(address && typeof address !== 'string');
const agentDir = join(directory, 'pi'); await mkdir(agentDir);
const path = join(agentDir, 'models.json');
const config = { providers: { 'hive-reload': { baseUrl: `http://127.0.0.1:${address.port}/v1`, api: 'openai-responses', apiKey: 'fixture-value', models: [{ id: 'gpt-6.1-terra', name: 'Before reload' }] } } };
const settings = JSON.stringify({ defaultProvider: 'hive-reload', defaultModel: 'gpt-6.1-terra' });
await writeFile(join(agentDir, 'settings.json'), settings);
await writeFile(path, JSON.stringify(config));
const service = new PiModelConfig(path);
let session: Awaited<ReturnType<typeof createPiSession>> | undefined;
try {
  session = await createPiSession({ cwd: directory, agentDir, sessionDir: join(directory, 'sessions'), model: { provider: 'hive-reload', id: 'gpt-6.1-terra' }, onEvent() {} });
  assert.equal(session.currentModel()?.name, 'Before reload');
  await service.change(service.get().revision, { action: 'removeProvider', id: 'hive-reload' });
  await session.refreshModels!();
  assert.equal(session.currentModel(), undefined);
  assert.ok(!session.listModels().some(model => model.provider === 'hive-reload'));
  const messages = session.history().length;
  await assert.rejects(session.prompt('Must be blocked before a request'));
  assert.equal(session.history().length, messages);
  assert.equal(requests, 0, 'An unavailable saved model must not reach the HTTP endpoint');
  config.providers['hive-reload'].models[0]!.name = 'After reload';
  await writeFile(path, JSON.stringify(config));
  await session.refreshModels!();
  assert.equal(session.currentModel()?.name, 'After reload');
  assert.equal(session.currentModel()?.id, 'gpt-6.1-terra');
  assert.equal(requests, 0);
  assert.equal(await readFile(join(agentDir, 'settings.json'), 'utf8'), settings);
  console.log('PI_MODEL_RELOAD_OK: real warm SDK session, removed model blocks before HTTP/history, restored model metadata reloads without changing global defaults.');
} finally { await session?.dispose(); await service.close(); endpoint.close(); await rm(directory, { recursive: true, force: true }); }

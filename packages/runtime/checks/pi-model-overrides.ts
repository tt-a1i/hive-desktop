/** Actual SDK catalog + JSONC file. Never loads real credentials or requests a model. */
import assert from 'node:assert/strict';
import { mkdtemp, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { PiModelConfig } from '../src/pi-model-config.js';
import { createConfigRuntime } from '../src/pi-model-file.js';

const directory = await mkdtemp(join(tmpdir(), 'hive-model-overrides-'));
const path = join(directory, 'models.json');
const service = new PiModelConfig(path);
const provider = 'openai';
const native = await createConfigRuntime(null);
// Read-only static catalog metadata, not an inference/model selection. No model is executed.
const original = native.getAllModels(provider).find(model => model.id.includes('terra') && model.reasoning && model.input.includes('image'))!;
assert.ok(original, 'A real installed catalog entry must demonstrate inherited capabilities');
const id = original.id;
const keep = { samplingParams: { temperature: 0.25 }, headers: { 'X-Fixture': 'preserved' } };
const config = { providers: { [provider]: { apiKey: '!touch ' + join(directory, 'MUST_NOT_EXECUTE'), modelOverrides: {
  [id]: { name: 'Only override', ...keep },
  'unknown-terra-override': { name: 'Unmatched config', contextWindow: 123456 },
} }, 'other': { api: 'openai-responses', baseUrl: 'https://example.test/v1', models: [{ id, name: 'Other provider' }] } } };
const form = { action: 'model', id: provider, modelId: id, name: 'Edited override', contextWindow: 262144, maxTokens: 16384, reasoning: null, vision: null };
const disk = async () => JSON.parse((await readFile(path, 'utf8')).replace(/^\/\/.*\n/, ''));
try {
  await writeFile(path, '// preserve this comment\n' + JSON.stringify(config, null, 2));
  const before = await service.get();
  const entries = before.providers.find(p => p.id === provider)!.models;
  assert.equal(entries.length, 2, 'modelOverrides-only entries must be editable even without models[]');
  const entry = entries.find(m => m.id === id)!;
  assert.equal(entry.name, 'Only override');
  assert.equal(entry.reasoning, null);
  assert.equal(entry.vision, null);
  assert.equal(entry.overrideOnly, true);
  // Inherited numeric fields stay blank rather than being pinned by an unrelated name edit.
  assert.equal(entry.contextWindow, null);
  const changed = await service.change(before.revision, form);
  assert.equal(changed.conflict, false);
  const saved = await disk();
  const p = saved.providers[provider];
  assert.equal(p.models, undefined, 'Editing an override must never create a custom model definition');
  assert.equal(p.modelOverrides[id].name, form.name);
  assert.equal(p.modelOverrides[id].reasoning, undefined);
  assert.equal(p.modelOverrides[id].input, undefined);
  assert.deepEqual(p.modelOverrides[id].samplingParams, keep.samplingParams);
  assert.deepEqual(p.modelOverrides[id].headers, keep.headers);
  assert.deepEqual(saved.providers.other, config.providers.other);
  assert.ok((await readFile(path, 'utf8')).startsWith('// preserve this comment\n'));
  const effective = (await createConfigRuntime(path)).getModel(provider, id)!;
  assert.equal(effective.contextWindow, form.contextWindow);
  assert.equal(effective.reasoning, original.reasoning);
  await service.change((await service.get()).revision, { ...form, reasoning: false, vision: false, contextWindow: null, maxTokens: null });
  const toggled = (await createConfigRuntime(path)).getModel(provider, id)!;
  assert.equal(toggled.reasoning, false);
  assert.equal(toggled.input.includes('image'), false);
  await service.change((await service.get()).revision, { ...form, contextWindow: null, maxTokens: null });
  const restored = (await createConfigRuntime(path)).getModel(provider, id)!;
  assert.equal(restored.reasoning, original.reasoning);
  assert.deepEqual(restored.input, original.input);
  assert.equal(toggled.contextWindow, original.contextWindow);
  assert.equal((await disk()).providers[provider].modelOverrides[id].contextWindow, undefined);
  const stale = await service.get();
  const external = await readFile(path, 'utf8') + '\n';
  await writeFile(path, external);
  assert.equal((await service.change(stale.revision, form)).conflict, true);
  assert.equal(await readFile(path, 'utf8'), external);
  await service.change((await service.get()).revision, { action: 'removeModel', id: provider, modelId: id });
  assert.equal((await disk()).providers[provider].modelOverrides[id], undefined);
  assert.equal((await createConfigRuntime(path)).getModel(provider, id)!.contextWindow, original.contextWindow);
  await service.change((await service.get()).revision, { action: 'removeModel', id: provider, modelId: 'unknown-terra-override' });
  assert.deepEqual((await disk()).providers[provider].modelOverrides, {});
  assert.deepEqual((await service.get()).providers.find(p => p.id === provider)!.models, []);
  assert.deepEqual((await disk()).providers.other, config.providers.other);
  assert.ok(!(await readdir(directory)).includes('MUST_NOT_EXECUTE'));
  console.log('PI_MODEL_OVERRIDES_OK: native defaults, override-only edit/remove, unrecognized entry cleanup, JSONC/advanced preservation, no custom model creation, stale conflict, no credentials or inference.');
} finally { await service.close(); await rm(directory, { recursive: true, force: true }); }

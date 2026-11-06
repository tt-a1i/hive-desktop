/** Actual Hive HTTP + SQLite; all memory entries are synthetic fixtures, no models or dream runs. */
import assert from 'node:assert/strict';
import { mkdir, mkdtemp, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { Database } from '@tt-a1i/hive/dist/src/server/sqlite.js';
import { createTeamMemoryStore } from '@tt-a1i/hive/dist/src/server/team-memory-store.js';
import { createHiveHost, HiveHttpError, type HiveHost } from '../src/hive-host.js';

const artifacts = resolve('../../.artifacts');
await mkdir(artifacts, { recursive: true });
const directory = await mkdtemp(join(artifacts, 'workspace-memory-'));
const desktopDataDir = join(directory, 'state');
const dataDir = join(desktopDataDir, 'hive');
const project = join(directory, 'synthetic-project');
const secondProject = join(directory, 'other-synthetic-project');
await mkdir(project); await mkdir(secondProject);
const receipt: Record<string, unknown> = { inference: false, syntheticFixtures: true, directory, desktopDataDir, dataDir, project,
  fixtureScope: 'Only this isolated runtime.sqlite; synthetic user-scope memory is not real user memory', nativeUiEnvironment: { HIVE_DESKTOP_DATA_DIR: desktopDataDir }, requests: [] };
const requests = receipt.requests as unknown[];
let host: HiveHost | undefined;
type Memory = { id: string; body: string; status: string; scope: string; kind: string; pinned: boolean; disabled: boolean; tags: string[]; archived_at: number | null; created_at: number; updated_at: number; workspace_id: string | null; confidence: number | null; procedure_ref: { type: string; id: string; title: string | null } | null; sources: { source_type: string; actor_name_snapshot: string; excerpt: string }[] };
const request = async <T>(method: string, path: string, body?: unknown): Promise<T> => {
  try { const result = await host!.request(method, path, body); requests.push({ method, path, ...(body !== undefined && { body }), status: 200, result }); return result as T; }
  catch (error) { requests.push({ method, path, ...(body !== undefined && { body }), status: error instanceof HiveHttpError ? error.status : null, error: error instanceof HiveHttpError ? error.body : String(error) }); throw error; }
};
const fails = (operation: Promise<unknown>, status: number) => assert.rejects(operation, error => error instanceof HiveHttpError && error.status === status);
try {
  host = await createHiveHost({ dataDir });
  const workspace = await request<{ id: string }>('POST', '/api/workspaces', { path: project, name: 'Synthetic memory verification', autostart_orchestrator: false });
  const second = await request<{ id: string }>('POST', '/api/workspaces', { path: secondProject, name: 'Synthetic isolation workspace', autostart_orchestrator: false });
  const base = `/api/ui/workspaces/${workspace.id}/memory`;
  receipt.workspaceId = workspace.id; receipt.secondWorkspaceId = second.id;
  const defaults = await request<{ enabled: boolean; dream_enabled: boolean }>('GET', `${base}/settings`);
  assert.equal(defaults.enabled, true);
  await request('PUT', `${base}/settings`, { enabled: true, dream_enabled: false });
  await request('PUT', `/api/ui/workspaces/${second.id}/memory/settings`, { enabled: true, dream_enabled: false });
  assert.deepEqual((await request<{ memories: Memory[] }>('GET', base)).memories, []);
  await host.close(); host = undefined;

  // Seed only synthetic data through the installed real store, using its existing initialized SQLite schema.
  const db = new Database(join(dataDir, 'runtime.sqlite'));
  let ids: Record<string, string>;
  try {
    const store = createTeamMemoryStore(db);
    const actor = { id: 'synthetic-orchestrator', name: 'Synthetic memory fixture', role: 'orchestrator' as const };
    const worker = { id: 'synthetic-worker', name: 'Synthetic candidate fixture', role: 'coder' as const };
    const add = (body: string, candidate = false, scope: 'workspace' | 'user' = 'workspace', workspaceId = workspace.id) => store.addEntry({ actor: candidate ? worker : actor, body, scope, workspaceId, kind: 'fact', tags: ['synthetic', 'verification'] });
    ids = { active: add('SYNTHETIC alpha workspace fact 中文检索标记').id,
      approve: add('SYNTHETIC candidate to approve', true).id,
      reject: add('SYNTHETIC candidate to reject', true).id,
      candidate: add('SYNTHETIC candidate archive is forbidden', true).id,
      user: add('SYNTHETIC user-scope preference fixture', false, 'user').id,
      other: add('SYNTHETIC other workspace secret-isolation marker', false, 'workspace', second.id).id,
      procedure: store.addEntry({ actor: worker, workspaceId: workspace.id, kind: 'procedure_ref', body: 'SYNTHETIC documented procedure reference', confidence: 0.75, tags: ['synthetic', 'procedure'], procedureRef: { type: 'doc', id: 'synthetic/procedure.md', title: 'Synthetic procedure reference' } }).id };
  } finally { db.close(); }
  receipt.fixtureIds = ids;
  host = await createHiveHost({ dataDir });
  const list = (query = '') => request<{ ok: boolean; memories: Memory[] }>('GET', base + query);
  const initial = await list();
  assert.equal(initial.ok, true); assert.deepEqual(initial.memories.map(memory => memory.id), [ids.active]);
  const all = await list('?status=all&scope=all&limit=50');
  assert.equal(all.memories.length, 6); assert.equal(all.memories.some(memory => memory.id === ids.other), false);
  const active = initial.memories[0]!;
  const procedure = all.memories.find(memory => memory.id === ids.procedure)!;
  assert.equal(procedure.confidence, 0.75);
  assert.deepEqual(procedure.procedure_ref, { type: 'doc', id: 'synthetic/procedure.md', title: 'Synthetic procedure reference' });
  assert.equal(active.confidence, 1); assert.equal(active.procedure_ref, null);
  assert.equal(active.body, 'SYNTHETIC alpha workspace fact 中文检索标记');
  assert.equal(active.sources[0]?.actor_name_snapshot, 'Synthetic memory fixture');
  assert.equal(active.sources[0]?.source_type, 'manual'); assert.ok(active.sources[0]?.excerpt.includes('SYNTHETIC'));
  assert.deepEqual(active.tags, ['synthetic', 'verification']); assert.equal(typeof active.created_at, 'number');
  assert.equal(typeof active.updated_at, 'number'); assert.equal(active.workspace_id, workspace.id);
  assert.deepEqual((await list('?query=alpha')).memories.map(memory => memory.id), [ids.active]);
  assert.deepEqual((await list('?query=verification')).memories.map(memory => memory.id), [ids.active], 'Search indexes synthetic tags as well as body');
  assert.deepEqual((await list(`?query=${encodeURIComponent('中文检索标记')}`)).memories.map(memory => memory.id), [ids.active]);
  assert.equal((await list('?status=candidate')).memories.length, 4);
  assert.deepEqual((await list('?scope=user')).memories.map(memory => memory.id), [ids.user]);
  assert.equal((await list('?status=all&scope=all&limit=1')).memories.length, 1);
  assert.deepEqual((await list('?limit=0')).memories.map(memory => memory.id), [ids.active]);
  assert.deepEqual((await list('?query=alpha&limit=0')).memories, []);
  receipt.limitZeroBehaviour = 'No-query list treats 0 as default 50; search treats 0 as empty';
  await fails(list('?status=invalid'), 400); await fails(list('?scope=invalid'), 400); await fails(list('?limit=-1'), 400);

  const flags = await request<{ memory: Memory }>('PATCH', `${base}/${ids.active}`, { pinned: true, disabled: true });
  assert.equal(flags.memory.pinned, true); assert.equal(flags.memory.disabled, true);
  assert.deepEqual((await list('?query=alpha')).memories.map(memory => memory.id), [ids.active], 'UI search includes disabled entries');
  const ignored = await request<{ memory: Memory }>('PATCH', `${base}/${ids.active}`, { body: 'SHOULD NOT REPLACE CONTENT', tags: ['SHOULD-NOT-REPLACE-TAGS'] });
  assert.equal(ignored.memory.body, active.body); assert.deepEqual(ignored.memory.tags, active.tags);
  receipt.contentEditing = { supported: false, observedStatus: 200, unknownBodyAndTagsFieldsIgnored: true };
  await fails(request('PATCH', `${base}/${ids.active}`, { pinned: 'true' }), 400);
  await fails(request('PATCH', `${base}/missing-memory`, { pinned: true }), 404);
  await fails(request('PATCH', `/api/ui/workspaces/${second.id}/memory/${ids.active}`, { disabled: false }), 404);
  const approved = await request<{ memory: Memory }>('POST', `${base}/${ids.approve}/approve`);
  assert.equal(approved.memory.status, 'active');
  await fails(request('POST', `${base}/${ids.approve}/approve`), 409);
  const rejected = await request<{ memory: Memory }>('POST', `${base}/${ids.reject}/reject`);
  assert.equal(rejected.memory.status, 'rejected');
  await fails(request('POST', `${base}/${ids.reject}/reject`), 409);
  await fails(request('POST', `${base}/${ids.candidate}/archive`), 409);
  await fails(request('POST', `${base}/${ids.reject}/archive`), 409);
  const archived = await request<{ memory: Memory }>('POST', `${base}/${ids.approve}/archive`);
  assert.equal(archived.memory.status, 'archived'); assert.equal(typeof archived.memory.archived_at, 'number');
  assert.equal((await request<{ memory: Memory }>('POST', `${base}/${ids.approve}/archive`)).memory.status, 'archived');
  for (const id of [ids.candidate, ids.reject, ids.approve]) {
    await fails(request('PATCH', `${base}/${id}`, { pinned: true }), 409);
    await fails(request('PATCH', `${base}/${id}`, { disabled: true }), 409);
  }
  await fails(request('POST', `${base}/missing-memory/approve`), 404);
  await fails(request('GET', '/api/ui/workspaces/missing-workspace/memory'), 404);

  const off = await request<{ enabled: boolean; dream_enabled: boolean }>('PUT', `${base}/settings`, { enabled: false });
  assert.equal(off.enabled, false); assert.equal(off.dream_enabled, false);
  assert.ok((await list()).memories.some(memory => memory.id === ids.active), 'Disabling injection does not hide management data');
  await fails(request('PUT', `${base}/settings`, { enabled: 'false' }), 400);
  const on = await request<{ enabled: boolean; dream_enabled: boolean }>('PUT', `${base}/settings`, { enabled: true });
  assert.equal(on.enabled, true); assert.equal(on.dream_enabled, false);
  const unauthorized = await fetch(`http://127.0.0.1:${host.port}${base}`);
  assert.equal(unauthorized.status, 403); await unauthorized.body?.cancel();
  receipt.unauthenticatedStatus = unauthorized.status;
  await host.close(); host = undefined;
  host = await createHiveHost({ dataDir });
  const persisted = await list('?status=all&scope=all&limit=50');
  assert.equal(persisted.memories.find(memory => memory.id === ids.active)?.disabled, true);
  assert.equal(persisted.memories.find(memory => memory.id === ids.active)?.pinned, true);
  assert.equal(persisted.memories.find(memory => memory.id === ids.approve)?.status, 'archived');
  assert.equal(persisted.memories.find(memory => memory.id === ids.reject)?.status, 'rejected');
  const settings = await request<{ enabled: boolean; dream_enabled: boolean }>('GET', `${base}/settings`);
  assert.equal(settings.enabled, true); assert.equal(settings.dream_enabled, false);
  const finalDb = new Database(join(dataDir, 'runtime.sqlite'), { readOnly: true });
  try { receipt.dreamRunCount = Number((finalDb.prepare('SELECT COUNT(*) AS count FROM dream_runs').get() as { count: number }).count); }
  finally { finalDb.close(); }
  assert.equal(receipt.dreamRunCount, 0);
  receipt.finalMemory = persisted; receipt.finalSettings = settings; receipt.pass = true;
} catch (error) { receipt.pass = false; receipt.error = String(error); throw error; }
finally {
  try { await host?.close(); }
  catch (error) { receipt.pass = false; receipt.cleanupError = String(error); throw error; }
  finally { await writeFile(join(directory, 'receipt.json'), JSON.stringify(receipt, null, 2) + '\n'); }
}
console.log(JSON.stringify({ pass: true, evidence: join(directory, 'receipt.json'), desktopDataDir, dataDir, project, workspaceId: receipt.workspaceId }));

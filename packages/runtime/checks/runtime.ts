import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdir, mkdtemp, readFile, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createInterface } from 'node:readline';
import { once } from 'node:events';

const directory = await mkdtemp(join(tmpdir(), 'hive-desktop-rpc-check-'));
const entry = fileURLToPath(new URL('../dist/main.js', import.meta.url));
const children: ReturnType<typeof spawn>[] = [];
const launch = (agentDir?: string) => {
  const child = spawn(process.execPath, [entry, '--data-dir', join(directory, 'state')], { stdio: ['pipe', 'pipe', 'pipe'], env: agentDir ? { ...process.env, PI_CODING_AGENT_DIR: agentDir } : process.env });
  children.push(child);
  let next = 0;
  let stderr = '';
  child.stderr?.on('data', data => { stderr += String(data); });
  const pending = new Map<number, { resolve: (value: any) => void; reject: (error: Error) => void }>();
  const reader = createInterface({ input: child.stdout! });
  reader.on('line', line => {
    const value = JSON.parse(line);
    if (typeof value.id === 'number') {
      const waiter = pending.get(value.id);
      pending.delete(value.id);
      if (value.error) waiter?.reject(new Error(JSON.stringify(value.error)));
      else waiter?.resolve(value.result);
    }
  });
  const exited = once(child, 'exit');
  child.once('exit', code => {
    for (const waiter of pending.values()) waiter.reject(new Error(`Runtime exit ${code}: ${stderr}`));
    pending.clear();
  });
  const request = (method: string, params: object = {}) => new Promise<any>((resolve, reject) => {
    const id = ++next;
    const timer = setTimeout(() => { pending.delete(id); reject(new Error(`RPC timeout: ${method}\n${stderr}`)); }, 30_000);
    pending.set(id, { resolve: value => { clearTimeout(timer); resolve(value); }, reject: error => { clearTimeout(timer); reject(error); } });
    child.stdin!.write(`${JSON.stringify({ id, method, params })}\n`);
  });
  return { request, exited };
};

try {
  const first = launch();
  const initialized = await first.request('initialize');
  assert.equal(initialized.version, '0.1.0');
  assert.equal(initialized.navigation, null);
  assert.ok(Number.isInteger(initialized.hivePort) && initialized.hivePort > 0);
  assert.deepEqual(await first.request('hive.request', { method: 'GET', path: '/api/workspaces' }), []);
  const workspace = await first.request('hive.request', { method: 'POST', path: '/api/workspaces', body: {
    path: directory, name: 'Runtime integration', autostart_orchestrator: false,
  } });
  assert.equal(workspace.orchestrator_start.run_id, null);
  const workers = [];
  for (const backend of ['pi', 'codex']) {
    const worker = await first.request('hive.request', { method: 'POST', path: `/api/workspaces/${workspace.id}/workers`,
      body: { name: `member-${backend}`, role: 'tester', autostart: false } });
    await first.request('hive.request', { method: 'POST', path: `/api/workspaces/${workspace.id}/agents/${worker.id}/config`,
      body: { command: `hive:${backend}`, args: ['--provider', 'openai', '--model', 'gpt-6.1-terra'] } });
    workers.push({ id: worker.id, backend });
  }
  const created = await first.request('chat.create', { workspaceId: workspace.id });
  assert.equal(created.workspaceId, workspace.id);
  assert.equal(created.history.filter((message: { role: string }) => message.role !== 'custom').length, 0);
  assert.ok(created.history.some((message: { customType?: string }) => message.customType === 'hive-desktop'));
  assert.ok(Array.isArray(created.models) && created.models.length > 0);
  const listed = await first.request('chat.list', { workspaceId: workspace.id });
  assert.equal(listed.length, 1);
  assert.equal(listed[0].id, created.id);
  const opened = await first.request('chat.open', { sessionId: created.id });
  assert.equal(opened.id, created.id);
  assert.equal(opened.busy, false);
  if (!opened.model) {
    await assert.rejects(first.request('chat.prompt', { sessionId: created.id, text: '离线草稿' }));
    const after = await first.request('chat.open', { sessionId: created.id });
    assert.equal(after.runId, null);
    assert.equal(after.agentId, undefined);
    assert.equal(after.title, created.title);
    assert.deepEqual(after.history, opened.history);
  }
  await assert.rejects(first.request('chat.setModel', { sessionId: created.id, provider: '__missing__', modelId: '__missing__' }));
  const newer = await first.request('chat.create', { workspaceId: workspace.id });
  const selection = (session: string) => ({ workspace: workspace.id, sessions: { [workspace.id]: session } });
  const lastSelection = selection(created.id);
  // Last click and immediate quit use the same ordered input stream.
  await Promise.all([
    first.request('desktop.navigation.save', selection(newer.id)),
    first.request('desktop.navigation.save', lastSelection),
    first.request('shutdown'),
  ]);
  assert.equal((await first.exited)[0], 0);
  await assert.rejects(fetch(`http://127.0.0.1:${initialized.hivePort}/api/ui/session`));

  // Previous Desktop builds persisted Pi core's no-model identity on first launch.
  const indexPath = join(directory, 'state', 'chats', 'index.json');
  const index = JSON.parse(await readFile(indexPath, 'utf8'));
  index.find((item: { id: string }) => item.id === created.id).model = { provider: 'unknown', id: 'unknown', name: 'unknown' };
  index.find((item: { id: string }) => item.id === newer.id).model = { provider: 'removed-provider', id: 'gpt-6.1-terra', name: 'Terra' };
  await writeFile(indexPath, JSON.stringify(index));
  const second = launch();
  const restart = await second.request('initialize');
  assert.deepEqual(JSON.parse(restart.navigation), lastSelection);
  const roster = await second.request('team.members', { workspaceId: workspace.id });
  for (const worker of workers) {
    const row = roster.find((member: { id: string }) => member.id === worker.id);
    assert.equal(row.configured_command, `hive:${worker.backend}`);
    assert.deepEqual(row.desktop_model, { provider: 'openai', id: 'gpt-6.1-terra' });
    assert.equal(row.status, 'stopped');
  }
  const persisted = await second.request('chat.list', { workspaceId: workspace.id });
  assert.equal(persisted.length, 2);
  assert.equal(persisted[0].id, newer.id);
  const unavailable = await second.request('chat.open', { sessionId: newer.id });
  assert.equal(unavailable.model, undefined);
  assert.equal(unavailable.unavailableModel.provider, 'removed-provider');
  assert.ok(unavailable.history.some((message: { customType?: string }) => message.customType === 'hive-desktop'));
  await assert.rejects(second.request('chat.prompt', { sessionId: newer.id, text: '留在原会话' }));
  const unchanged = await second.request('chat.list', { workspaceId: workspace.id });
  assert.equal(unchanged.find((item: { id: string }) => item.id === newer.id).model.provider, 'removed-provider');
  if (unavailable.models.some((m: { provider: string; id: string; available: boolean }) => m.provider === 'openai' && m.id === 'gpt-6.1-terra' && m.available)) {
    await second.request('chat.setModel', { sessionId: newer.id, provider: 'openai', modelId: 'gpt-6.1-terra' });
    const repaired = await second.request('chat.open', { sessionId: newer.id });
    assert.equal(repaired.model.id, 'gpt-6.1-terra');
    assert.equal(repaired.unavailableModel, undefined);
    assert.deepEqual(repaired.history, unavailable.history);
  }
  const restored = await second.request('chat.open', { sessionId: JSON.parse(restart.navigation).sessions[workspace.id] });
  assert.equal(restored.id, created.id);
  assert.notDeepEqual([restored.model?.provider, restored.model?.id], ['unknown', 'unknown']);
  assert.equal(restored.history.filter((message: { role: string }) => message.role !== 'custom').length, 0);
  assert.ok(restored.history.some((message: { customType?: string }) => message.customType === 'hive-desktop'));
  assert.deepEqual(await second.request('shutdown'), { ok: true });
  assert.equal((await second.exited)[0], 0);
  const agentDir = join(directory, 'pi-refresh');
  await mkdir(agentDir);
  const third = launch(agentDir);
  assert.equal((await third.request('initialize')).piAgentDir, agentDir);
  const draft = await third.request('chat.create', { workspaceId: workspace.id });
  const provider = 'local-refresh-fixture';
  const modelId = 'gpt-6.1-terra';
  assert.ok(!draft.models.some((m: {provider: string}) => m.provider === provider));
  await writeFile(join(agentDir, 'models.json'), JSON.stringify({ providers: {
    [provider]: { baseUrl: 'http://127.0.0.1:1/v1', api: 'openai-completions', apiKey: 'local-fixture', models: [{ id: modelId }] }
  } }));
  const refreshed = await third.request('chat.refreshPiModels', { sessionId: draft.id });
  assert.ok(refreshed.some((m: {provider: string; id: string; available: boolean}) => m.provider === provider && m.id === modelId && m.available));
  await third.request('chat.setModel', { sessionId: draft.id, provider, modelId });
  const selected = await third.request('chat.open', { sessionId: draft.id });
  assert.equal(selected.model.provider, provider);
  assert.deepEqual(selected.history, draft.history);
  await third.request('shutdown');
  assert.equal((await third.exited)[0], 0);
  console.log('Runtime integration passed: real Hive workspace + Pi SDK + restart + shutdown. No model request sent.');
} finally {
  for (const child of children) if (child.exitCode === null) child.kill();
  await rm(directory, { recursive: true, force: true });
}

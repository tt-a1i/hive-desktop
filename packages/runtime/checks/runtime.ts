import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createInterface } from 'node:readline';
import { once } from 'node:events';

const directory = await mkdtemp(join(tmpdir(), 'hive-desktop-rpc-check-'));
const entry = fileURLToPath(new URL('../dist/main.js', import.meta.url));
const children: ReturnType<typeof spawn>[] = [];
const launch = () => {
  const child = spawn(process.execPath, [entry, '--data-dir', join(directory, 'state')], { stdio: ['pipe', 'pipe', 'pipe'] });
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

  const second = launch();
  const restart = await second.request('initialize');
  assert.deepEqual(JSON.parse(restart.navigation), lastSelection);
  const persisted = await second.request('chat.list', { workspaceId: workspace.id });
  assert.equal(persisted.length, 2);
  assert.equal(persisted[0].id, newer.id);
  const restored = await second.request('chat.open', { sessionId: JSON.parse(restart.navigation).sessions[workspace.id] });
  assert.equal(restored.id, created.id);
  assert.equal(restored.history.filter((message: { role: string }) => message.role !== 'custom').length, 0);
  assert.ok(restored.history.some((message: { customType?: string }) => message.customType === 'hive-desktop'));
  assert.deepEqual(await second.request('shutdown'), { ok: true });
  assert.equal((await second.exited)[0], 0);
  console.log('Runtime integration passed: real Hive workspace + Pi SDK + restart + shutdown. No model request sent.');
} finally {
  for (const child of children) if (child.exitCode === null) child.kill();
  await rm(directory, { recursive: true, force: true });
}

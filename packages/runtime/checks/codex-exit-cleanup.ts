/** Real Hive/SQLite/spool and production Codex transport; owned local JSONL peer, no inference. */
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, readFile, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { setTimeout as delay } from 'node:timers/promises';
import { createDesktopRuntime } from '../src/desktop-runtime.js';

const directory = await mkdtemp(join(tmpdir(), 'hive-codex-exit-'));
const project = join(directory, 'project');
await mkdir(project);
await writeFile(join(project, 'package.json'), '{"type":"commonjs"}');
await writeFile(join(project, 'app-server'),
  `require('node:fs').writeFileSync('owned-peer-pid.json',JSON.stringify({pid:process.pid}));\n` +
  await readFile(new URL('./fixtures/codex-queue-peer.cjs', import.meta.url), 'utf8'));
const events: any[] = [];
let runtime = await createDesktopRuntime(join(directory, 'state'), event => events.push(event),
  { executableEnvironment: { HIVE_CODEX_COMMAND: process.execPath } });
const wait = async (condition: () => Promise<boolean>) => {
  const deadline = Date.now() + 10000;
  while (!await condition()) { assert.ok(Date.now() < deadline, 'Owned exit check timed out'); await delay(10); }
};
try {
  const workspace = await runtime.hive.request('POST', '/api/workspaces', {
    path: project, name: 'Owned exit cleanup check', autostart_orchestrator: false,
  }) as { id: string };
  const chat = await runtime.chats.create(workspace.id, 'codex');
  await runtime.chats.prompt(chat.id, 'Owned active input');
  await wait(async () => (await runtime.chats.open(chat.id)).busy);
  await runtime.chats.prompt(chat.id, 'Owned queued input');
  const accepted = (await runtime.chats.open(chat.id)).pendingUserInputs;
  assert.equal(accepted.length, 2);
  const live = (runtime.chats as any).live.get(chat.id);
  const { pid } = JSON.parse(await readFile(join(project, 'owned-peer-pid.json'), 'utf8'));
  assert.ok(Number.isInteger(pid) && pid > 0);
  process.kill(pid, 'SIGTERM');
  await wait(async () => Boolean(live.cleanup));
  // Cleanup must reap the dead session, not reject with the already-reported run error.
  await live.cleanup;
  assert.equal((runtime.chats as any).live.has(chat.id), false);
  const failures = events.filter(event => event.type === 'error');
  assert.ok(failures.length > 0);
  assert.match(failures.at(-1).message, /Codex.*(SIGTERM|EOF)/);
  assert.ok(failures.every(event => event.message !== 'Backend exit cleanup failed'));
  await runtime.close();
  runtime = await createDesktopRuntime(join(directory, 'state'), event => events.push(event),
    { executableEnvironment: { HIVE_CODEX_COMMAND: process.execPath } });
  const cold = await runtime.chats.open(chat.id);
  assert.equal(cold.busy, false);
  assert.deepEqual(cold.pendingUserInputs.map(input => [input.deliveryId, input.text]),
    accepted.map(input => [input.deliveryId, input.text]));
  assert.equal(JSON.parse(await readFile(join(project, 'queue-peer-thread.json'), 'utf8')).turns.length, 1);
  await writeFile(join(project, 'queue-peer-control.json'), '{"complete":true}');
  await runtime.chats.resume(chat.id);
  await wait(async () => { const s = await runtime.chats.open(chat.id); return !s.busy && s.pending === 0; });
  const completed = await runtime.chats.open(chat.id);
  for (const input of accepted) assert.ok(completed.completedDeliveryIds.includes(input.deliveryId));
  const turns = JSON.parse(await readFile(join(project, 'queue-peer-thread.json'), 'utf8')).turns;
  assert.equal(turns.length, 3);
  assert.ok(turns.slice(1).every((turn: any) => turn.status === 'completed'));
  const output = resolve('../../.artifacts/codex-exit-cleanup');
  await mkdir(output, { recursive: true });
  await writeFile(join(output, 'receipt.json'), JSON.stringify({ pass: true, directory,
    realModelRequests: 0, retainedDeliveryIds: accepted.map(input => input.deliveryId),
    cleanupResolved: true, nativeTurns: 3, originalFailure: failures.at(-1).message }, null, 2));
  console.log('CODEX_EXIT_CLEANUP_PASS: root diagnostic, reaping, exact cold queue and explicit recovery; local peer only.');
} finally { await runtime.close(); }

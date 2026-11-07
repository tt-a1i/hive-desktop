/** Actual Hive HTTP/SQLite persistence and installed external Pi startup; no model turns. */
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, writeFile } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { createDesktopRuntime } from '../src/desktop-runtime.js';
import { DesktopSettings, DesktopSettingsValidationError } from '../src/desktop-settings.js';
import type { Chats } from '../src/chats.js';
import type { PiSessionHandle } from '../src/agents/types.js';

await mkdir(resolve('../../.artifacts'), { recursive: true });
const directory = await mkdtemp(resolve('../../.artifacts/desktop-settings-'));
const dataDir = join(directory, 'state');
const rowPath = '/api/settings/app-state/desktop_executables';
const nativePi = '/Users/tsk/Library/pnpm/pi';
type Runtime = Awaited<ReturnType<typeof createDesktopRuntime>>;
let runtime: Runtime | undefined;
const options = { executableEnvironment: {} };
const live = (chats: Chats) => (chats as unknown as { live: Map<string, { handle: PiSessionHandle }> }).live;
try {
  runtime = await createDesktopRuntime(dataDir, () => {}, options);
  assert.deepEqual(await runtime.settings.get(), { piCommand: null, codexCommand: null, claudeCommand: null,
    effective: { pi: { command: 'pi', source: 'path' }, codex: { command: 'codex', source: 'path' }, claude: { command: 'claude', source: 'path' } } });
  const saved = await runtime.settings.update({ piCommand: ' /Applications/External Agent/Pi CLI ', codexCommand: ' /Applications/Codex Agent/codex ' });
  assert.equal(saved.piCommand, '/Applications/External Agent/Pi CLI');
  assert.equal(saved.effective.pi.source, 'app');
  const row = await runtime.hive.request('GET', rowPath) as { key: string; value: string };
  assert.equal(row.key, 'desktop_executables');
  assert.deepEqual(JSON.parse(row.value), { piCommand: saved.piCommand, codexCommand: saved.codexCommand, claudeCommand: null });
  for (const value of ['', '   ', '\0', 'bad\npath', 'bad\tpath', 'bad\u007fpath', 'bad\u009fpath', 'x'.repeat(4097), 42, undefined]) {
    await assert.rejects(runtime.settings.update({ piCommand: value, codexCommand: null }), DesktopSettingsValidationError);
    assert.deepEqual(await runtime.settings.get(), saved, 'Rejected input must leave the prior SQLite settings unchanged');
  }
  for (const value of [null, [], {}, { piCommand: null }, { codexCommand: null }]) {
    await assert.rejects(runtime.settings.update(value), DesktopSettingsValidationError);
  }
  const environment = new DesktopSettings(runtime.hive, { HIVE_PI_COMMAND: '/env/Agent With Spaces/pi', HIVE_CODEX_COMMAND: '/env/codex' });
  assert.deepEqual((await environment.get()).effective, {
    pi: { command: '/env/Agent With Spaces/pi', source: 'env' }, codex: { command: '/env/codex', source: 'env' }, claude: { command: 'claude', source: 'path' },
  });
  await runtime.close(); runtime = undefined;
  runtime = await createDesktopRuntime(dataDir, () => {}, options);
  assert.deepEqual(await runtime.settings.get(), saved, 'Settings must survive complete Hive runtime restart');
  await runtime.settings.update({ piCommand: null, codexCommand: null });
  assert.equal((await runtime.settings.get()).effective.pi.source, 'path');
  assert.equal((await new DesktopSettings(runtime.hive, { HIVE_PI_COMMAND: '/env/pi' }).get()).effective.pi.source, 'env');

  await runtime.settings.update({ piCommand: nativePi, codexCommand: null });
  const workspace = await runtime.hive.request('POST', '/api/workspaces', {
    path: directory, name: 'Executable settings lifecycle', autostart_orchestrator: false,
  }) as { id: string };
  const existing = await runtime.chats.create(workspace.id, 'pi-rpc');
  await runtime.chats.setModel(existing.id, 'openai', 'gpt-6.1-terra');
  const handle = live(runtime.chats).get(existing.id)!.handle;
  const missing = join(directory, 'Missing Agent With Spaces', 'pi');
  await runtime.settings.update({ piCommand: missing, codexCommand: null });
  assert.equal((await runtime.chats.open(existing.id)).model?.id, 'gpt-6.1-terra');
  assert.equal(live(runtime.chats).get(existing.id)!.handle, handle, 'Saved settings do not replace an already-running process');
  const failed = await runtime.chats.createRecord(workspace.id, 'pi-rpc');
  await assert.rejects(runtime.chats.open(failed.id), (error: NodeJS.ErrnoException) => error.code === 'ENOENT');
  assert.deepEqual(runtime.chats.list(workspace.id).map(record => record.id).sort(), [existing.id, failed.id].sort());
  await runtime.settings.update({ piCommand: nativePi, codexCommand: null });
  assert.equal((await runtime.chats.open(failed.id)).backend, 'pi-rpc', 'Next handle load must read the repaired app command');
  // The old orchestrator can be uninstalled while Desktop is closed.
  await runtime.chats.resume(existing.id);
  await runtime.close();
  runtime = await createDesktopRuntime(dataDir, () => {}, options);
  await runtime.settings.update({ piCommand: missing, codexCommand: null });
  const replacement = await runtime.chats.create(workspace.id, 'pi');
  await runtime.chats.resume(replacement.id);
  const rebound = runtime.chats.list(workspace.id);
  assert.equal(rebound.find(chat => chat.id === replacement.id)?.agentId, workspace.id + ':orchestrator');
  assert.equal(rebound.find(chat => chat.id === existing.id)?.externalPiSessionId, existing.externalPiSessionId);
  assert.equal(rebound.find(chat => chat.id === existing.id)?.agentId, undefined);
  assert.equal(live(runtime.chats).has(existing.id), false);
  const beforeFailure = await runtime.settings.get();
  await runtime.hive.close();
  await assert.rejects(runtime.settings.update({ piCommand: '/should-not-save/pi', codexCommand: null }));
  await runtime.close(); runtime = undefined;
  runtime = await createDesktopRuntime(dataDir, () => {}, options);
  assert.deepEqual(await runtime.settings.get(), beforeFailure, 'Failed persistence must preserve prior saved settings');
  await writeFile(join(directory, 'receipt.json'), JSON.stringify({ nativePi, saved, beforeFailure,
    persisted: await runtime.settings.get(), noModelTurns: true, existingHandlePreserved: true,
    nextHandleUsesUpdatedCommand: true, invalidInputsRejected: true, environmentPrecedenceVerified: true }, null, 2));
  console.log(`DESKTOP_SETTINGS_OK ${directory}`);
} catch (error) {
  await writeFile(join(directory, 'failure.json'), JSON.stringify({ message: error instanceof Error ? error.message : String(error), stack: error instanceof Error ? error.stack : undefined }, null, 2));
  throw error;
} finally {
  await runtime?.close();
  console.log(`Evidence retained: ${directory}`);
}

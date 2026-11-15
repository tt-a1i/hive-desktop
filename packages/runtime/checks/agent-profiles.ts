/** Temporary real Hive HTTP/SQLite only. No agent/CLI/model/account operation. */
import assert from 'node:assert/strict';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createHiveHost } from '../src/hive-host.js';
import { DesktopSettings } from '../src/desktop-settings.js';
import {
  AgentProfiles, AgentProfileError, AGENT_PROFILES_STATE_PATH, defaultAgentProfileId,
  validateAgentProfileLaunch, type CreateAgentProfile, type UpdateAgentProfile,
} from '../src/agent-profiles.js';
import { agentCapabilities } from '../src/agent-capabilities.js';

const directory = await mkdtemp(join(tmpdir(), 'hive-agent-profiles-'));
let host: Awaited<ReturnType<typeof createHiveHost>> | undefined;
const errorCode = (code: string) => (error: unknown) => error instanceof AgentProfileError && error.code === code;
// Native config-home references must remain absolute when passed straight to the CLI.
// On macOS/Linux, accepting a Windows drive/UNC spelling creates a relative directory.
const configPaths = [
  { value: '/native/config with spaces', accepted: true },
  { value: 'relative/config', accepted: false },
  { value: 'C:relative-config', accepted: false },
  { value: 'C:\\Agent Config\\Codex', accepted: process.platform === 'win32' },
  { value: '\\\\server\\share\\Agent Config', accepted: process.platform === 'win32' },
];
for (const { value, accepted } of configPaths) {
  const input = { command: 'codex', args: [], configDir: value };
  if (accepted) assert.equal(validateAgentProfileLaunch('codex', input).configDir, value);
  else assert.throws(() => validateAgentProfileLaunch('codex', input), errorCode('PROFILE_INVALID'));
}
let failBefore = false, failAfter = false, writes = 0;
let writeGate: Promise<void> | undefined, onWrite: (() => void) | undefined;
const transport = { async request(method: string, path: string, body?: unknown) {
  if (method === 'PUT' && path === AGENT_PROFILES_STATE_PATH) {
    writes++; onWrite?.(); await writeGate;
    if (failBefore) { failBefore = false; throw new Error('controlled pre-commit failure'); }
    const response = await host!.request(method, path, body);
    if (failAfter) { failAfter = false; throw new Error('controlled lost response after commit'); }
    return response;
  }
  return host!.request(method, path, body);
} };
try {
  host = await createHiveHost({ dataDir: join(directory, 'state') });
  const settings = new DesktopSettings(host, { HIVE_CODEX_COMMAND: '/explicit env/Codex/codex' });
  await settings.update({ piCommand: '/saved Pi/pi', codexCommand: '/unused app/codex', claudeCommand: '/saved Claude/claude' });
  const migration = await settings.get();
  const store = new AgentProfiles(transport);
  await assert.rejects(store.list(), errorCode('PROFILE_STORE_UNINITIALIZED'));
  assert.throws(() => store.currentRefSnapshot('default-codex'), errorCode('PROFILE_STORE_UNINITIALIZED'));
  const invalidMigration = structuredClone(migration);
  invalidMigration.effective.claude.command = '';
  await assert.rejects(store.initialize(invalidMigration), errorCode('PROFILE_INVALID'));
  assert.equal((await host.request('GET', AGENT_PROFILES_STATE_PATH) as { value: unknown }).value, null);
  assert.equal(writes, 0, 'An invalid default must not write the other three defaults');
  failBefore = true;
  await assert.rejects(store.initialize(migration), /controlled pre-commit/);
  assert.equal((await host.request('GET', AGENT_PROFILES_STATE_PATH) as { value: unknown }).value, null);
  const initial = await store.initialize(migration);
  assert.equal(initial.storeVersion, 1);
  assert.equal(initial.profiles.length, 4);
  assert.deepEqual((await store.getRef({ id: 'default-codex', revision: 1 })).launch,
    { command: '/explicit env/Codex/codex', args: [] });
  assert.deepEqual((await store.getRef({ id: 'default-pi', revision: 1 })).launch, { args: [] });
  assert.equal((await store.getRef({ id: 'default-pi-rpc', revision: 1 })).launch.command, '/saved Pi/pi');
  assert.equal((await store.getRef({ id: 'default-claude', revision: 1 })).launch.command, '/saved Claude/claude');
  const newDefaults = await store.update('default-codex', { launch: { command: '/new default/codex', args: [] } }, 1);
  assert.equal(newDefaults.profile.ref.revision, 2);
  assert.deepEqual(await store.legacyRef('codex'), { id: 'default-codex', revision: 1 });
  assert.equal((await store.resolveForExecution(await store.legacyRef('codex'))).launch.command, '/explicit env/Codex/codex');
  assert.deepEqual(store.currentRefSnapshot('default-codex'), { id: 'default-codex', revision: 2 });

  let version = newDefaults.storeVersion;
  const created = await store.create({ driver: 'codex', label: 'Work account',
    launch: { command: '/Applications/CLI With Spaces/codex', args: ['-c', 'model_provider="work"'], configDir: '/nonsecret/config-reference' } }, version);
  version = created.storeVersion;
  const pinned = structuredClone(created.profile.ref);
  const edited = await store.update(pinned.id, { label: 'Renamed work', launch: { command: '/different/codex', args: [], configDir: join(directory, 'config reference') } }, version);
  version = edited.storeVersion;
  assert.equal(edited.profile.ref.revision, 2);
  assert.equal((await store.getRef(pinned)).label, 'Renamed work');
  assert.equal((await store.resolveForExecution(pinned)).launch.command, '/Applications/CLI With Spaces/codex');
  created.profile.launch.args.push('caller mutation');
  assert.deepEqual((await store.getRef(pinned)).launch.args, ['-c', 'model_provider="work"']);
  await assert.rejects(store.getRef(pinned, 'pi-rpc'), errorCode('PROFILE_DRIVER_MISMATCH'));
  await assert.rejects(store.getRef({ id: pinned.id, revision: 99 }), errorCode('PROFILE_REVISION_NOT_FOUND'));
  await assert.rejects(store.getRef({ id: 'missing', revision: 1 }), errorCode('PROFILE_NOT_FOUND'));
  assert.equal((await store.list()).profiles.length, 5, 'A bad session ref does not poison other profile/history reads');
  const disabled = await store.enableDisable(pinned.id, false, version); version = disabled.storeVersion;
  assert.equal((await store.getRef(pinned)).enabled, false);
  await assert.rejects(store.resolveForExecution(pinned), errorCode('PROFILE_DISABLED'));
  assert.throws(() => store.resolveForExecutionSnapshot(pinned), errorCode('PROFILE_DISABLED'));
  await assert.rejects(store.update(pinned.id, { launch: { command: '/blocked revision', args: [] } }, version), errorCode('PROFILE_DISABLED'));
  const renamed = await store.update(pinned.id, { label: 'Disabled, readable history' }, version); version = renamed.storeVersion;
  assert.equal(renamed.profile.ref.revision, 2, 'Renaming does not create a connection revision');
  const enabled = await store.enableDisable(pinned.id, true, version); version = enabled.storeVersion;
  assert.equal((await store.resolveForExecution(pinned)).launch.command, '/Applications/CLI With Spaces/codex');

  const beforeFailures = await store.list();
  for (const { value, accepted } of configPaths) {
    if (!accepted) await assert.rejects(store.update(pinned.id, { launch: { command: '/codex', args: [], configDir: value } }, version), errorCode('PROFILE_INVALID'));
  }
  assert.deepEqual(await store.list(), beforeFailures, 'Host-relative config references must not create a new SQLite revision');
  for (const launch of [{ command: '/pi', args: [] }, { args: ['-c'] }, { args: [], environment: { HIVE_AGENT_TOKEN: 'never-store' } }]) {
    await assert.rejects(store.create({ driver: 'pi', label: 'invalid', launch } as CreateAgentProfile, version), errorCode('PROFILE_INVALID'));
  }
  for (const launch of [{ command: '/codex', args: ['--api-key', 'never-store'] }, { command: '/codex', args: [], configDir: 'relative' },
    { command: '/codex', args: [], env: { HIVE_AGENT_ID: 'replacement' } }, { command: '/codex', args: ['HIVE_AGENT_TOKEN=never-store'] }]) {
    await assert.rejects(store.create({ driver: 'codex', label: 'invalid', launch } as CreateAgentProfile, version), errorCode('PROFILE_INVALID'));
  }
  await assert.rejects(store.update(pinned.id, { driver: 'claude' } as UpdateAgentProfile, version), errorCode('PROFILE_INVALID'));
  await assert.rejects(store.create({ driver: 'claude', label: 'unsupported ordered args', launch: { command: '/claude', args: ['--custom'] } }, version), errorCode('PROFILE_INVALID'));
  assert.deepEqual(await store.list(), beforeFailures);

  failBefore = true;
  await assert.rejects(store.create({ driver: 'codex', label: 'must not appear', launch: { command: '/failed/codex', args: [] } }, version), /controlled pre-commit/);
  assert.throws(() => store.currentRefSnapshot('default-codex'), errorCode('PROFILE_STORE_UNINITIALIZED'));
  assert.deepEqual(await store.list(), beforeFailures, 'Failed write leaves no partial profile and no advanced version');

  let release!: () => void, entered!: () => void;
  writeGate = new Promise(resolve => { release = resolve; });
  const enteredWrite = new Promise<void>(resolve => { entered = resolve; });
  onWrite = entered;
  const first = store.create({ driver: 'pi-rpc', label: 'Concurrent winner', launch: { command: '/rpc pi', args: [] } }, version);
  await enteredWrite;
  const second = store.create({ driver: 'codex', label: 'Stale loser', launch: { command: '/codex', args: [] } }, version);
  const queuedRead = store.list();
  const mutableInput: UpdateAgentProfile = { label: 'Captured input' };
  const capturedUpdate = store.update(pinned.id, mutableInput, version + 1);
  mutableInput.label = 'Changed by caller after enqueue';
  release(); writeGate = undefined; onWrite = undefined;
  const [firstResult, secondResult] = await Promise.allSettled([first, second]);
  assert.equal(firstResult.status, 'fulfilled');
  assert.equal(secondResult.status, 'rejected');
  if (secondResult.status === 'rejected') assert.ok(errorCode('PROFILE_STORE_CONFLICT')(secondResult.reason));
  const afterConcurrent = await queuedRead;
  assert.equal(afterConcurrent.storeVersion, version + 1);
  assert.equal(afterConcurrent.profiles.filter(item => item.label === 'Concurrent winner').length, 1);
  assert.equal(afterConcurrent.profiles.some(item => item.label === 'Stale loser'), false);
  const captured = await capturedUpdate; version = captured.storeVersion;
  assert.equal(captured.profile.label, 'Captured input');

  // A commit may succeed while the caller loses its response. Do not keep an assumed cache.
  failAfter = true;
  await assert.rejects(store.update(pinned.id, { launch: { command: '/committed-but-response-lost/codex', args: [] } }, version), /lost response/);
  assert.throws(() => store.currentRefSnapshot(pinned.id), errorCode('PROFILE_STORE_UNINITIALIZED'));
  const afterLostResponse = await store.list();
  assert.equal(afterLostResponse.storeVersion, version + 1);
  await assert.rejects(store.update(pinned.id, { label: 'stale retry' }, version), errorCode('PROFILE_STORE_CONFLICT'));
  assert.equal((await store.resolveForExecution(pinned)).launch.command, '/Applications/CLI With Spaces/codex');
  const savedRow = await host.request('GET', AGENT_PROFILES_STATE_PATH) as { value: string };
  for (const corrupt of ['{bad JSON', JSON.stringify({ schemaVersion: 2, storeVersion: 1, profiles: [] }),
    JSON.stringify({ ...afterLostResponse, schemaVersion: 1, profiles: afterLostResponse.profiles.map(item => item.id === 'default-codex' ? { ...item, driver: 'pi-rpc' } : item) })]) {
    await host.request('PUT', AGENT_PROFILES_STATE_PATH, { value: corrupt });
    await assert.rejects(store.initialize(migration), errorCode('PROFILE_STORE_CORRUPT'));
    assert.equal((await host.request('GET', AGENT_PROFILES_STATE_PATH) as { value: string }).value, corrupt, 'Corrupt settings are never silently replaced');
  }
  await host.request('PUT', AGENT_PROFILES_STATE_PATH, { value: savedRow.value });
  await host.close(); host = undefined;
  host = await createHiveHost({ dataDir: join(directory, 'state') });
  const cold = new AgentProfiles(host);
  const changedMigration = structuredClone(migration); changedMigration.effective.codex.command = '/must-not-remigrate';
  assert.deepEqual(await cold.initialize(changedMigration), afterLostResponse);
  assert.equal((await cold.resolveForExecution(pinned)).launch.command, '/Applications/CLI With Spaces/codex');
  assert.equal((await new DesktopSettings(host, {}).get()).codexCommand, '/unused app/codex', 'Migration does not change legacy settings/global defaults');
  const capabilities = agentCapabilities('codex', { text: { state: 'supported', source: 'driver' } });
  assert.equal(capabilities.features.text.source, 'driver');
  assert.deepEqual(capabilities.features.nativeResume, { state: 'unknown', source: 'unknown', reason: 'No capability evidence supplied' });
  assert.equal(defaultAgentProfileId('pi-rpc'), 'default-pi-rpc');
  console.log('AGENT_PROFILES_OK: actual Hive SQLite migration/restart, immutable refs, disabled history, serialized stale writes, pre/post-commit failures, corrupt preservation; no native agent execution.');
} finally {
  await host?.close();
  await rm(directory, { recursive: true, force: true });
}

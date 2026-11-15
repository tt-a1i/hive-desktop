import assert from 'node:assert/strict';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createHiveHost } from '../src/hive-host.js';
import { DesktopSettings } from '../src/desktop-settings.js';
import { AgentProfiles, AgentProfileError } from '../src/agent-profiles.js';
import { parseStructuredLaunch, resolveStructuredLaunch, withStructuredProfile, StructuredLaunchError } from '../src/structured-launch.js';
import type { AgentLaunchConfigInput } from '../src/hive-runtime-contract.js';

// Hive's actual launch envelope includes generic resume/cwd/preset fields. Parsing must not
// narrow it to a backend/model tuple and silently discard those fields or passthrough argv.
const original: AgentLaunchConfigInput = {
  command: 'hive:codex', args: ['--provider', 'openai', '--model', 'gpt-6.1-terra', '--other-native-option', 'value with spaces'],
  commandPresetId: 'saved-preset', cwd: '/project worktree', interactiveCommand: null, presetAugmentationDisabled: true,
  resumedSessionId: 'historical-native-id', resumeArgsTemplate: '--resume {sessionId}',
  sessionIdCapture: { source: 'stdout_regex', pattern: 'thread: (.*)' },
};
const parsed = parseStructuredLaunch(original);
assert.deepEqual(parsed.config, original);
assert.deepEqual(parsed.extraArgs, ['--other-native-option', 'value with spaces']);
assert.deepEqual(parsed.model, { provider: 'openai', id: 'gpt-6.1-terra' });
assert.deepEqual(parsed.profile, { kind: 'legacy-default', id: 'default-codex' });
parsed.config.args!.push('mutating returned config');
assert.equal(original.args!.includes('mutating returned config'), false);
const explicit = withStructuredProfile(original, { id: 'work', revision: 2 });
assert.deepEqual({ ...explicit, args: original.args }, original);
assert.deepEqual(parseStructuredLaunch(explicit).profile, { kind: 'explicit', ref: { id: 'work', revision: 2 } });
const replaced = withStructuredProfile(explicit, { id: 'personal', revision: 3 });
assert.equal(replaced.args!.filter(arg => arg === '--profile-id').length, 1);
assert.deepEqual(parseStructuredLaunch(replaced).extraArgs, parsed.extraArgs);
assert.deepEqual(original.args, ['--provider', 'openai', '--model', 'gpt-6.1-terra', '--other-native-option', 'value with spaces']);
const separator = withStructuredProfile({ command: 'hive:pi-rpc', args: ['--generic', 'x', '--', '--model', 'literal argument'] }, { id: 'rpc-work', revision: 1 });
assert.deepEqual(separator.args, ['--generic', 'x', '--profile-id', 'rpc-work', '--profile-revision', '1', '--', '--model', 'literal argument']);
assert.deepEqual(parseStructuredLaunch(separator).extraArgs, ['--generic', 'x', '--', '--model', 'literal argument']);
assert.equal(parseStructuredLaunch(separator).model, undefined);
for (const args of [
  ['--provider'], ['--model', 'only-model'], ['--provider', 'only-provider'],
  ['--provider', '--model', 'm'], ['--provider', 'p', '--model', '-bad'],
  ['--provider', 'p', '--model', 'm', '--provider', 'duplicate'],
  ['--profile-id', 'work'], ['--profile-revision', '1'], ['--profile-id', '--profile-revision', '1'],
  ['--profile-id', 'work', '--profile-revision', '1', '--profile-revision', '2'],
  ['--profile-id', 'work', '--profile-revision', '0'], ['--profile-id', 'work', '--profile-revision', '01'],
  ['--profile-id', 'work', '--profile-revision', '1.1'], ['--profile-id', 'work', '--profile-revision', '9007199254740992'],
  ['--profile-id', '../work', '--profile-revision', '1'], ['--profile-id=work'], ['--profile-revison', '1'],
  ['--provider=p', '--model=m'], ['--model=m'], ['--model', 'bad\nvalue', '--provider', 'p'],
]) assert.throws(() => parseStructuredLaunch({ command: 'hive:codex', args }), StructuredLaunchError);
for (const command of ['codex', 'hive:unknown', 'hive:codex ', 'hive:acp', 'hive:pi --model x'])
  assert.throws(() => parseStructuredLaunch({ command }), StructuredLaunchError);
for (const command of ['hive:pi', 'hive:codex', 'hive:pi-rpc', 'hive:claude'])
  assert.equal(parseStructuredLaunch({ command }).driver, command.slice(5));

const directory = await mkdtemp(join(tmpdir(), 'hive-structured-launch-'));
let host: Awaited<ReturnType<typeof createHiveHost>> | undefined;
try {
  host = await createHiveHost({ dataDir: join(directory, 'state') });
  const profiles = new AgentProfiles(host);
  const initialized = await profiles.initialize(await new DesktopSettings(host, {}).get());
  const changedDefault = await profiles.update('default-codex', { launch: { command: '/new-default/codex', args: [] } }, initialized.storeVersion);
  const legacy = await resolveStructuredLaunch(parseStructuredLaunch(original), profiles);
  assert.equal(legacy.resolvedProfile.ref.revision, 1);
  assert.equal(legacy.resolvedProfile.launch.command, 'codex', 'Legacy launch must not migrate into the newly edited default');
  const custom = await profiles.create({ driver: 'codex', label: 'Work', launch: { command: '/work/codex', args: [] } }, changedDefault.storeVersion);
  const pinned = withStructuredProfile(original, custom.profile.ref);
  assert.equal((await resolveStructuredLaunch(parseStructuredLaunch(pinned), profiles)).resolvedProfile.launch.command, '/work/codex');
  const mismatch = withStructuredProfile({ command: 'hive:pi-rpc' }, custom.profile.ref);
  await assert.rejects(resolveStructuredLaunch(parseStructuredLaunch(mismatch), profiles),
    (error: unknown) => error instanceof AgentProfileError && error.code === 'PROFILE_DRIVER_MISMATCH');
  const tamperedProjection = { ...parseStructuredLaunch(mismatch), driver: 'codex' as const };
  await assert.rejects(resolveStructuredLaunch(tamperedProjection, profiles),
    (error: unknown) => error instanceof AgentProfileError && error.code === 'PROFILE_DRIVER_MISMATCH');
  await profiles.enableDisable(custom.profile.ref.id, false, custom.storeVersion);
  await assert.rejects(resolveStructuredLaunch(parseStructuredLaunch(pinned), profiles),
    (error: unknown) => error instanceof AgentProfileError && error.code === 'PROFILE_DISABLED');
  const offline = await resolveStructuredLaunch(parseStructuredLaunch(pinned), profiles, { forExecution: false });
  assert.equal(offline.resolvedProfile.enabled, false);
  assert.deepEqual(offline.config, pinned);
  console.log('STRUCTURED_LAUNCH_OK: exact Hive envelope/argv round-trip, strict owned flags, explicit/legacy pinned refs, backend mismatch and disabled execution checks; no native agent execution.');
} finally {
  await host?.close();
  await rm(directory, { recursive: true, force: true });
}

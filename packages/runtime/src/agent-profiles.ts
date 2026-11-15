import { randomUUID } from 'node:crypto';
import { isAbsolute } from 'node:path';
import type { DesktopSettingsSnapshot } from './desktop-settings.js';
import type { HiveHost } from './hive-host.js';

export const AGENT_DRIVERS = ['pi', 'codex', 'pi-rpc', 'claude'] as const;
export type AgentDriver = typeof AGENT_DRIVERS[number];
export interface AgentProfileRef { id: string; revision: number }
/** Nonsecret invocation/configuration references only. This is not a credential store. */
export interface AgentProfileLaunch { command?: string; args: string[]; configDir?: string }
export interface AgentProfileRevision {
  revision: number;
  launch: AgentProfileLaunch;
  /** Captured at migration, not proof of the executable/account used historically. */
  legacyConfiguration?: true;
}
export interface AgentProfile {
  id: string; driver: AgentDriver; label: string; enabled: boolean;
  currentRevision: number; revisions: AgentProfileRevision[];
}
export interface ResolvedAgentProfile {
  ref: AgentProfileRef; driver: AgentDriver; label: string; enabled: boolean;
  launch: AgentProfileLaunch; legacyConfiguration?: true;
}
export interface AgentProfilesSnapshot { storeVersion: number; profiles: AgentProfile[] }
export interface AgentProfileMutation { storeVersion: number; profile: ResolvedAgentProfile }
export interface CreateAgentProfile { driver: AgentDriver; label: string; launch: AgentProfileLaunch }
export interface UpdateAgentProfile { label?: string; launch?: AgentProfileLaunch }
interface StoredProfiles extends AgentProfilesSnapshot { schemaVersion: 1 }

export class AgentProfileError extends Error {
  constructor(readonly code: 'PROFILE_INVALID' | 'PROFILE_STORE_CORRUPT' | 'PROFILE_STORE_UNINITIALIZED'
    | 'PROFILE_STORE_CONFLICT' | 'PROFILE_NOT_FOUND' | 'PROFILE_REVISION_NOT_FOUND'
    | 'PROFILE_DRIVER_MISMATCH' | 'PROFILE_DISABLED', message: string) {
    super(`[${code}] ${message}`); this.name = 'AgentProfileError';
  }
}
export const AGENT_PROFILES_STATE_PATH = '/api/settings/app-state/desktop_agent_profiles';
const defaultIds: Record<AgentDriver, string> = {
  pi: 'default-pi', codex: 'default-codex', 'pi-rpc': 'default-pi-rpc', claude: 'default-claude',
};
const labels: Record<AgentDriver, string> = { pi: 'Built-in Pi', codex: 'Codex', 'pi-rpc': 'External Pi', claude: 'Claude Code' };
export function isAgentDriver(value: unknown): value is AgentDriver {
  return typeof value === 'string' && (AGENT_DRIVERS as readonly string[]).includes(value);
}
export function defaultAgentProfileId(driver: AgentDriver): string {
  if (!isAgentDriver(driver)) throw invalid('Unknown agent driver');
  return defaultIds[driver];
}
const invalid = (message: string) => new AgentProfileError('PROFILE_INVALID', message);
function object(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw invalid('Expected an object');
  return value as Record<string, unknown>;
}
function keys(value: Record<string, unknown>, allowed: readonly string[]): void {
  if (Object.keys(value).some(key => !allowed.includes(key))) throw invalid('Unexpected configuration field');
}
function text(value: unknown, max: number, description: string): string {
  if (typeof value !== 'string' || !value.trim() || value.length > max || /[\u0000-\u001f\u007f-\u009f]/u.test(value))
    throw invalid(`Invalid ${description}`);
  return value;
}
export function validateAgentProfileRef(value: unknown): AgentProfileRef {
  const input = object(value); keys(input, ['id', 'revision']);
  const id = text(input.id, 128, 'profile ID');
  if (!/^[a-zA-Z0-9][a-zA-Z0-9_-]*$/u.test(id)) throw invalid('Invalid profile ID');
  if (!Number.isSafeInteger(input.revision) || (input.revision as number) < 1) throw invalid('Invalid profile revision');
  return { id, revision: input.revision as number };
}
/** Does not read paths, environment variables, credentials, or executable metadata. */
export function validateAgentProfileLaunch(driver: AgentDriver, value: unknown): AgentProfileLaunch {
  if (!isAgentDriver(driver)) throw invalid('Unknown agent driver');
  const input = object(value); keys(input, ['command', 'args', 'configDir']);
  if (!Array.isArray(input.args) || input.args.length > 256) throw invalid('Expected at most 256 invocation arguments');
  const args = input.args.map(arg => {
    if (typeof arg !== 'string' || arg.length > 16384 || /[\u0000-\u001f\u007f-\u009f]/u.test(arg)) throw invalid('Invalid invocation argument');
    // Reject explicit credential inputs. Arbitrary strings cannot be classified as secrets;
    // callers must only supply nonsecret native CLI configuration references.
    if (/^--?(?:api[-_]?key|access[-_]?token|token|password|secret|authorization|env)(?:=|$)/iu.test(arg)
      || /^(?:HIVE_[A-Z0-9_]+|[A-Z0-9_]*(?:TOKEN|PASSWORD|SECRET|API_KEY))=/u.test(arg))
      throw invalid('Credentials and environment overrides are not profile arguments');
    return arg;
  });
  if (driver === 'pi' && (input.command !== undefined || args.length)) throw invalid('Built-in Pi does not accept an executable or invocation arguments');
  if (driver === 'claude' && args.length) throw invalid('Claude SDK profiles do not accept ordered invocation arguments');
  const command = input.command === undefined ? undefined : text(input.command, 4096, 'executable').trim();
  if (driver !== 'pi' && command === undefined) throw invalid('External agent profiles require an executable');
  const configDir = input.configDir === undefined ? undefined : text(input.configDir, 4096, 'configuration directory');
  // Profiles launch on this host. A Windows-looking path on POSIX would be relative
  // when passed to a native CLI's configuration-home environment variable.
  if (configDir !== undefined && !isAbsolute(configDir)) throw invalid('Configuration directory must be absolute on this host');
  return { ...(command !== undefined && { command }), args, ...(configDir !== undefined && { configDir }) };
}
function profile(value: unknown): AgentProfile {
  const input = object(value); keys(input, ['id', 'driver', 'label', 'enabled', 'currentRevision', 'revisions']);
  const ref = validateAgentProfileRef({ id: input.id, revision: input.currentRevision });
  if (!isAgentDriver(input.driver)) throw invalid('Unknown agent driver');
  const driver = input.driver;
  const reserved = Object.entries(defaultIds).find(([, id]) => id === ref.id);
  if (reserved && reserved[0] !== driver) throw invalid('Default profile driver cannot change');
  const label = text(input.label, 256, 'profile label').trim();
  if (typeof input.enabled !== 'boolean' || !Array.isArray(input.revisions) || !input.revisions.length) throw invalid('Invalid profile state');
  const revisions = input.revisions.map((value, index): AgentProfileRevision => {
    const entry = object(value); keys(entry, ['revision', 'launch', 'legacyConfiguration']);
    if (entry.revision !== index + 1 || entry.legacyConfiguration !== undefined && entry.legacyConfiguration !== true)
      throw invalid('Invalid profile revision history');
    return { revision: index + 1, launch: validateAgentProfileLaunch(driver, entry.launch),
      ...(entry.legacyConfiguration === true && { legacyConfiguration: true }) };
  });
  if (ref.revision !== revisions.length) throw invalid('Current profile revision must be the latest saved revision');
  return { id: ref.id, driver, label, enabled: input.enabled, currentRevision: ref.revision, revisions };
}
function storedProfiles(value: unknown): StoredProfiles {
  const input = object(value); keys(input, ['schemaVersion', 'storeVersion', 'profiles']);
  if (input.schemaVersion !== 1 || !Number.isSafeInteger(input.storeVersion) || (input.storeVersion as number) < 1
    || !Array.isArray(input.profiles)) throw invalid('Invalid profile store');
  const profiles = input.profiles.map(profile);
  const ids = new Set(profiles.map(entry => entry.id));
  if (ids.size !== profiles.length || AGENT_DRIVERS.some(driver => !ids.has(defaultIds[driver]))) throw invalid('Invalid default/duplicate profiles');
  return { schemaVersion: 1, storeVersion: input.storeVersion as number, profiles };
}
function resolved(store: StoredProfiles, rawRef: AgentProfileRef, expectedDriver?: AgentDriver): ResolvedAgentProfile {
  const ref = validateAgentProfileRef(rawRef);
  if (expectedDriver !== undefined && !isAgentDriver(expectedDriver)) throw invalid('Unknown expected agent driver');
  const entry = store.profiles.find(item => item.id === ref.id);
  if (!entry) throw new AgentProfileError('PROFILE_NOT_FOUND', 'The saved profile does not exist');
  if (expectedDriver !== undefined && entry.driver !== expectedDriver)
    throw new AgentProfileError('PROFILE_DRIVER_MISMATCH', 'The profile does not use the requested backend');
  const revision = entry.revisions.find(item => item.revision === ref.revision);
  if (!revision) throw new AgentProfileError('PROFILE_REVISION_NOT_FOUND', 'The saved profile revision does not exist');
  return { ref, driver: entry.driver, label: entry.label, enabled: entry.enabled,
    launch: structuredClone(revision.launch), ...(revision.legacyConfiguration && { legacyConfiguration: true }) };
}

/** One instance per runtime. Serializes its reads/mutations; app-state PUT is one atomic SQLite row.
 * storeVersion detects stale callers within that runtime, not concurrent external processes.
 * No local cache is advanced before persistence succeeds. Historical revisions are never rewritten.
 */
export class AgentProfiles {
  private queue: Promise<void> = Promise.resolve();
  private observedStore: StoredProfiles | undefined;
  constructor(private readonly hive: Pick<HiveHost, 'request'>) {}
  private serial<T>(operation: () => Promise<T>): Promise<T> {
    const task = this.queue.then(operation);
    this.queue = task.then(() => {}, () => {});
    return task;
  }
  private async read(): Promise<StoredProfiles | undefined> {
    let raw: unknown;
    try { raw = await this.hive.request('GET', AGENT_PROFILES_STATE_PATH); }
    catch (error) { this.observedStore = undefined; throw error; }
    try {
      const response = object(raw);
      if (response.value === null) { this.observedStore = undefined; return undefined; }
      if (typeof response.value !== 'string') throw invalid('Invalid profile row');
      const store = storedProfiles(JSON.parse(response.value));
      this.observedStore = structuredClone(store);
      return store;
    } catch {
      this.observedStore = undefined;
      throw new AgentProfileError('PROFILE_STORE_CORRUPT', 'Saved agent profiles are invalid; no configuration was replaced');
    }
  }
  private async requireStore(): Promise<StoredProfiles> {
    const store = await this.read();
    if (!store) throw new AgentProfileError('PROFILE_STORE_UNINITIALIZED', 'Initialize profiles from explicit desktop settings first');
    return store;
  }
  private async write(store: StoredProfiles): Promise<void> {
    // Validate the entire row before a single write: no partial default/profile migration.
    const validated = storedProfiles(store);
    try {
      await this.hive.request('PUT', AGENT_PROFILES_STATE_PATH, { value: JSON.stringify(validated) });
      this.observedStore = structuredClone(validated);
    } catch (error) {
      // A failed response may follow a committed PUT. Force a new read before synchronous use.
      this.observedStore = undefined;
      throw error;
    }
  }
  initialize(settings: DesktopSettingsSnapshot): Promise<AgentProfilesSnapshot> {
    const captured: unknown = structuredClone(settings);
    return this.serial(async () => {
      const existing = await this.read();
      if (existing) return this.snapshot(existing);
      const effective = object(object(captured).effective);
      const profiles = AGENT_DRIVERS.map(driver => {
        const executable = driver === 'pi' ? undefined : object(effective[driver === 'pi-rpc' ? 'pi' : driver]).command;
        return profile({ id: defaultIds[driver], driver, label: labels[driver], enabled: true, currentRevision: 1,
          revisions: [{ revision: 1, legacyConfiguration: true,
            launch: { ...(executable !== undefined && { command: executable }), args: [] } }] });
      });
      const store: StoredProfiles = { schemaVersion: 1, storeVersion: 1, profiles };
      await this.write(store);
      return this.snapshot(store);
    });
  }
  private snapshot(store: StoredProfiles): AgentProfilesSnapshot {
    return structuredClone({ storeVersion: store.storeVersion, profiles: store.profiles });
  }
  list(): Promise<AgentProfilesSnapshot> { return this.serial(async () => this.snapshot(await this.requireStore())); }
  getRef(ref: AgentProfileRef, expectedDriver?: AgentDriver): Promise<ResolvedAgentProfile> {
    const captured = structuredClone(ref);
    return this.serial(async () => resolved(await this.requireStore(), captured, expectedDriver));
  }
  resolveForExecution(ref: AgentProfileRef, expectedDriver?: AgentDriver): Promise<ResolvedAgentProfile> {
    const captured = structuredClone(ref);
    return this.serial(async () => {
      const result = resolved(await this.requireStore(), captured, expectedDriver);
      if (!result.enabled) throw new AgentProfileError('PROFILE_DISABLED', 'Enable this same profile before starting or resuming it');
      return result;
    });
  }
  currentRef(id: string): Promise<AgentProfileRef> {
    return this.serial(async () => {
      const store = await this.requireStore();
      const entry = store.profiles.find(item => item.id === id);
      if (!entry) throw new AgentProfileError('PROFILE_NOT_FOUND', 'The requested profile does not exist');
      return { id: entry.id, revision: entry.currentRevision };
    });
  }
  /** For Hive's synchronous spawn hook, after initialize/list or a successful mutation.
   * This is only the last confirmed store snapshot; execution still resolves and checks it.
   */
  currentRefSnapshot(id: string): AgentProfileRef {
    if (!this.observedStore) throw new AgentProfileError('PROFILE_STORE_UNINITIALIZED', 'Read profile settings before resolving a synchronous launch');
    const entry = this.observedStore.profiles.find(item => item.id === id);
    if (!entry) throw new AgentProfileError('PROFILE_NOT_FOUND', 'The requested profile does not exist');
    return { id: entry.id, revision: entry.currentRevision };
  }
  /** Preserve Hive's synchronous member/spawn API. Start must resolve authoritatively again. */
  resolveForExecutionSnapshot(ref: AgentProfileRef, expectedDriver?: AgentDriver): ResolvedAgentProfile {
    if (!this.observedStore) throw new AgentProfileError('PROFILE_STORE_UNINITIALIZED', 'Read profile settings before resolving a synchronous launch');
    const result = resolved(this.observedStore, ref, expectedDriver);
    if (!result.enabled) throw new AgentProfileError('PROFILE_DISABLED', 'Enable this same profile before starting or resuming it');
    return result;
  }
  /** Legacy records have no evidence of later profile edits: pin the migration revision. */
  legacyRef(driver: AgentDriver): Promise<AgentProfileRef> {
    return this.serial(async () => resolved(await this.requireStore(), { id: defaultAgentProfileId(driver), revision: 1 }, driver).ref);
  }
  private async mutate(expectedStoreVersion: number, operation: (store: StoredProfiles) => { id: string; changed: boolean }): Promise<AgentProfileMutation> {
    const store = await this.requireStore();
    if (!Number.isSafeInteger(expectedStoreVersion) || expectedStoreVersion !== store.storeVersion)
      throw new AgentProfileError('PROFILE_STORE_CONFLICT', 'Profile settings changed; read the latest store version before saving');
    const { id, changed } = operation(store);
    if (changed) {
      if (store.storeVersion === Number.MAX_SAFE_INTEGER) throw invalid('Profile store version is exhausted');
      store.storeVersion++;
      await this.write(store);
    }
    const entry = store.profiles.find(item => item.id === id)!;
    return { storeVersion: store.storeVersion, profile: resolved(store, { id, revision: entry.currentRevision }) };
  }
  create(input: CreateAgentProfile, expectedStoreVersion: number): Promise<AgentProfileMutation> {
    // Snapshot caller input before queued work to prevent mutation changing the pending request.
    const captured: unknown = structuredClone(input);
    return this.serial(() => this.mutate(expectedStoreVersion, store => {
      const value = object(captured); keys(value, ['driver', 'label', 'launch']);
      const entry = profile({ id: randomUUID(), driver: value.driver, label: value.label, enabled: true, currentRevision: 1,
        revisions: [{ revision: 1, launch: value.launch }] });
      store.profiles.push(entry);
      return { id: entry.id, changed: true };
    }));
  }
  update(id: string, input: UpdateAgentProfile, expectedStoreVersion: number): Promise<AgentProfileMutation> {
    const captured: unknown = structuredClone(input);
    return this.serial(() => this.mutate(expectedStoreVersion, store => {
      const value = object(captured); keys(value, ['label', 'launch']);
      const entry = store.profiles.find(item => item.id === id);
      if (!entry) throw new AgentProfileError('PROFILE_NOT_FOUND', 'The requested profile does not exist');
      const label = value.label === undefined ? entry.label : text(value.label, 256, 'profile label').trim();
      const launch = value.launch === undefined ? undefined : validateAgentProfileLaunch(entry.driver, value.launch);
      let changed = label !== entry.label;
      if (launch && JSON.stringify(launch) !== JSON.stringify(entry.revisions.at(-1)!.launch)) {
        if (!entry.enabled) throw new AgentProfileError('PROFILE_DISABLED', 'Enable this profile before creating a new configuration revision');
        entry.revisions.push({ revision: ++entry.currentRevision, launch }); changed = true;
      }
      entry.label = label;
      return { id: entry.id, changed };
    }));
  }
  enableDisable(id: string, enabled: boolean, expectedStoreVersion: number): Promise<AgentProfileMutation> {
    return this.serial(() => this.mutate(expectedStoreVersion, store => {
      if (typeof enabled !== 'boolean') throw invalid('Enabled must be boolean');
      const entry = store.profiles.find(item => item.id === id);
      if (!entry) throw new AgentProfileError('PROFILE_NOT_FOUND', 'The requested profile does not exist');
      const changed = entry.enabled !== enabled; entry.enabled = enabled;
      return { id, changed };
    }));
  }
}

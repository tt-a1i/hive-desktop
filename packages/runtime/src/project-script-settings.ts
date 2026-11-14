import { createHash, randomUUID } from 'node:crypto';

export interface ProjectScript { id: string; version: string; name: string; command: string }
export interface ProjectScriptsSnapshot { version: 1; revision: string; scripts: ProjectScript[] }
/** The CAS must compare and mutate one SQLite app_state row in a single transaction. */
export interface ProjectScriptStatePort {
  read(key: string, workspaceId: string): string | null;
  compareAndSwap(key: string, expected: string | null, replacement: string | null, workspaceId: string): boolean;
}
export class ProjectScriptSettingsError extends Error {
  constructor(public readonly code: 'INVALID' | 'CORRUPT' | 'CONFLICT' | 'NOT_FOUND', message: string) { super(`[SCRIPT_SETTINGS_${code}] ${message}`); this.name = 'ProjectScriptSettingsError'; }
}
const uuid = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;
const fail = (code: ProjectScriptSettingsError['code'], message: string): never => { throw new ProjectScriptSettingsError(code, message); };
function key(workspaceId: string): string {
  if (typeof workspaceId !== 'string' || !workspaceId || workspaceId.length > 256 || /[\x00-\x1f\x7f]/.test(workspaceId)) fail('INVALID', 'Invalid workspace identity');
  return `desktop_project_scripts_v1_${createHash('sha256').update(workspaceId).digest('hex')}`;
}
function text(value: unknown, kind: 'name' | 'command'): string {
  const limit = kind === 'name' ? 120 : 16384;
  if (typeof value !== 'string' || !value.trim() || value.length > limit || (kind === 'name' ? /[\x00-\x1f\x7f]/ : /[\x00-\x08\x0b\x0c\x0e-\x1f\x7f]/).test(value)) fail('INVALID', `Invalid script ${kind}`);
  return value as string;
}
function decode(raw: string | null, workspaceId: string): ProjectScriptsSnapshot {
  if (raw === null) return { version: 1, revision: '0', scripts: [] };
  try {
    const record: unknown = JSON.parse(raw);
    if (!record || typeof record !== 'object' || Array.isArray(record)) throw 0;
    const object = record as Record<string, unknown>;
    if (Object.keys(object).sort().join(',') !== 'revision,scripts,version,workspaceId' || object.version !== 1 || object.workspaceId !== workspaceId || typeof object.revision !== 'string' || !uuid.test(object.revision) || !Array.isArray(object.scripts) || object.scripts.length > 100) throw 0;
    const ids = new Set<string>();
    const scripts = object.scripts.map((item: unknown): ProjectScript => {
      if (!item || typeof item !== 'object' || Array.isArray(item)) throw 0;
      const script = item as Record<string, unknown>;
      if (Object.keys(script).sort().join(',') !== 'command,id,name,version' || typeof script.id !== 'string' || !uuid.test(script.id) || ids.has(script.id) || typeof script.version !== 'string' || !uuid.test(script.version)) throw 0;
      ids.add(script.id);
      return { id: script.id, version: script.version, name: text(script.name, 'name'), command: text(script.command, 'command') };
    });
    return { version: 1, revision: object.revision, scripts };
  } catch { return fail('CORRUPT', 'Saved project scripts are invalid; no changes were made'); }
}
/** The caller must coordinate workspace deletion with mutations using its workspace lifecycle lock. */
export class ProjectScriptSettings {
  constructor(private readonly port: ProjectScriptStatePort, private readonly assertWorkspaceAvailable: (workspaceId: string) => void | Promise<void>) {}
  private async current(workspaceId: string) {
    const stateKey = key(workspaceId); await this.assertWorkspaceAvailable(workspaceId);
    const raw = this.port.read(stateKey, workspaceId);
    return { stateKey, raw, snapshot: decode(raw, workspaceId) };
  }
  async list(workspaceId: string): Promise<ProjectScriptsSnapshot> { return (await this.current(workspaceId)).snapshot; }
  async capture(workspaceId: string, id: string, expectedVersion: string): Promise<ProjectScript> {
    const snapshot = await this.list(workspaceId);
    const script = snapshot.scripts.find(item => item.id === id);
    if (!script) return fail('NOT_FOUND', 'Saved project script not found');
    if (script.version !== expectedVersion) return fail('CONFLICT', 'Project script changed; reload settings');
    return { ...script };
  }
  async save(workspaceId: string, expectedRevision: string, input: { id?: string; name: unknown; command: unknown }): Promise<ProjectScriptsSnapshot> {
    if (!input || typeof input !== 'object' || Array.isArray(input) || Object.keys(input).some(k => !['id', 'name', 'command'].includes(k))) return fail('INVALID', 'Invalid project script');
    const name = text(input.name, 'name'), command = text(input.command, 'command');
    if (input.id !== undefined && (typeof input.id !== 'string' || !uuid.test(input.id))) return fail('INVALID', 'Invalid project script ID');
    const {stateKey, raw, snapshot} = await this.current(workspaceId);
    if (snapshot.revision !== expectedRevision) return fail('CONFLICT', 'Project scripts changed; reload settings');
    const index = input.id === undefined ? -1 : snapshot.scripts.findIndex(script => script.id === input.id);
    if (input.id !== undefined && index < 0) return fail('NOT_FOUND', 'Saved project script not found');
    if (index < 0 && snapshot.scripts.length >= 100) return fail('INVALID', 'Too many project scripts');
    const next = { id: input.id ?? randomUUID(), version: randomUUID(), name, command };
    const scripts = [...snapshot.scripts]; if (index < 0) scripts.push(next); else scripts[index] = next;
    return this.commit(workspaceId, stateKey, raw, scripts);
  }
  async remove(workspaceId: string, expectedRevision: string, id: string): Promise<ProjectScriptsSnapshot> {
    const {stateKey, raw, snapshot} = await this.current(workspaceId);
    if (snapshot.revision !== expectedRevision) return fail('CONFLICT', 'Project scripts changed; reload settings');
    if (!snapshot.scripts.some(script => script.id === id)) return fail('NOT_FOUND', 'Saved project script not found');
    return this.commit(workspaceId, stateKey, raw, snapshot.scripts.filter(script => script.id !== id));
  }
  /** Invoke before workspace deletion under its lifecycle lock; never silently replace corrupt data. */
  async clearWorkspace(workspaceId: string, expectedRevision: string): Promise<void> {
    const {stateKey, raw, snapshot} = await this.current(workspaceId);
    if (snapshot.revision !== expectedRevision) return fail('CONFLICT', 'Project scripts changed; reload settings');
    if (raw !== null && !this.port.compareAndSwap(stateKey, raw, null, workspaceId)) fail('CONFLICT', 'Project scripts changed; reload settings');
  }
  private commit(workspaceId: string, stateKey: string, raw: string | null, scripts: ProjectScript[]): ProjectScriptsSnapshot {
    const next: ProjectScriptsSnapshot = {version: 1, revision: randomUUID(), scripts};
    if (!this.port.compareAndSwap(stateKey, raw, JSON.stringify({ ...next, workspaceId }), workspaceId)) return fail('CONFLICT', 'Project scripts changed; reload settings');
    return next;
  }
}

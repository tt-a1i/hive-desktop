import { existsSync, mkdirSync, readFileSync, renameSync, rmSync, writeFileSync } from 'node:fs';
import { randomUUID } from 'node:crypto';
import { join } from 'node:path';

export type ChatBackend = 'pi' | 'codex' | 'pi-rpc' | 'claude';

// Pi core uses this identity until an actual model has been selected.
export function isUnsetPiModel(model: { provider: string; id: string } | undefined): boolean {
  return model?.provider === 'unknown' && model.id === 'unknown';
}

export interface DeliveryFailure {deliveryId:string;message:string}
export interface ChatExecution { kind: 'worktree'; path: string; branch: string }
export class SessionIdentityError extends Error {
  constructor(readonly code: string, message: string) { super(`[${code}] ${message}`); this.name = 'SessionIdentityError'; }
}

export interface ChatRecord {
  backend: ChatBackend;
  execution?: ChatExecution;
  codexThreadId?: string;
  nativeclaudeSessionId?: string;
  id: string;
  workspaceId: string;
  title: string;
  customTitle?: boolean;
  pinned?: boolean;
  archived?: boolean;
  lastDeliveryFailure?: DeliveryFailure;
  reasoning?: {provider:string;modelId:string;level:string};
  sessionFile: string | null;
  agentId?: string;
  /** The first binding is immutable; agentId remains only the current binding. */
  originAgentId?: string;
  identityState?: 'new' | 'known' | 'unknown';
  piSessionId?: string;
  externalPiSessionId?: string;
  startupContext?: string;
  model?: { provider: string; id: string; name: string };
  updatedAt: number;
}

// Desktop navigation metadata only. Hive owns workspaces/team state and each
// backend owns its conversation transcript. This index is not a second collaboration ledger.
export class ChatIndex {
  private records: ChatRecord[];
  private readonly file: string;
  constructor(directory: string) {
    mkdirSync(directory, { recursive: true });
    this.file = join(directory, 'index.json');
    const value: unknown = existsSync(this.file) ? JSON.parse(readFileSync(this.file, 'utf8')) : [];
    if (!Array.isArray(value) || !value.every(isChatRecord)) throw new Error('Invalid desktop chat index');
    this.records = value.map(record => {
      // Older automatic titles could end with half an emoji after UTF-16 slicing.
      // With /u, this range matches only unpaired surrogates, preserving valid emoji.
      const title = record.customTitle ? record.title : record.title.replace(/[\uD800-\uDFFF]/gu, '');
      const normalized = normalizeIdentity({ ...record, title, backend: record.backend ?? 'pi' });
      if (normalized.backend === 'pi' && isUnsetPiModel(normalized.model)) delete normalized.model;
      return normalized;
    });
    if (JSON.stringify(value) !== JSON.stringify(this.records)) this.commit(this.records);
  }
  list(workspaceId: string): ChatRecord[] {
    return this.records.filter(record => record.workspaceId === workspaceId).sort((a, b) => Number(!!b.pinned) - Number(!!a.pinned) || b.updatedAt - a.updatedAt);
  }
  peek(id:string):ChatRecord|undefined {return this.records.find(record=>record.id===id);}
  get(id: string): ChatRecord {
    const record = this.peek(id);
    if (!record) throw new Error('Conversation not found');
    return record;
  }
  forAgent(workspaceId: string, agentId: string): ChatRecord | undefined {
    return this.records.find(record => record.workspaceId === workspaceId && record.agentId === agentId);
  }
  bind(id: string, agentId: string): ChatRecord {
    const original = this.get(id);
    if (original.identityState === 'unknown') throw new SessionIdentityError('SESSION_IDENTITY_UNKNOWN', 'This older conversation has no reliable original agent identity. Its history is read-only; create a new conversation to continue.');
    if (original.originAgentId && original.originAgentId !== agentId) throw new SessionIdentityError('SESSION_IDENTITY_CONFLICT', 'A historical conversation cannot be rebound to a different agent.');
    const bound: ChatRecord = { ...original, agentId, originAgentId: original.originAgentId ?? agentId, identityState: 'known' };
    const next = this.records.map(record => {
      if (record.id === id) return bound;
      if (record.workspaceId !== bound.workspaceId || record.agentId !== agentId) return record;
      const { agentId: previousAgentId, ...unbound } = record;
      return unbound;
    });
    this.commit(next);
    return bound;
  }
  save(record: ChatRecord): void {
    const existing = this.peek(record.id);
    if (existing?.identityState === 'unknown' && (record.agentId || record.originAgentId) && !existing.originAgentId)
      throw new SessionIdentityError('SESSION_IDENTITY_UNKNOWN', 'An older unknown conversation needs exact persisted source evidence before it can be bound.');
    if (existing?.originAgentId && ((record.originAgentId && record.originAgentId !== existing.originAgentId)
      || (record.agentId && record.agentId !== existing.originAgentId))) throw new SessionIdentityError('SESSION_IDENTITY_CONFLICT', 'The original agent identity cannot be changed.');
    const normalized = normalizeIdentity(existing?.originAgentId
      ? {...record, originAgentId: existing.originAgentId, identityState: 'known'}
      : existing ? {...record, identityState: existing.identityState ?? 'unknown'} : record);
    this.commit([...this.records.filter(item => item.id !== record.id), normalized]);
  }
  /** Only precise persisted provenance may repair a legacy unknown identity. */
  establishOrigin(id: string, agentId: string): ChatRecord {
    const original = this.get(id);
    if (original.originAgentId && original.originAgentId !== agentId || original.agentId && original.agentId !== agentId)
      throw new SessionIdentityError('SESSION_IDENTITY_CONFLICT', 'Persisted execution sources disagree about the original agent.');
    if (original.originAgentId === agentId) return original;
    const recovered: ChatRecord = {...original, originAgentId: agentId, identityState: 'known'};
    this.commit(this.records.map(record => record.id === id ? recovered : record));
    return recovered;
  }
  removeWorkspace(workspaceId: string): void {
    this.commit(this.records.filter(record => record.workspaceId !== workspaceId));
  }
  private commit(next: ChatRecord[]): void {
    const temporary = `${this.file}.${randomUUID()}.tmp`;
    try {
      writeFileSync(temporary, JSON.stringify(next, null, 2), { encoding: 'utf8', mode: 0o600, flush: true });
      renameSync(temporary, this.file);
      this.records = next;
    } finally { rmSync(temporary, { force: true }); }
  }
}

function normalizeIdentity(record: ChatRecord): ChatRecord {
  if (record.originAgentId) {
    if (record.agentId && record.agentId !== record.originAgentId) throw new SessionIdentityError('SESSION_IDENTITY_CONFLICT', 'Current and original conversation agent identities disagree.');
    return {...record, identityState: 'known'};
  }
  if (record.agentId) return {...record, identityState: 'known', originAgentId: record.agentId};
  return {...record, identityState: record.identityState === 'new' ? 'new' : 'unknown'};
}

function isChatRecord(value: unknown): value is ChatRecord {
  if (!value || typeof value !== 'object') return false;
  const record = value as Record<string, unknown>;
  if (typeof record.id !== 'string' || typeof record.workspaceId !== 'string' || typeof record.title !== 'string'
    || typeof record.updatedAt !== 'number' || (record.sessionFile !== null && typeof record.sessionFile !== 'string')) return false;
  if (record.pinned !== undefined && typeof record.pinned !== 'boolean') return false;
  if (record.archived !== undefined && typeof record.archived !== 'boolean') return false;
  if (record.execution !== undefined) {
    const execution = record.execution as Record<string, unknown> | null;
    if (!execution || typeof execution !== 'object' || Array.isArray(execution)
      || execution.kind !== 'worktree' || typeof execution.path !== 'string' || !execution.path
      || typeof execution.branch !== 'string' || !execution.branch) return false;
  }
  if (record.lastDeliveryFailure !== undefined) {
    const failure=record.lastDeliveryFailure as Record<string,unknown>|null;
    if (!failure || typeof failure!=='object' || Array.isArray(failure) || typeof failure.deliveryId!=='string' || !failure.deliveryId || typeof failure.message!=='string') return false;
  }
  if (record.reasoning !== undefined) {
    const setting = record.reasoning as Record<string,unknown> | null;
    if (!setting || typeof setting !== 'object' || Array.isArray(setting) || !['provider','modelId','level'].every(key=>typeof setting[key]==='string' && (setting[key] as string).length>0)) return false;
  }
  if (record.customTitle !== undefined && typeof record.customTitle !== 'boolean') return false;
  if (record.backend !== undefined && record.backend !== 'pi' && record.backend !== 'codex' && record.backend !== 'pi-rpc' && record.backend !== 'claude') return false;
  if (record.nativeclaudeSessionId !== undefined && typeof record.nativeclaudeSessionId !== 'string') return false;
  if (record.codexThreadId !== undefined && typeof record.codexThreadId !== 'string') return false;
  if (record.agentId !== undefined && typeof record.agentId !== 'string') return false;
  if (record.originAgentId !== undefined && (typeof record.originAgentId !== 'string' || !record.originAgentId)) return false;
  if (record.identityState !== undefined && !['new','known','unknown'].includes(record.identityState as string)) return false;
  if (record.identityState === 'known' && !record.originAgentId && !record.agentId) return false;
  if (record.externalPiSessionId !== undefined && typeof record.externalPiSessionId !== 'string') return false;
  if (record.piSessionId !== undefined && typeof record.piSessionId !== 'string') return false;
  if (record.startupContext !== undefined && typeof record.startupContext !== 'string') return false;
  if (record.model !== undefined) {
    if (!record.model || typeof record.model !== 'object') return false;
    const model = record.model as Record<string, unknown>;
    if (typeof model.provider !== 'string' || typeof model.id !== 'string' || typeof model.name !== 'string') return false;
  }
  return true;
}

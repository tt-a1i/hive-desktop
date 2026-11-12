import { existsSync, mkdirSync, readFileSync, renameSync, rmSync, writeFileSync } from 'node:fs';
import { randomUUID } from 'node:crypto';
import { join } from 'node:path';

export type ChatBackend = 'pi' | 'codex' | 'pi-rpc' | 'claude';

// Pi core uses this identity until an actual model has been selected.
export function isUnsetPiModel(model: { provider: string; id: string } | undefined): boolean {
  return model?.provider === 'unknown' && model.id === 'unknown';
}

export interface DeliveryFailure {deliveryId:string;message:string}

export interface ChatRecord {
  backend: ChatBackend;
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
      const normalized = { ...record, title, backend: record.backend ?? 'pi' };
      if (normalized.backend === 'pi' && isUnsetPiModel(normalized.model)) delete normalized.model;
      return normalized;
    });
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
    const bound = { ...this.get(id), agentId };
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
    this.commit([...this.records.filter(item => item.id !== record.id), record]);
  }
  removeWorkspace(workspaceId: string): void {
    this.commit(this.records.filter(record => record.workspaceId !== workspaceId));
  }
  private commit(next: ChatRecord[]): void {
    const temporary = `${this.file}.${randomUUID()}.tmp`;
    try {
      writeFileSync(temporary, JSON.stringify(next, null, 2), { mode: 0o600 });
      renameSync(temporary, this.file);
      this.records = next;
    } finally { rmSync(temporary, { force: true }); }
  }
}

function isChatRecord(value: unknown): value is ChatRecord {
  if (!value || typeof value !== 'object') return false;
  const record = value as Record<string, unknown>;
  if (typeof record.id !== 'string' || typeof record.workspaceId !== 'string' || typeof record.title !== 'string'
    || typeof record.updatedAt !== 'number' || (record.sessionFile !== null && typeof record.sessionFile !== 'string')) return false;
  if (record.pinned !== undefined && typeof record.pinned !== 'boolean') return false;
  if (record.archived !== undefined && typeof record.archived !== 'boolean') return false;
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

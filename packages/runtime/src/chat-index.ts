import { existsSync, mkdirSync, readFileSync, renameSync, writeFileSync } from 'node:fs';
import { randomUUID } from 'node:crypto';
import { join } from 'node:path';

export type ChatBackend = 'pi' | 'codex' | 'pi-rpc' | 'claude';

// Pi core uses this identity until an actual model has been selected.
export function isUnsetPiModel(model: { provider: string; id: string } | undefined): boolean {
  return model?.provider === 'unknown' && model.id === 'unknown';
}

export interface ChatRecord {
  backend: ChatBackend;
  codexThreadId?: string;
  nativeclaudeSessionId?: string;
  id: string;
  workspaceId: string;
  title: string;
  customTitle?: boolean;
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
      const normalized = { ...record, backend: record.backend ?? 'pi' };
      if (normalized.backend === 'pi' && isUnsetPiModel(normalized.model)) delete normalized.model;
      return normalized;
    });
  }
  list(workspaceId: string): ChatRecord[] {
    return this.records.filter(record => record.workspaceId === workspaceId).sort((a, b) => b.updatedAt - a.updatedAt);
  }
  get(id: string): ChatRecord {
    const record = this.records.find(record => record.id === id);
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
    writeFileSync(temporary, JSON.stringify(next, null, 2), { mode: 0o600 });
    renameSync(temporary, this.file);
    this.records = next;
  }
}

function isChatRecord(value: unknown): value is ChatRecord {
  if (!value || typeof value !== 'object') return false;
  const record = value as Record<string, unknown>;
  if (typeof record.id !== 'string' || typeof record.workspaceId !== 'string' || typeof record.title !== 'string'
    || typeof record.updatedAt !== 'number' || (record.sessionFile !== null && typeof record.sessionFile !== 'string')) return false;
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

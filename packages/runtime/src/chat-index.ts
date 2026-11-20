import { isGithubPrReviewSeed, type GithubPrReviewSeed } from './github-pr-review.js';
import type { AgentCapabilities } from './agent-capabilities.js';
import type { AgentProfileRef } from './agent-profiles.js';
import { closeSync, existsSync, fsyncSync, mkdirSync, openSync, readFileSync, renameSync, rmSync, writeFileSync } from 'node:fs';
import { randomUUID } from 'node:crypto';
import { dirname, isAbsolute, join } from 'node:path';
import { isDeepStrictEqual } from 'node:util';
import type { NativeImportSource } from './native-import-source.js';
import { isScratchId, scratchConversationPath } from './scratch-workspace.js';

export type ChatBackend = 'pi' | 'codex' | 'pi-rpc' | 'claude' | 'acp';

// Pi core uses this identity until an actual model has been selected.
export function isUnsetPiModel(model: { provider: string; id: string } | undefined): boolean {
  return model?.provider === 'unknown' && model.id === 'unknown';
}

export interface DeliveryFailure {deliveryId:string;message:string}
export interface ChatExecution { kind: 'worktree'; path: string; branch: string; ownerSessionId?: string }
export interface PinnedSessionRef { workspaceId: string; sessionId: string }
export class SessionIdentityError extends Error {
  constructor(readonly code: string, message: string) { super(`[${code}] ${message}`); this.name = 'SessionIdentityError'; }
}

export interface ChatRecord {
  backend: ChatBackend;
  deferFirstConnection?: true;
  draftCopy?: string;
  worktreeReuse?: {sourceSessionId: string; ownerFingerprint: string};
  handoff?: {operationId:string;snapshotId:string;snapshotHash:string;sourceSessionId:string};
  profile?: AgentProfileRef;
  legacyConfiguration?: true;
  nativeImport?: NativeImportSource;
  lineage?: {operationId:string;parentSessionId:string;nativeUserItemId:string;edit:boolean;codeRelation:{kind:'current'|'checkpoint';checkpointId:string};inheritedItemIds:string[]};
  rewindOperationId?: string;
  rewound?: true;
  prReview?: GithubPrReviewSeed;
  forkDraft?: {text:string;attachmentIds:string[];missingAttachments:string[]};
  execution?: ChatExecution;
  /** The borrowed directory is retained as historical identity only. */
  worktreeReleased?: true;
  scratch?: true;
  codexThreadId?: string;
  nativeclaudeSessionId?: string;
  id: string;
  workspaceId: string;
  title: string;
  customTitle?: boolean;
  pinned?: boolean;
  pinRank?: number;
  archived?: boolean;
  lastDeliveryFailure?: DeliveryFailure;
  inboxReply?: {revision: number; messageId: string; receivedAt?: number};
  nativeMode?: {backend: 'codex' | 'claude'; mode?: string; permissionMode?: string};
  reasoning?: {provider:string;modelId:string;level:string};
  sessionFile: string | null;
  agentId?: string;
  /** The first binding is immutable; agentId remains only the current binding. */
  originAgentId?: string;
  identityState?: 'new' | 'known' | 'unknown';
  piSessionId?: string;
  externalPiSessionId?: string;
  acpSessionId?: string;
  capabilities?: AgentCapabilities;
  startupContext?: string;
  model?: { provider: string; id: string; name: string };
  updatedAt: number;
}

/** Declared directory for offline history. Execution must still verify ownership. */
export function declaredExecutionPath(record: ChatRecord, workspacePath: string): string {
  return record.scratch ? scratchConversationPath(workspacePath, record.workspaceId, record.id)
    : record.execution?.path ?? workspacePath;
}

export function comparePinnedSessions(a: ChatRecord, b: ChatRecord): number {
  if (a.pinRank !== undefined && b.pinRank !== undefined && a.pinRank !== b.pinRank)
    return a.pinRank < b.pinRank ? -1 : 1;
  if ((a.pinRank === undefined) !== (b.pinRank === undefined))
    return a.pinRank === undefined ? 1 : -1;
  return b.updatedAt - a.updatedAt || a.id.localeCompare(b.id);
}

// Desktop navigation metadata only. Hive owns workspaces/team state and each
// backend owns its conversation transcript. This index is not a second collaboration ledger.
export class ChatIndex {
  private records: ChatRecord[];
  private readonly generations = new Map<string,number>();
  private generation = 0;
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
    for(const record of this.records)this.generations.set(record.id,++this.generation);
    if (JSON.stringify(value) !== JSON.stringify(this.records)) this.commit(this.records);
  }
  all(): ChatRecord[] { return this.records.map(recordSnapshot); }
  /** Runtime-local mutation token. Detects equal-value saves and A→B→A without relying on clocks. */
  snapshotVersion(id:string):number {
    if(!this.records.some(record=>record.id===id))throw new Error('Conversation not found');
    return this.generations.get(id)!;
  }
  list(workspaceId: string): ChatRecord[] {
    return this.records.filter(record => record.workspaceId === workspaceId).map(recordSnapshot).sort((a, b) =>
      Number(!!b.pinned) - Number(!!a.pinned) || (a.pinned && b.pinned ? comparePinnedSessions(a, b) : b.updatedAt - a.updatedAt));
  }
  peek(id:string):ChatRecord|undefined {const record=this.records.find(record=>record.id===id);return record && recordSnapshot(record);}
  hasWorktreeBorrowers(workspaceId: string, ownerId: string): boolean {
    return this.records.some(record => record.workspaceId === workspaceId && record.execution?.ownerSessionId === ownerId && !record.worktreeReleased);
  }
  releaseWorktreeUse(workspaceId: string, id: string, ownerId: string): ChatRecord {
    const record = this.get(id);
    if (record.workspaceId !== workspaceId || !record.worktreeReuse || record.execution?.ownerSessionId !== ownerId)
      throw new SessionIdentityError('WORKTREE_SOURCE_INVALID', '请选择当前项目中借用此工作树的会话。');
    if (record.worktreeReleased) return record;
    if (record.agentId) throw new SessionIdentityError('WORKTREE_IN_USE', '请结束此会话的运行和待处理事项，并关闭其终端后重试。');
    const next: ChatRecord = {...record, worktreeReleased: true};
    this.commit(this.records.map(item => item.id === id ? next : item), id);
    return next;
  }
  get(id: string): ChatRecord {
    const record = this.peek(id);
    if (!record) throw new Error('Conversation not found');
    return record;
  }
  forAgent(workspaceId: string, agentId: string): ChatRecord | undefined {
    const record = this.records.find(record => record.workspaceId === workspaceId && record.agentId === agentId);
    return record && recordSnapshot(record);
  }
  bind(id: string, agentId: string): ChatRecord {
    const original = this.get(id);
    assertWorktreeUse(original);
    if (original.identityState === 'unknown') throw new SessionIdentityError('SESSION_IDENTITY_UNKNOWN', 'This older conversation has no reliable original agent identity. Its history is read-only; create a new conversation to continue.');
    if (original.originAgentId && original.originAgentId !== agentId) throw new SessionIdentityError('SESSION_IDENTITY_CONFLICT', 'A historical conversation cannot be rebound to a different agent.');
    const {deferFirstConnection, ...session} = original;
    const bound: ChatRecord = { ...session, agentId, originAgentId: original.originAgentId ?? agentId, identityState: 'known' };
    const next = this.records.map(record => {
      if (record.id === id) return bound;
      if (record.workspaceId !== bound.workspaceId || record.agentId !== agentId) return record;
      const { agentId: previousAgentId, ...unbound } = record;
      return unbound;
    });
    this.commit(next,id);
    return bound;
  }
  save(record: ChatRecord): void {
    const existing = this.peek(record.id);
    if (existing?.worktreeReleased !== record.worktreeReleased)
      throw new SessionIdentityError('WORKTREE_REUSE_CONFLICT', '共享工作树的解除状态不能通过会话保存更改。');
    if (existing && existing.draftCopy !== record.draftCopy)
      throw new SessionIdentityError('DRAFT_COPY_CONFLICT', '该草稿复制请求已用于其他会话。');
    if (existing && (!isDeepStrictEqual(existing.worktreeReuse, record.worktreeReuse)
      || existing.execution?.ownerSessionId !== record.execution?.ownerSessionId
      || record.worktreeReuse && (existing.workspaceId !== record.workspaceId || !isDeepStrictEqual(existing.execution, record.execution))))
      throw new SessionIdentityError('WORKTREE_REUSE_CONFLICT', '共享工作树的会话身份不能更改。');
    if (!validWorktreeReuse(record))
      throw new SessionIdentityError('WORKTREE_REUSE_CONFLICT', '共享工作树的会话身份无效。');
    if (existing && (existing.scratch !== record.scratch || existing.scratch && existing.workspaceId !== record.workspaceId))
      throw new SessionIdentityError('SCRATCH_IDENTITY_CONFLICT', 'A Scratch conversation’s directory ownership cannot be changed.');
    if (record.scratch !== undefined && !isScratchRecord(record))
      throw new SessionIdentityError('SCRATCH_IDENTITY_CONFLICT', 'Invalid Scratch conversation identity.');
    if (existing?.handoff && JSON.stringify(existing.handoff) !== JSON.stringify(record.handoff))
      throw new SessionIdentityError('SESSION_HANDOFF_CONFLICT', 'A fixed historical handoff cannot be changed.');
    if (existing && !isDeepStrictEqual(existing.nativeImport, record.nativeImport))
      throw new SessionIdentityError('NATIVE_IMPORT_CONFLICT', 'The original native import source cannot be changed.');
    if (existing?.nativeImport && (existing.codexThreadId !== record.codexThreadId
      || existing.nativeclaudeSessionId !== record.nativeclaudeSessionId
      || existing.externalPiSessionId !== record.externalPiSessionId || existing.sessionFile !== record.sessionFile))
      throw new SessionIdentityError('NATIVE_IMPORT_CONFLICT', 'The original native import identity cannot be changed.');
    if (existing && (existing.backend !== record.backend || existing.profile && (existing.profile.id !== record.profile?.id || existing.profile.revision !== record.profile?.revision)))
      throw new SessionIdentityError('SESSION_PROFILE_CONFLICT', 'The conversation backend and fixed profile cannot be changed.');
    if (existing?.identityState === 'unknown' && (record.agentId || record.originAgentId) && !existing.originAgentId)
      throw new SessionIdentityError('SESSION_IDENTITY_UNKNOWN', 'An older unknown conversation needs exact persisted source evidence before it can be bound.');
    if (existing?.originAgentId && ((record.originAgentId && record.originAgentId !== existing.originAgentId)
      || (record.agentId && record.agentId !== existing.originAgentId))) throw new SessionIdentityError('SESSION_IDENTITY_CONFLICT', 'The original agent identity cannot be changed.');
    if (existing?.lineage && JSON.stringify(existing.lineage)!==JSON.stringify(record.lineage)) throw new SessionIdentityError('HISTORY_LINEAGE_CONFLICT', 'Fork lineage is immutable');
    const normalized = normalizeIdentity(existing?.originAgentId
      ? {...record, originAgentId: existing.originAgentId, identityState: 'known'}
      : existing ? {...record, identityState: existing.identityState ?? 'unknown'} : record);
    // Metadata saves can hold a pre-reply snapshot. Only noteInboxReply owns
    // this cursor, and a new chat cannot inherit the source chat's cursor.
    if (existing?.inboxReply) normalized.inboxReply = existing.inboxReply;
    else delete normalized.inboxReply;
    // A turn or metadata update can hold a copy from before a pin change.
    if (existing?.pinned !== undefined) normalized.pinned = existing.pinned;
    else if (existing) delete normalized.pinned;
    if (existing?.pinRank !== undefined) normalized.pinRank = existing.pinRank;
    else delete normalized.pinRank;
    this.commit([...this.records.filter(item => item.id !== record.id), normalized],record.id);
  }
  setPinned(id: string, pinned: boolean): ChatRecord {
    const current = this.get(id);
    if (!!current.pinned === pinned) return current;
    const changed = {...current, pinned};
    const order = this.records.filter(record => record.pinned).sort(comparePinnedSessions);
    const firstRank = order[0]?.pinRank;
    let records = this.records;
    delete changed.pinRank;
    if (pinned && firstRank !== undefined) {
      if (firstRank > Number.MIN_SAFE_INTEGER) changed.pinRank = firstRank - 1;
      else {
        // Rebuild ranks from the explicit order before the next signed rank
        // would exceed the JSON-safe integer range.
        records = this.withPinRanks(order.map(record => record.id));
        changed.pinRank = -1;
      }
    }
    this.commit(records.map(record => record.id === id ? changed : record), id, true);
    return recordSnapshot(changed);
  }
  movePinned(source: PinnedSessionRef, target: PinnedSessionRef, position: 'before' | 'after',
      expectedOrder: string[], workspaceIds: ReadonlySet<string>): string[] {
    const order = this.records.filter(record => record.pinned).sort(comparePinnedSessions);
    const visible = (record: ChatRecord) => !record.archived && workspaceIds.has(record.workspaceId);
    const current = order.filter(visible);
    if (expectedOrder.length !== current.length || current.some((session, index) => session.id !== expectedOrder[index]))
      throw new SessionIdentityError('PIN_ORDER_STALE', 'The pinned conversations changed. Refresh their order and try again.');
    const matches = (ref: PinnedSessionRef) => current.some(session => session.id === ref.sessionId && session.workspaceId === ref.workspaceId);
    if (source.sessionId === target.sessionId || !matches(source) || !matches(target))
      throw new SessionIdentityError('PIN_SOURCE_UNAVAILABLE', 'Select two distinct pinned conversations in their original workspaces.');
    const orderedIds = expectedOrder.filter(id => id !== source.sessionId);
    const slot = orderedIds.indexOf(target.sessionId) + Number(position === 'after');
    orderedIds.splice(slot, 0, source.sessionId);

    // Archived pins stay in their existing slots. Reordering visible pins
    // cannot indirectly move a hidden pin or invalidate its archive receipt.
    let next = 0;
    const rankedIds = order.map(session => visible(session) ? orderedIds[next++]! : session.id);
    const slots = order.map(session => session.pinRank);
    const reusableSlots = slots.every(rank => rank !== undefined) && new Set(slots).size === slots.length;
    const records = this.withPinRanks(rankedIds, reusableSlots ? slots : undefined);
    if (records.some((record, index) => record !== this.records[index]))
      this.commit(records, undefined, true);
    return orderedIds;
  }
  private withPinRanks(orderedIds: string[], slots?: number[]): ChatRecord[] {
    const ranks = new Map(orderedIds.map((id, rank) => [id, slots?.[rank] ?? rank]));
    return this.records.map(record => {
      const pinRank = ranks.get(record.id);
      return pinRank === undefined || pinRank === record.pinRank ? record : {...record, pinRank};
    });
  }
  noteInboxReply(id: string, messageId: string): {workspaceId: string; revision: number; receivedAt?: number} | undefined {
    const record = this.peek(id);
    if (!record || !messageId || record.inboxReply?.messageId === messageId) return;
    const revision = (record.inboxReply?.revision ?? 0) + 1;
    if (!Number.isSafeInteger(revision)) throw new Error('Inbox reply revision exhausted');
    const receivedAt = Date.now();
    const receipt = {revision, ...(receivedAt > 0 && {receivedAt})};
    const updated = {...record, inboxReply: {...receipt, messageId}};
    this.commit(this.records.map(item => item.id === id ? updated : item), id);
    return {workspaceId: record.workspaceId, ...receipt};
  }
  addImported(records: ChatRecord[]): void {
    const ids = new Set(this.records.map(record => record.id));
    const next = [...this.records];
    for (const record of records) {
      if (ids.has(record.id) || !record.nativeImport || !isChatRecord(record))
        throw new SessionIdentityError('NATIVE_IMPORT_CONFLICT', 'An imported conversation identity is invalid or already recorded.');
      ids.add(record.id);
      next.push(normalizeIdentity(record));
    }
    if (records.length) this.commit(next);
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
  private commit(next: ChatRecord[], touchedId?:string, rankOnlyForOtherRecords = false): void {
    const temporary = `${this.file}.${randomUUID()}.tmp`;
    try {
      writeFileSync(temporary, JSON.stringify(next, null, 2), { encoding: 'utf8', mode: 0o600, flush: true });
      renameSync(temporary, this.file);
      // A successful rename publishes the new index even if directory sync
      // later fails. Keep retries and subsequent changes on that revision.
      const previous=new Map(this.records.map(record=>[record.id,record]));
      for(const record of next)if(record.id===touchedId||!previous.has(record.id)||
        !rankOnlyForOtherRecords&&!isDeepStrictEqual(previous.get(record.id),record))this.generations.set(record.id,++this.generation);
      const ids=new Set(next.map(record=>record.id));for(const id of this.generations.keys())if(!ids.has(id))this.generations.delete(id);
      this.records = next.map(recordSnapshot);
      if(process.platform!=='win32'){const directory=openSync(dirname(this.file),'r');try{fsyncSync(directory);}finally{closeSync(directory);}}
    } finally { rmSync(temporary, { force: true }); }
  }
}

function recordSnapshot(record: ChatRecord): ChatRecord {
  return structuredClone(record);
}

function normalizeIdentity(record: ChatRecord): ChatRecord {
  if (record.originAgentId) {
    if (record.agentId && record.agentId !== record.originAgentId) throw new SessionIdentityError('SESSION_IDENTITY_CONFLICT', 'Current and original conversation agent identities disagree.');
    return {...record, identityState: 'known'};
  }
  if (record.agentId) return {...record, identityState: 'known', originAgentId: record.agentId};
  return {...record, identityState: record.identityState === 'new' ? 'new' : 'unknown'};
}

function isScratchRecord(record: Record<string, unknown> | ChatRecord): boolean {
  return record.scratch === true && isScratchId(record.id) && isScratchId(record.workspaceId)
    && !record.execution && !record.nativeImport && !record.handoff && !record.lineage;
}

function validWorktreeReuse(record: Record<string, unknown> | ChatRecord): boolean {
  const owner = (record.execution as ChatExecution | undefined)?.ownerSessionId;
  const use = record.worktreeReuse as ChatRecord['worktreeReuse'];
  if (record.worktreeReleased !== undefined && (record.worktreeReleased !== true || !owner || record.agentId)) return false;
  if (owner === undefined && use === undefined) return true;
  return typeof owner === 'string' && /^[A-Za-z0-9_-]{1,100}$/.test(owner) && owner !== record.id
    && !!use && typeof use.sourceSessionId === 'string' && /^[A-Za-z0-9_-]{1,100}$/.test(use.sourceSessionId)
    && use.sourceSessionId !== record.id && typeof use.ownerFingerprint === 'string' && /^[a-f0-9]{64}$/.test(use.ownerFingerprint)
    && !record.scratch && !record.nativeImport && !record.lineage && !record.handoff && !record.prReview;
}

export function assertWorktreeUse(record: ChatRecord): void {
  if (record.worktreeReleased)
    throw new SessionIdentityError('WORKTREE_RELEASED', '此会话已解除共用工作树，历史和草稿仍可查看。');
}

function isChatRecord(value: unknown): value is ChatRecord {
  if (!value || typeof value !== 'object') return false;
  const record = value as Record<string, unknown>;
  if (record.deferFirstConnection !== undefined && record.deferFirstConnection !== true) return false;
  if (record.draftCopy !== undefined && (typeof record.draftCopy !== 'string' || !/^[a-f0-9]{64}$/.test(record.draftCopy))) return false;
  if (record.handoff !== undefined) {
    const h = record.handoff as Record<string,unknown>;
    if (!h || typeof h !== 'object' || !['operationId','snapshotId','snapshotHash','sourceSessionId'].every(key=>typeof h[key]==='string' && !!h[key])) return false;
  }
  if (typeof record.id !== 'string' || typeof record.workspaceId !== 'string' || typeof record.title !== 'string'
    || typeof record.updatedAt !== 'number' || (record.sessionFile !== null && typeof record.sessionFile !== 'string')) return false;
  if (record.inboxReply !== undefined) {
    const reply = record.inboxReply as ChatRecord['inboxReply'];
    if (!reply || !Number.isSafeInteger(reply.revision) || reply.revision < 1
      || typeof reply.messageId !== 'string' || !reply.messageId
      || reply.receivedAt !== undefined && (!Number.isSafeInteger(reply.receivedAt) || reply.receivedAt <= 0)) return false;
  }
  if (record.scratch !== undefined && !isScratchRecord(record)) return false;
  if (record.profile !== undefined && (!record.profile || typeof record.profile !== 'object' || typeof (record.profile as Record<string, unknown>).id !== 'string' || !(record.profile as Record<string, unknown>).id || !Number.isSafeInteger((record.profile as Record<string, unknown>).revision) || ((record.profile as Record<string, unknown>).revision as number) < 1)) return false;
  if (record.legacyConfiguration !== undefined && record.legacyConfiguration !== true) return false;
  if (record.nativeImport !== undefined) {
    const source = record.nativeImport as NativeImportSource | null;
    if (!source || ![source.home, source.path, source.cwd].every(path => typeof path === 'string' && isAbsolute(path))
      || !record.profile || record.execution
      || !(record.backend === 'codex' && record.codexThreadId || record.backend === 'claude' && record.nativeclaudeSessionId
        || record.backend === 'pi-rpc' && record.externalPiSessionId && record.sessionFile === source.path)) return false;
  }
  if (record.lineage !== undefined) {
    const lineage=record.lineage as ChatRecord['lineage'];
    if(!lineage||typeof lineage.operationId!=='string'||typeof lineage.parentSessionId!=='string'||typeof lineage.nativeUserItemId!=='string'||typeof lineage.edit!=='boolean'||!['current','checkpoint'].includes(lineage.codeRelation?.kind)||typeof lineage.codeRelation.checkpointId!=='string'||!Array.isArray(lineage.inheritedItemIds)||!lineage.inheritedItemIds.every(id=>typeof id==='string')) return false;
  }
  if(record.rewindOperationId!==undefined&&(typeof record.rewindOperationId!=='string'||!/^[a-f0-9]{8}-[a-f0-9]{4}-4[a-f0-9]{3}-[89ab][a-f0-9]{3}-[a-f0-9]{12}$/i.test(record.rewindOperationId)))return false;
  if(record.rewound!==undefined&&record.rewound!==true)return false;
  if (record.prReview !== undefined && !isGithubPrReviewSeed(record.prReview)) return false;
  if(record.forkDraft!==undefined){const draft=record.forkDraft as ChatRecord['forkDraft'];if(!draft||typeof draft.text!=='string'||![draft.attachmentIds,draft.missingAttachments].every(ids=>Array.isArray(ids)&&ids.every(id=>typeof id==='string')))return false;}
  if (record.pinned !== undefined && typeof record.pinned !== 'boolean') return false;
  if (record.pinRank !== undefined && (!Number.isSafeInteger(record.pinRank) || record.pinned !== true)) return false;
  if (record.archived !== undefined && typeof record.archived !== 'boolean') return false;
  if (record.execution !== undefined) {
    const execution = record.execution as Record<string, unknown> | null;
    if (!execution || typeof execution !== 'object' || Array.isArray(execution)
      || execution.kind !== 'worktree' || typeof execution.path !== 'string' || !execution.path
      || typeof execution.branch !== 'string' || !execution.branch) return false;
  }
  if (!validWorktreeReuse(record)) return false;
  if (record.lastDeliveryFailure !== undefined) {
    const failure=record.lastDeliveryFailure as Record<string,unknown>|null;
    if (!failure || typeof failure!=='object' || Array.isArray(failure) || typeof failure.deliveryId!=='string' || !failure.deliveryId || typeof failure.message!=='string') return false;
  }
  if (record.nativeMode !== undefined) {
    const mode = record.nativeMode as Record<string,unknown> | null;
    if (!mode || typeof mode !== 'object' || Array.isArray(mode) || mode.backend !== record.backend
      || !['codex', 'claude'].includes(mode.backend as string)
      || (mode.mode !== undefined && (typeof mode.mode !== 'string' || !mode.mode))
      || (mode.permissionMode !== undefined && (mode.backend !== 'codex' || typeof mode.permissionMode !== 'string' || !mode.permissionMode))
      || (mode.mode === undefined && mode.permissionMode === undefined)) return false;
  }
  if (record.reasoning !== undefined) {
    const setting = record.reasoning as Record<string,unknown> | null;
    if (!setting || typeof setting !== 'object' || Array.isArray(setting) || !['provider','modelId','level'].every(key=>typeof setting[key]==='string' && (setting[key] as string).length>0)) return false;
  }
  if (record.customTitle !== undefined && typeof record.customTitle !== 'boolean') return false;
  if (record.backend !== undefined && record.backend !== 'pi' && record.backend !== 'codex' && record.backend !== 'pi-rpc' && record.backend !== 'claude' && record.backend !== 'acp') return false;
  if (record.nativeclaudeSessionId !== undefined && typeof record.nativeclaudeSessionId !== 'string') return false;
  if (record.codexThreadId !== undefined && typeof record.codexThreadId !== 'string') return false;
  if (record.agentId !== undefined && typeof record.agentId !== 'string') return false;
  if (record.originAgentId !== undefined && (typeof record.originAgentId !== 'string' || !record.originAgentId)) return false;
  if (record.identityState !== undefined && !['new','known','unknown'].includes(record.identityState as string)) return false;
  if (record.identityState === 'known' && !record.originAgentId && !record.agentId) return false;
  if (record.acpSessionId !== undefined && typeof record.acpSessionId !== 'string') return false;
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

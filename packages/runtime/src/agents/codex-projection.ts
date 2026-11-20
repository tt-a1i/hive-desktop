import { randomUUID } from 'node:crypto';
import { readFileSync, renameSync, rmSync, writeFileSync } from 'node:fs';
import { codexMetadataFile, readCodexMetadata } from './codex-metadata.js';
import { codexDeliveryLookup, isCodexToolItem, normalizeCodexItem, type CodexItem, type CodexTurn } from './codex-history.js';
import { openIndexedHistory, savedHistoryFileVersion, visitSavedHistoryJson, type IndexedHistoryRow } from './saved-history-index.js';

export type CodexDisplayEntry = { turnId: string; item: CodexItem };
type Projection = { version: 1; entries: CodexDisplayEntry[]; completedTurns: string[]; completeHistory?: boolean; searchEntries?: CodexDisplayEntry[]; searchVersion?: number };

function isDisplayEntry(value: unknown): value is CodexDisplayEntry {
  const entry = value as CodexDisplayEntry | null | undefined;
  return typeof entry?.turnId === 'string' && typeof entry.item?.id === 'string' && typeof entry.item.type === 'string';
}

function hasCompleteHistory(entries: CodexDisplayEntry[], searchEntries: CodexDisplayEntry[]): boolean {
  if (entries.length !== searchEntries.length) return false;
  const itemIds = new Set<string>();
  return searchEntries.every((entry, index) => {
    if (!isDisplayEntry(entry) || !entry.turnId || !entry.item.id || itemIds.has(entry.item.id)
      || entry.item.status === 'inProgress' || entry.item.type === 'hiveProjectionNotice') return false;
    // Version 2 writers copy the display entries. Verify that the copy covers
    // the same ordered identities before claiming the visible history is complete.
    const visible = entries[index]!;
    if (visible.turnId !== entry.turnId || visible.item.id !== entry.item.id || visible.item.type !== entry.item.type
      || visible.item.status === 'inProgress') return false;
    itemIds.add(entry.item.id);
    return true;
  });
}

export function parseCodexProjection(raw: string): { entries: CodexDisplayEntry[]; completedTurns: string[]; completeHistory: boolean } {
  const saved = JSON.parse(raw) as Projection | null;
  if (saved?.version !== 1 || !Array.isArray(saved.entries) || !saved.entries.every(isDisplayEntry)) {
    throw new Error('Invalid Codex UI projection');
  }
  const completeHistory = saved.completeHistory === true && saved.searchVersion === 2 && Array.isArray(saved.searchEntries)
    && hasCompleteHistory(saved.entries, saved.searchEntries);
  return { entries: saved.entries, completedTurns: saved.completedTurns ?? [], completeHistory };
}

const DISPLAY_ARRAYS = new Set(['entries', 'searchEntries', 'completedTurns']);

/** The display journal and Hive delivery metadata are one read snapshot. */
export async function openSavedCodexProjection(directory: string, threadId: string, options: {
  cacheKey: string; sourceSessionId: string; signal?: AbortSignal; snapshot?: string;
}) {
  const metadataFile = codexMetadataFile(directory, threadId);
  const files = [`${metadataFile}.display.json`, metadataFile, `${metadataFile}.required`];
  const versions = await Promise.all(files.map(async (file, i) => {
    const optional = i > 0;
    return {file, optional, version: await savedHistoryFileVersion(file, optional)};
  }));
  const source = await openIndexedHistory({
    ...options,
    cacheKey: JSON.stringify(['codex-display', options.cacheKey, threadId, options.sourceSessionId]),
    versions,
    async build(handle) {
      const metadata = readCodexMetadata(directory, threadId);
      const deliveryFor = codexDeliveryLookup(metadata);
      const inheritedSources = new Map<string, string>();
      for (const entry of metadata.inheritedSources ?? []) {
        if (!inheritedSources.has(entry.itemId)) inheritedSources.set(entry.itemId, entry.sourceSessionId);
      }
      const focus = new Map<string, number>(), toolFocus = new Map<string, number>();
      const rows: IndexedHistoryRow[] = [];
      const visibleIdentities: (string | undefined)[] = [], searchIdentities: (string | undefined)[] = [];
      const searchedIds = new Set<string>();
      let version: unknown, searchVersion: unknown, completeHistory: unknown;
      let hasEntries = false, hasSearchEntries = false;
      await visitSavedHistoryJson(handle, DISPLAY_ARRAYS, (key, value, span, part) => {
        if (part === 'field') {
          if (key === 'version') version = value;
          if (key === 'searchVersion') searchVersion = value;
          if (key === 'completeHistory') completeHistory = value;
        } else if (part === 'array') {
          if (key === 'entries') hasEntries = true;
          if (key === 'searchEntries') hasSearchEntries = true;
        } else if (key === 'entries') {
          if (!isDisplayEntry(value)) throw new Error('Invalid Codex UI projection');
          const {turnId, item} = value;
          if (item.type === 'userMessage') {
            const alias = deliveryFor(turnId, item.id)?.deliveryId;
            if (alias !== undefined && !focus.has(alias)) focus.set(alias, rows.length);
          }
          rows.push({...span, id: item.id});
          visibleIdentities.push(item.status !== 'inProgress' ? JSON.stringify([turnId, item.id, item.type]) : undefined);
        } else if (key === 'searchEntries') {
          if (isDisplayEntry(value) && value.turnId && value.item.id && !searchedIds.has(value.item.id)
            && value.item.status !== 'inProgress' && value.item.type !== 'hiveProjectionNotice') {
            searchedIds.add(value.item.id);
            searchIdentities.push(JSON.stringify([value.turnId, value.item.id, value.item.type]));
          } else searchIdentities.push(undefined);
        }
      }, options.signal);
      if (version !== 1 || !hasEntries) throw new Error('Invalid Codex UI projection');
      const complete = completeHistory === true && searchVersion === 2 && hasSearchEntries
        && searchIdentities.length === rows.length
        && searchIdentities.every((identity, index) => identity !== undefined && identity === visibleIdentities[index]);
      return {rows, focus, toolFocus, complete, deliveryFor, inheritedSources, completed: metadata.completed};
    },
    project(value, index) {
      if (!isDisplayEntry(value)) throw new Error('Invalid Codex UI projection');
      const {item, turnId} = value;
      return normalizeCodexItem(item, index.deliveryFor(turnId, item.id), index.inheritedSources.get(item.id) ?? options.sourceSessionId);
    },
  });
  const {index, ...reader} = source;
  return {...reader, completeHistory: index.complete, completedDeliveryIds: index.completed};
}

/** A UI event journal only. Never supplied to thread/resume, turn/start, or a model. */
export class CodexProjection {
  private readonly file: string;
  private completedTurns: string[];
  private entries: CodexDisplayEntry[];
  private completeHistory = false;
  constructor(directory: string, threadId: string, options?: { requireSaved: boolean }) {
    this.file = `${codexMetadataFile(directory, threadId)}.display.json`;
    let saved: ReturnType<typeof parseCodexProjection>;
    try { saved = parseCodexProjection(readFileSync(this.file, 'utf8')); }
    catch (error) {
      if (options?.requireSaved || (error as NodeJS.ErrnoException).code !== 'ENOENT') throw error;
      saved = { entries: [], completedTurns: [], completeHistory: false };
    }
    this.entries = saved.entries;
    this.completeHistory = saved.completeHistory;
    this.completedTurns = saved.completedTurns;
  }
  current(): CodexDisplayEntry[] { return [...this.entries]; }
  hasCompleteHistory(): boolean { return this.completeHistory; }
  invalidateHistory(): void {
    if (!this.completeHistory) return;
    this.completeHistory = false;
    this.save(this.entries);
  }
  put(item: CodexItem, turnId: string): void {
    this.completeHistory = false;
    const next = [...this.entries];
    const index = next.findIndex(entry => entry.item.id === item.id && entry.turnId === turnId);
    if (index < 0) next.push({ item, turnId }); else next[index] = { item, turnId };
    this.save(next);
  }
  complete(turnId: string): void {
    this.save(this.entries, [...new Set([...this.completedTurns, turnId])]);
  }
  recover(turns: CodexTurn[], nativeItems = false): void {
    const next: CodexDisplayEntry[] = [];
    const turnIds = new Set<string>();
    const itemIds = new Set<string>();
    const nativeValid = nativeItems && turns.every(turn => {
      if (typeof turn.id !== 'string' || !turn.id || turnIds.has(turn.id) || !Array.isArray(turn.items) || turn.status === 'inProgress') return false;
      turnIds.add(turn.id);
      return turn.items.every(item => {
        if (typeof item.id !== 'string' || !item.id || typeof item.type !== 'string' || item.status === 'inProgress' || itemIds.has(item.id)) return false;
        itemIds.add(item.id);
        return true;
      });
    });
    this.completeHistory = nativeValid;
    const completed = new Set(this.completedTurns);
    // Cached-only reads preserve the existing marker list, including duplicates.
    let mergedCompletions = false;
    const byTurn = new Map<string, CodexDisplayEntry[]>();
    for (const entry of this.entries) {
      const group = byTurn.get(entry.turnId);
      if (group) group.push(entry); else byTurn.set(entry.turnId, [entry]);
    }
    for (const turn of turns) {
      const cached = byTurn.get(turn.id) ?? [];
      if (nativeItems) {
        // Native full pages define the current branch, order and message text.
        // A matching cached tool may contain richer event details that the
        // native item omits; never carry cached-only items into this branch.
        const cachedById = new Map(cached.map(entry => [entry.item.id, entry.item]));
        for (const item of turn.items) {
          const previous = cachedById.get(item.id);
          const merged = previous && previous.type === item.type && isCodexToolItem(item)
            ? { ...previous, ...item }
            : item;
          if (merged !== item && item.status !== 'inProgress' && !Object.hasOwn(item, 'hiveInterrupted')) {
            delete merged.hiveInterrupted;
          }
          next.push({ turnId: turn.id, item: merged });
        }
        if (cached.length > 0 && !completed.has(turn.id) && turn.status !== 'inProgress') {
          next.push({ turnId: turn.id, item: { id: `${turn.id}:display-gap`, type: 'hiveProjectionNotice',
            text: 'This turn ended before its display journal was finalized. Some message or tool details may be missing.' } });
        } else if (turn.status !== 'inProgress' && !completed.has(turn.id)) {
          completed.add(turn.id);
          mergedCompletions = true;
        }
      } else if (cached.length > 0) {
        next.push(...cached);
        if (!completed.has(turn.id) && turn.status !== 'inProgress'
          && !cached.some(entry => entry.item.id === `${turn.id}:display-gap`)) {
          next.push({ turnId: turn.id, item: { id: `${turn.id}:display-gap`, type: 'hiveProjectionNotice',
            text: 'This turn ended before its display journal was finalized. Some message or tool details may be missing.' } });
        }
      } else {
        // Explicit legacy import, never claim its synthetic IDs match live items.
        next.push(...turn.items.map(item => ({ turnId: turn.id, item: { ...item, id: `${turn.id}:legacy:${item.id}` } })));
        next.push({ turnId: turn.id, item: { id: `${turn.id}:legacy-import`, type: 'hiveProjectionNotice',
          text: 'Imported legacy Codex history. Tool and reasoning details are unavailable for this turn.' } });
        completed.add(turn.id);
        mergedCompletions = true;
      }
    }
    // A display gap or legacy import is evidence of an incomplete journal,
    // even when this fetch returned native items for the same turn.
    if (next.some(entry => entry.item.type === 'hiveProjectionNotice')) this.completeHistory = false;
    const terminal = new Set(turns.filter(turn => turn.status === 'interrupted' || turn.status === 'failed').map(turn => turn.id));
    this.save(next.map(entry => terminal.has(entry.turnId) && entry.item.status === 'inProgress'
      ? { ...entry, item: { ...entry.item, hiveInterrupted: true } } : entry), mergedCompletions ? [...completed] : this.completedTurns);
  }
  private save(entries: CodexDisplayEntry[], completedTurns = this.completedTurns): void {
    const temporary = `${this.file}.${randomUUID()}.tmp`;
    try {
      writeFileSync(temporary, JSON.stringify({ version: 1, entries, completedTurns, ...(this.completeHistory ? { completeHistory: true, searchVersion: 2, searchEntries: entries } : {}) }), { mode: 0o600, flush: true });
      renameSync(temporary, this.file);
      this.entries = entries;
      this.completedTurns = completedTurns;
    } finally {
      rmSync(temporary, { force: true });
    }
  }
}

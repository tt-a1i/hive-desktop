import { randomUUID } from 'node:crypto';
import { readFileSync, renameSync, rmSync, writeFileSync } from 'node:fs';
import { codexMetadataFile } from './codex-metadata.js';
import { isCodexToolItem, type CodexItem, type CodexTurn } from './codex-history.js';

export type CodexDisplayEntry = { turnId: string; item: CodexItem };
type Projection = { version: 1; entries: CodexDisplayEntry[]; completedTurns: string[]; completeHistory?: boolean; searchEntries?: CodexDisplayEntry[] };
/** A UI event journal only. Never supplied to thread/resume, turn/start, or a model. */
export class CodexProjection {
  private readonly file: string;
  private completedTurns: string[];
  private entries: CodexDisplayEntry[];
  private completeHistory = false;
  private searchEntries: CodexDisplayEntry[] | undefined;
  constructor(directory: string, threadId: string) {
    this.file = `${codexMetadataFile(directory, threadId)}.display.json`;
    let saved: Projection;
    try { saved = JSON.parse(readFileSync(this.file, 'utf8')); }
    catch (error) {
      if ((error as NodeJS.ErrnoException).code !== 'ENOENT') throw error;
      saved = { version: 1, entries: [], completedTurns: [] };
    }
    if (saved.version !== 1 || !Array.isArray(saved.entries)
      || !saved.entries.every(entry => typeof entry.turnId === 'string' && typeof entry.item?.id === 'string' && typeof entry.item.type === 'string')) {
      throw new Error('Invalid Codex UI projection');
    }
    this.entries = saved.entries;
    this.completeHistory = saved.completeHistory === true && Array.isArray(saved.searchEntries)
      && saved.searchEntries.every(entry => typeof entry.turnId === 'string' && typeof entry.item?.id === 'string' && typeof entry.item.type === 'string');
    this.searchEntries = this.completeHistory ? saved.searchEntries : undefined;
    this.completedTurns = saved.completedTurns ?? [];
  }
  current(): CodexDisplayEntry[] { return [...this.entries]; }
  invalidateHistory(): void {
    if (!this.completeHistory) return;
    this.completeHistory = false;
    this.searchEntries = undefined;
    this.save(this.entries);
  }
  put(item: CodexItem, turnId: string): void {
    this.completeHistory = false;
    this.searchEntries = undefined;
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
    this.searchEntries = nativeValid ? turns.flatMap(turn => turn.items.map(item => ({turnId: turn.id, item}))) : undefined;
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
    if (next.some(entry => entry.item.type === 'hiveProjectionNotice')) {
      this.completeHistory = false;
      this.searchEntries = undefined;
    }
    const terminal = new Set(turns.filter(turn => turn.status === 'interrupted' || turn.status === 'failed').map(turn => turn.id));
    this.save(next.map(entry => terminal.has(entry.turnId) && entry.item.status === 'inProgress'
      ? { ...entry, item: { ...entry.item, hiveInterrupted: true } } : entry), mergedCompletions ? [...completed] : this.completedTurns);
  }
  private save(entries: CodexDisplayEntry[], completedTurns = this.completedTurns): void {
    const temporary = `${this.file}.${randomUUID()}.tmp`;
    try {
      writeFileSync(temporary, JSON.stringify({ version: 1, entries, completedTurns, ...(this.completeHistory && this.searchEntries ? { completeHistory: true, searchEntries: this.searchEntries } : {}) }), { mode: 0o600 });
      renameSync(temporary, this.file);
      this.entries = entries;
      this.completedTurns = completedTurns;
    } finally {
      rmSync(temporary, { force: true });
    }
  }
}

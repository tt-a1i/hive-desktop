import { randomUUID } from 'node:crypto';
import { readFileSync, renameSync, writeFileSync } from 'node:fs';
import { codexMetadataFile } from './codex-metadata.js';
import type { CodexItem, CodexTurn } from './codex-history.js';

export type CodexDisplayEntry = { turnId: string; item: CodexItem };
type Projection = { version: 1; entries: CodexDisplayEntry[]; completedTurns: string[] };
/** A UI event journal only. Never supplied to thread/resume, turn/start, or a model. */
export class CodexProjection {
  private readonly file: string;
  private completedTurns: string[];
  private entries: CodexDisplayEntry[];
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
    this.completedTurns = saved.completedTurns ?? [];
  }
  current(): CodexDisplayEntry[] { return [...this.entries]; }
  put(item: CodexItem, turnId: string): void {
    const next = [...this.entries];
    const index = next.findIndex(entry => entry.item.id === item.id && entry.turnId === turnId);
    if (index < 0) next.push({ item, turnId }); else next[index] = { item, turnId };
    this.save(next);
  }
  complete(turnId: string): void {
    const previous = this.completedTurns;
    this.completedTurns = [...new Set([...previous, turnId])];
    try { this.save(this.entries); } catch (error) { this.completedTurns = previous; throw error; }
  }
  recover(turns: CodexTurn[], nativeItems = false): void {
    const next: CodexDisplayEntry[] = [];
    for (const turn of turns) {
      const cached = this.entries.filter(entry => entry.turnId === turn.id);
      if (cached.length > 0) {
        next.push(...cached);
        if (!this.completedTurns.includes(turn.id) && turn.status !== 'inProgress'
          && !cached.some(entry => entry.item.id === `${turn.id}:display-gap`)) {
          next.push({ turnId: turn.id, item: { id: `${turn.id}:display-gap`, type: 'hiveProjectionNotice',
            text: 'This turn ended before its display journal was finalized. Some message or tool details may be missing.' } });
        }
      } else if (nativeItems) {
        next.push(...turn.items.map(item => ({turnId: turn.id, item})));
        if (turn.status !== 'inProgress') this.completedTurns = [...new Set([...this.completedTurns, turn.id])];
      } else {
        // Explicit legacy import, never claim its synthetic IDs match live items.
        next.push(...turn.items.map(item => ({ turnId: turn.id, item: { ...item, id: `${turn.id}:legacy:${item.id}` } })));
        next.push({ turnId: turn.id, item: { id: `${turn.id}:legacy-import`, type: 'hiveProjectionNotice',
          text: 'Imported legacy Codex history. Tool and reasoning details are unavailable for this turn.' } });
        this.completedTurns = [...new Set([...this.completedTurns, turn.id])];
      }
    }
    const terminal = new Set(turns.filter(turn => turn.status === 'interrupted' || turn.status === 'failed').map(turn => turn.id));
    this.save(next.map(entry => terminal.has(entry.turnId) && entry.item.status === 'inProgress'
      ? { ...entry, item: { ...entry.item, hiveInterrupted: true } } : entry));
  }
  private save(entries: CodexDisplayEntry[]): void {
    const temporary = `${this.file}.${randomUUID()}.tmp`;
    writeFileSync(temporary, JSON.stringify({ version: 1, entries, completedTurns: this.completedTurns }), { mode: 0o600 });
    renameSync(temporary, this.file);
    this.entries = entries;
  }
}

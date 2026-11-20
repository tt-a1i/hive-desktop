import { mkdir, readFile, open, rename, rm } from 'node:fs/promises';
import { dirname } from 'node:path';
import { randomUUID } from 'node:crypto';
import { historyRowFocusId, historyToolCallFocusIds } from '../history-cursor.js';
import { object } from './acp-protocol.js';
import { openIndexedHistory, savedHistoryFileVersion, visitSavedHistoryJson, type IndexedHistoryRow } from './saved-history-index.js';
export interface AcpHistoryState {
    version: 1;
    sessionId: string;
    history: unknown[];
    historyIds: string[];
    contextTypes?: string[];
    contexts: {
        text: string;
        customType: string;
    }[];
    inputs?: {
        id: string;
        text: string;
        nativeText?: string;
        presentation?: unknown;
    }[];
    deliveries: Record<string, 'sent' | 'completed'>;
}

const HISTORY_ARRAYS = new Set(['history', 'historyIds', 'contexts', 'contextTypes', 'inputs']);

function validContext(value: unknown): boolean {
    return object(value) && typeof value.text === 'string' && typeof value.customType === 'string';
}

function validDeliveries(value: unknown): value is AcpHistoryState['deliveries'] {
    return object(value) && Object.values(value).every(v => v === 'sent' || v === 'completed');
}

/** Read the saved ACP transcript without restoring the native ACP session. */
export async function openSavedAcpHistory(file: string, expectedSessionId: string, options: {
    cacheKey: string; signal?: AbortSignal; snapshot?: string;
}) {
    const source = await openIndexedHistory({
        ...options,
        cacheKey: JSON.stringify(['acp', options.cacheKey, expectedSessionId]),
        versions: [{file, version: await savedHistoryFileVersion(file)}],
        async build(handle) {
            const rows: IndexedHistoryRow[] = [];
            const ids: string[] = [];
            const arrays = new Set<string>();
            const focus = new Map<string, number>(), toolFocus = new Map<string, number>();
            const invalid = () => new Error('Invalid ACP transcript');
            let version: unknown, sessionId: unknown, deliveries: unknown;
            await visitSavedHistoryJson(handle, HISTORY_ARRAYS, (key, value, span, part) => {
                if (part === 'array') { arrays.add(key); return; }
                if (part === 'field') {
                    if (key === 'version') version = value;
                    if (key === 'sessionId') {
                        sessionId = value;
                        if (sessionId !== expectedSessionId) throw new Error('ACP native session identity does not match its local transcript');
                    }
                    if (key === 'deliveries') deliveries = value;
                    return;
                }
                if (key === 'historyIds') {
                    if (typeof value !== 'string') throw invalid();
                    ids.push(value);
                } else if (key === 'history') {
                    const alias = historyRowFocusId(value);
                    if (alias !== undefined && !focus.has(alias)) focus.set(alias, rows.length);
                    for (const tool of historyToolCallFocusIds(value)) if (!toolFocus.has(tool)) toolFocus.set(tool, rows.length);
                    rows.push({...span, id: ''});
                } else if (key === 'contexts' && !validContext(value)) throw invalid();
            }, options.signal);
            if (version !== 1 || typeof sessionId !== 'string' || !arrays.has('history') || !arrays.has('historyIds')
                || !arrays.has('contexts') || rows.length !== ids.length || !validDeliveries(deliveries)) throw invalid();
            for (let i = 0; i < rows.length; i++) rows[i]!.id = ids[i]!;
            return {rows, focus, toolFocus, deliveries};
        },
        project: value => value,
    });
    const {index, ...reader} = source;
    return {...reader, deliveries: index.deliveries};
}

/** Each append is a complete fsync+atomic replacement. Incomplete deliveries are never automatically replayed. */
export class AcpHistory {
    private tail: Promise<void> = Promise.resolve();
    failed: Error | undefined;
    constructor(readonly file: string, public state: AcpHistoryState) { }
    static async load(file: string, signal?: AbortSignal): Promise<AcpHistory | undefined> {
        let raw: string;
        try {
            raw = await readFile(file, {encoding: 'utf8', ...(signal && {signal})});
        }
        catch (error) {
            if ((error as NodeJS.ErrnoException).code === 'ENOENT')
                return;
            throw error;
        }
        signal?.throwIfAborted();
        const state: unknown = JSON.parse(raw);
        if (!object(state) || state.version !== 1 || typeof state.sessionId !== 'string' || !Array.isArray(state.history) || !Array.isArray(state.historyIds) || state.historyIds.length !== state.history.length || !state.historyIds.every(id => typeof id === 'string') || !Array.isArray(state.contexts) || !state.contexts.every(validContext) || !validDeliveries(state.deliveries))
            throw new Error('Invalid ACP transcript');
        return new AcpHistory(file, state as unknown as AcpHistoryState);
    }
    write(change: (state: AcpHistoryState) => void): Promise<void> {
        const task = this.tail.then(async () => {
            if (this.failed)
                throw this.failed;
            const next = structuredClone(this.state);
            change(next);
            await mkdir(dirname(this.file), { recursive: true });
            const temporary = `${this.file}.${randomUUID()}.tmp`;
            try {
                const file = await open(temporary, 'wx', 0o600);
                try {
                    await file.writeFile(JSON.stringify(next));
                    await file.sync();
                }
                finally {
                    await file.close();
                }
                await rename(temporary, this.file);
                this.state = next;
            }
            finally {
                await rm(temporary, { force: true });
            }
        });
        this.tail = task.catch(error => { this.failed = error instanceof Error ? error : new Error(String(error)); });
        return task;
    }
}

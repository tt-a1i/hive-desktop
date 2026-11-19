import { mkdir, readFile, open, rename, rm } from 'node:fs/promises';
import { dirname } from 'node:path';
import { randomUUID } from 'node:crypto';
import { object } from './acp-protocol.js';
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
        if (!object(state) || state.version !== 1 || typeof state.sessionId !== 'string' || !Array.isArray(state.history) || !Array.isArray(state.historyIds) || state.historyIds.length !== state.history.length || !state.historyIds.every(id => typeof id === 'string') || !Array.isArray(state.contexts) || !state.contexts.every(c => object(c) && typeof c.text === 'string' && typeof c.customType === 'string') || !object(state.deliveries) || !Object.values(state.deliveries).every(v => v === 'sent' || v === 'completed'))
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

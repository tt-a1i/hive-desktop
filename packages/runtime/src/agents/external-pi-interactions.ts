import { randomUUID } from 'node:crypto';
import { CodexInteractionError } from './codex-interactions.js';
import type { ExternalPiRecord } from './external-pi-transport.js';
import type { ExternalPiInteraction, InteractionResolutionReason, PiEvent } from './types.js';

type Pending = { value: ExternalPiInteraction; write(response: Record<string, unknown>): Promise<void>; timer?: NodeJS.Timeout };
const object = (value: unknown): value is Record<string, unknown> => !!value && typeof value === 'object' && !Array.isArray(value);
/** Live native callbacks only: answers never enter snapshots, events or history. */
export class ExternalPiInteractions {
  private readonly pending = new Map<string, Pending>();
  constructor(private readonly sessionId: () => string, private readonly emit: (event: PiEvent) => void,
    private readonly expireNative: (id: string) => void) {}
  snapshot(): ExternalPiInteraction[] { return [...this.pending.values()].map(entry => structuredClone(entry.value)); }
  request(params: ExternalPiRecord, write: Pending['write']): void {
    const invalid = () => new CodexInteractionError('INTERACTION_INVALID_REQUEST', 'Invalid native Pi dialog');
    if (typeof params.id !== 'string' || !['select', 'confirm', 'input', 'editor'].includes(String(params.method)) || typeof params.title !== 'string'
      || (params.method === 'select' && (!Array.isArray(params.options) || !params.options.every(option => typeof option === 'string')))
      || (params.method === 'confirm' && typeof params.message !== 'string')
      || (params.timeout !== undefined && (typeof params.timeout !== 'number' || !Number.isFinite(params.timeout) || params.timeout < 0 || params.timeout > 2147483647))
      || [...this.pending.values()].some(entry => entry.value.nativeRequestId === params.id)) throw invalid();
    const expiresAt = typeof params.timeout === 'number' && params.timeout > 0 ? Date.now() + params.timeout : undefined;
    const value: ExternalPiInteraction = { id: randomUUID(), backend: 'pi-rpc', method: params.method as ExternalPiInteraction['method'],
      nativeRequestId: params.id, state: 'pending', params: structuredClone(params), ...(expiresAt !== undefined && { expiresAt }) };
    const entry: Pending = { value, write };
    this.pending.set(value.id, entry);
    if (expiresAt !== undefined) {
      entry.timer = setTimeout(() => {
        this.expireNative(value.nativeRequestId); this.remove(entry, 'expired');
      }, Math.max(0, expiresAt - Date.now()));
      entry.timer.unref?.();
    }
    this.emit({ type: 'interaction', sessionId: this.sessionId(), interaction: structuredClone(value) });
  }
  async respond(id: string, response: unknown): Promise<void> {
    const entry = this.pending.get(id);
    if (!entry || entry.value.state !== 'pending' || (entry.value.expiresAt !== undefined && Date.now() >= entry.value.expiresAt)) {
      throw new CodexInteractionError('INTERACTION_STALE', 'This interaction is no longer awaiting a response');
    }
    if (!object(response) || Object.keys(response).length !== 1 || !(response.cancelled === true
      || (entry.value.method === 'confirm' ? typeof response.confirmed === 'boolean'
        : typeof response.value === 'string' && (entry.value.method !== 'select' || (entry.value.params.options as string[]).includes(response.value))))) {
      throw new CodexInteractionError('INTERACTION_INVALID_RESPONSE', 'Response does not match this native Pi dialog');
    }
    entry.value.state = 'responding'; clearTimeout(entry.timer);
    this.emit({ type: 'interaction', sessionId: this.sessionId(), interaction: structuredClone(entry.value) });
    await entry.write(structuredClone(response));
    if (this.pending.get(id) === entry) this.remove(entry, 'resolved');
  }
  async cancel(reason: InteractionResolutionReason): Promise<void> {
    const entries = [...this.pending.values()];
    // Invalidate cards before asynchronous writes; native cancellation releases awaited command callbacks.
    for (const entry of entries) this.remove(entry, reason);
    const results = await Promise.allSettled(entries.filter(entry => entry.value.state === 'pending').map(entry => entry.write({ cancelled: true })));
    const errors = results.flatMap(result => result.status === 'rejected' ? [result.reason] : []);
    if (errors.length === 1) throw errors[0];
    if (errors.length > 1) throw new AggregateError(errors, 'Failed to cancel external Pi dialogs');
  }
  clear(reason: InteractionResolutionReason): void { for (const entry of this.pending.values()) this.remove(entry, reason); }
  private remove(entry: Pending, reason: InteractionResolutionReason): void {
    clearTimeout(entry.timer); this.pending.delete(entry.value.id);
    this.emit({ type: 'interaction_resolved', sessionId: this.sessionId(), interactionId: entry.value.id, reason });
  }
}

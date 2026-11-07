import { randomUUID } from 'node:crypto';
import { CodexRequestCancelled, type CodexServerRequest, type CodexRequestId } from './codex-transport.js';
import type { CodexInteraction, InteractionResolutionReason, PiEvent } from './types.js';

export class CodexInteractionError extends Error {
  constructor(readonly code: 'INTERACTION_STALE' | 'INTERACTION_INVALID_RESPONSE' | 'INTERACTION_INVALID_REQUEST', message: string) { super(message); this.name = 'CodexInteractionError'; }
}
type Pending = { value: CodexInteraction; resolve(value: unknown): void; reject(error: Error): void };
const methods = new Set(['item/commandExecution/requestApproval', 'item/fileChange/requestApproval', 'item/tool/requestUserInput']);
const object = (value: unknown): value is Record<string, unknown> => !!value && typeof value === 'object' && !Array.isArray(value);
const invalid = () => new CodexInteractionError('INTERACTION_INVALID_RESPONSE', 'Response does not match this native interaction');

/** Ephemeral JSON-RPC callbacks, never an approval ledger or a model transcript. */
export class CodexInteractions {
  private readonly pending = new Map<string, Pending>();
  private readonly items = new Map<string, unknown>();
  constructor(private readonly sessionId: () => string, private readonly emit: (event: PiEvent) => void,
    private readonly cancelNative: (id: CodexRequestId) => void) {}
  supports(method: string): boolean { return methods.has(method); }
  snapshot(): CodexInteraction[] { return [...this.pending.values()].map(entry => structuredClone(entry.value)); }
  request(request: CodexServerRequest): Promise<unknown> {
    const params = request.params;
    if (!this.supports(request.method) || !object(params) || !['threadId', 'turnId', 'itemId'].every(key => typeof params[key] === 'string')
      || params.threadId !== this.sessionId()) throw new CodexInteractionError('INTERACTION_INVALID_REQUEST', 'Invalid native interaction ownership');
    if (request.method === 'item/tool/requestUserInput') {
      if (typeof params.isBlocking !== 'boolean' || !Array.isArray(params.questions) || !params.questions.every(question => object(question)
        && ['id', 'header', 'question'].every(key => typeof question[key] === 'string')
        && (question.isSecret === undefined || typeof question.isSecret === 'boolean') && (question.isOther === undefined || typeof question.isOther === 'boolean')
        && (question.options == null || (Array.isArray(question.options) && question.options.every(option => object(option) && typeof option.label === 'string' && typeof option.description === 'string'))))) {
        throw new CodexInteractionError('INTERACTION_INVALID_REQUEST', 'Invalid native user-input request');
      }
      if (new Set(params.questions.map(question => (question as Record<string, unknown>).id)).size !== params.questions.length) throw new CodexInteractionError('INTERACTION_INVALID_REQUEST', 'Duplicate native question identifiers');
    } else if (typeof params.startedAtMs !== 'number' || !Number.isInteger(params.startedAtMs)
      || (params.availableDecisions != null && !Array.isArray(params.availableDecisions))) {
      throw new CodexInteractionError('INTERACTION_INVALID_REQUEST', 'Invalid native approval request');
    }
    if ([...this.pending.values()].some(entry => entry.value.nativeRequestId === request.id)) throw new CodexInteractionError('INTERACTION_INVALID_REQUEST', 'Duplicate native request identifier');
    const id = randomUUID();
    const item = this.items.get(`${params.turnId}:${params.itemId}`);
    const value: CodexInteraction = { id, backend: 'codex', method: request.method as CodexInteraction['method'],
      threadId: params.threadId as string, turnId: params.turnId as string, itemId: params.itemId as string,
      nativeRequestId: request.id, state: 'pending', params: structuredClone(params), ...(item !== undefined && { item: structuredClone(item) }) };
    const answer = new Promise<unknown>((resolve, reject) => this.pending.set(id, { value, resolve, reject }));
    this.emit({ type: 'interaction', sessionId: this.sessionId(), interaction: structuredClone(value) });
    return answer;
  }
  item(turnId: string, item: unknown): void {
    if (!object(item) || typeof item.id !== 'string') return;
    this.items.set(`${turnId}:${item.id}`, structuredClone(item));
    for (const entry of this.pending.values()) if (entry.value.turnId === turnId && entry.value.itemId === item.id) {
      entry.value.item = structuredClone(item);
      this.emit({ type: 'interaction', sessionId: this.sessionId(), interaction: structuredClone(entry.value) });
    }
  }
  respond(id: string, response: unknown): void {
    const entry = this.pending.get(id);
    if (!entry || entry.value.state !== 'pending') throw new CodexInteractionError('INTERACTION_STALE', 'This interaction is no longer awaiting a response');
    if (!object(response)) throw invalid();
    if (entry.value.method === 'item/tool/requestUserInput') {
      if (Object.keys(response).length !== 1 || !object(response.answers)) throw invalid();
      const ids = new Set((entry.value.params.questions as { id: string }[]).map(question => question.id));
      for (const [key, answer] of Object.entries(response.answers)) {
        if (!ids.has(key) || !object(answer) || Object.keys(answer).length !== 1 || !Array.isArray(answer.answers) || !answer.answers.every(value => typeof value === 'string')) throw invalid();
      }
    } else {
      if (Object.keys(response).length !== 1 || !['accept', 'decline', 'cancel'].includes(response.decision as string)) throw invalid();
      const allowed = entry.value.params.availableDecisions;
      if (Array.isArray(allowed) && !allowed.includes(response.decision)) throw invalid();
      if (entry.value.method === 'item/fileChange/requestApproval' && response.decision === 'accept'
        && (!object(entry.value.item) || !Array.isArray(entry.value.item.changes))) throw new CodexInteractionError('INTERACTION_INVALID_RESPONSE', 'Native file changes have not been received');
    }
    entry.value.state = 'responding';
    this.emit({ type: 'interaction', sessionId: this.sessionId(), interaction: structuredClone(entry.value) });
    // Answers, especially isSecret values, only flow into the pending native RPC.
    entry.resolve(structuredClone(response));
  }
  resolved(nativeRequestId: CodexRequestId): void {
    for (const [id, entry] of this.pending) if (entry.value.nativeRequestId === nativeRequestId) this.remove(id, entry, 'resolved');
  }
  endTurn(turnId: string, reason: InteractionResolutionReason = 'turn-ended'): void {
    for (const [id, entry] of this.pending) if (entry.value.turnId === turnId) this.remove(id, entry, reason);
    for (const key of this.items.keys()) if (key.startsWith(`${turnId}:`)) this.items.delete(key);
  }
  clear(reason: InteractionResolutionReason): void {
    for (const [id, entry] of this.pending) this.remove(id, entry, reason);
    this.items.clear();
  }
  private remove(id: string, entry: Pending, reason: InteractionResolutionReason): void {
    this.cancelNative(entry.value.nativeRequestId);
    this.pending.delete(id);
    entry.reject(new CodexRequestCancelled());
    this.emit({ type: 'interaction_resolved', sessionId: this.sessionId(), interactionId: id, reason });
  }
}

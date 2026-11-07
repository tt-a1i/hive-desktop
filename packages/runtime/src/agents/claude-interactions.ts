import { randomUUID } from 'node:crypto';
import type { CanUseTool, PermissionResult } from '@anthropic-ai/claude-agent-sdk';
import { CodexInteractionError } from './codex-interactions.js';
import type { ClaudeInteraction, InteractionResolutionReason, PiEvent } from './types.js';

type Run = { id: string; signal: AbortSignal };
type NativeOptions = Parameters<CanUseTool>[2];
type Question = { question: string; options: { label: string }[]; multiSelect: boolean };
type Pending = { value: ClaudeInteraction; input: Record<string, unknown>; run: Run; signal: AbortSignal;
  resolve(result: PermissionResult): void; cleanup(): void };
const object = (value: unknown): value is Record<string, unknown> => !!value && typeof value === 'object' && !Array.isArray(value);
const denied = (message = 'Claude interaction was cancelled', interrupt = true): PermissionResult => ({ behavior: 'deny', message, ...(interrupt && { interrupt: true }) });
const stale = () => new CodexInteractionError('INTERACTION_STALE', 'This interaction is no longer awaiting a response');
const invalid = () => new CodexInteractionError('INTERACTION_INVALID_RESPONSE', 'Response does not match this native Claude interaction');

/** Live SDK callbacks only. Permission decisions never update persistent policy. */
export class ClaudeInteractions {
  private readonly pending = new Map<string, Pending>();
  constructor(private readonly sessionId: string, private readonly currentRun: () => Run | undefined,
    private readonly emit: (event: PiEvent) => void) {}
  snapshot(): ClaudeInteraction[] { return [...this.pending.values()].map(entry => structuredClone(entry.value)); }
  async request(toolName: string, input: Record<string, unknown>, options: NativeOptions): Promise<PermissionResult> {
    const run = this.currentRun();
    if (!run || run.signal.aborted || options.signal.aborted) return denied();
    if (!toolName || !object(input) || typeof options.requestId !== 'string' || !options.requestId
      || typeof options.toolUseID !== 'string' || !options.toolUseID || (toolName === 'AskUserQuestion' && !validQuestions(input.questions))) {
      return denied('Invalid native Claude interaction');
    }
    const duplicate = [...this.pending.values()].find(entry => entry.value.nativeRequestId === options.requestId);
    if (duplicate) { this.finish(duplicate, denied('Duplicate native Claude interaction'), 'aborted'); return denied('Duplicate native Claude interaction'); }
    const { signal, requestId, toolUseID, ...hints } = options;
    const originalInput = structuredClone(input);
    const value: ClaudeInteraction = { id: randomUUID(), backend: 'claude', method: toolName === 'AskUserQuestion' ? 'ask-user-question' : 'tool-permission',
      nativeRequestId: requestId, turnId: run.id, itemId: toolUseID, state: 'pending', params: { toolName, input: originalInput, ...structuredClone(hints) } };
    const answer = new Promise<PermissionResult>(resolve => {
      const cancel = () => this.finish(entry, denied(), 'aborted');
      const entry: Pending = { value, input: originalInput, run, signal, resolve,
        cleanup() { signal.removeEventListener('abort', cancel); run.signal.removeEventListener('abort', cancel); } };
      this.pending.set(value.id, entry);
      signal.addEventListener('abort', cancel, { once: true }); run.signal.addEventListener('abort', cancel, { once: true });
      this.emit({ type: 'interaction', sessionId: this.sessionId, interaction: structuredClone(value) });
    });
    const result = await answer;
    // Abort may win after a click but before the SDK resumes the native callback.
    return signal.aborted || run.signal.aborted || this.currentRun()?.id !== run.id ? denied() : result;
  }
  respond(id: string, response: unknown): void {
    const entry = this.pending.get(id);
    if (!entry || entry.value.state !== 'pending' || entry.signal.aborted || entry.run.signal.aborted || this.currentRun()?.id !== entry.run.id) throw stale();
    if (!object(response) || Object.keys(response).length !== 1) throw invalid();
    let result: PermissionResult;
    if (entry.value.method === 'ask-user-question' && response.decision !== 'cancel') {
      const questions = entry.input.questions as Question[];
      if (!object(response.answers) || Object.keys(response.answers).length !== questions.length) throw invalid();
      const answers: [string, string][] = [];
      for (const question of questions) {
        if (!Object.hasOwn(response.answers, question.question)) throw invalid();
        const values = response.answers[question.question];
        const labels = new Set(question.options.map(option => option.label));
        if (!Array.isArray(values) || !values.length || values.some(value => typeof value !== 'string' || !value.trim())
          || new Set(values).size !== values.length || (!question.multiSelect && values.length !== 1)
          || values.length > labels.size + 1 || values.filter(value => !labels.has(value as string)).length > 1) throw invalid();
        answers.push([question.question, values.join(', ')]);
      }
      result = { behavior: 'allow', updatedInput: { ...structuredClone(entry.input), answers: Object.fromEntries(answers) } };
    } else {
      if (!['accept', 'decline', 'cancel'].includes(response.decision as string)
        || (entry.value.method === 'ask-user-question' && response.decision !== 'cancel')) throw invalid();
      result = response.decision === 'accept' ? { behavior: 'allow', updatedInput: structuredClone(entry.input) }
        : denied(response.decision === 'cancel' ? 'User cancelled the Claude interaction' : 'User declined this tool', response.decision === 'cancel');
    }
    entry.value.state = 'responding';
    this.emit({ type: 'interaction', sessionId: this.sessionId, interaction: structuredClone(entry.value) });
    if (this.pending.get(id) !== entry) throw stale();
    this.finish(entry, result, 'resolved');
  }
  clear(reason: InteractionResolutionReason, runId?: string): void {
    for (const entry of this.pending.values()) if (runId === undefined || entry.run.id === runId) this.finish(entry, denied(), reason);
  }
  private finish(entry: Pending, result: PermissionResult, reason: InteractionResolutionReason): void {
    if (this.pending.get(entry.value.id) !== entry) return;
    this.pending.delete(entry.value.id); entry.cleanup(); entry.resolve(result);
    this.emit({ type: 'interaction_resolved', sessionId: this.sessionId, interactionId: entry.value.id, reason });
  }
}
function validQuestions(value: unknown): value is Question[] {
  if (!Array.isArray(value) || value.length < 1 || value.length > 4) return false;
  if (!value.every(question => object(question) && typeof question.question === 'string' && question.question.trim()
    && typeof question.header === 'string' && typeof question.multiSelect === 'boolean'
    && Array.isArray(question.options) && question.options.length >= 2 && question.options.length <= 4
    && question.options.every(option => object(option) && typeof option.label === 'string' && option.label.trim() && typeof option.description === 'string'
      && (option.preview === undefined || typeof option.preview === 'string'))
    && new Set(question.options.map(option => (option as { label: string }).label)).size === question.options.length)) return false;
  return new Set(value.map(question => (question as Question).question)).size === value.length;
}

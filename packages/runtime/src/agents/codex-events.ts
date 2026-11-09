import type { PiEvent } from './types.js';
import { codexToolInput, isCodexToolItem, normalizeCodexItem, toolName, type CodexDelivery, type CodexItem } from './codex-history.js';

export class CodexEvents {
  private readonly items = new Map<string, { item: CodexItem; turnId: string }>();
  private readonly tools = new Map<string, { item: CodexItem; turnId: string }>();
  private readonly running = new Map<string, { messageId: string; message: unknown }>();
  constructor(private readonly sessionId: string, private readonly emit: (event: PiEvent) => void,
    private readonly delivery: (turnId: string, itemId: string) => CodexDelivery | undefined) {}

  restore(items: { item: CodexItem; turnId: string }[]): void {
    this.items.clear();
    this.running.clear();
    this.tools.clear();
    for (const entry of items) {
      this.items.set(entry.item.id, entry);
      if (entry.item.status === 'inProgress' && entry.item.hiveInterrupted !== true) this.tools.set(entry.item.id, entry);
    }
  }
  snapshot() {
    const entries = [...this.items.values()];
    const activeMessage = [...this.running.values()].at(-1);
    return { history: entries.map(({ item, turnId }) => normalizeCodexItem(item, this.delivery(turnId, item.id))),
      historyMessageIds: entries.map(({ item }) => item.id), ...(activeMessage && { activeMessage }) };
  }
  item(item: CodexItem, turnId: string, completed: boolean): void {
    const message = normalizeCodexItem(item, this.delivery(turnId, item.id));
    if (['agentMessage', 'reasoning', 'plan'].includes(item.type) && !completed) {
      this.running.set(item.id, { messageId: item.id, message });
    } else if (isCodexToolItem(item)) {
      if (!completed) {
        this.tools.set(item.id, { item, turnId });
        this.items.set(item.id, { item, turnId });
        this.emit({ type: 'tool_start', sessionId: this.sessionId, toolCallId: item.id,
          toolName: toolName(item), args: codexToolInput(item) ?? null });
      } else this.emit({ type: 'tool_end', sessionId: this.sessionId, toolCallId: item.id,
        toolName: toolName(item), result: message, isError: message.isError === true });
    }
    if (completed) {
      this.running.delete(item.id);
      this.tools.delete(item.id);
      this.items.set(item.id, { item, turnId });
      this.emit({ type: 'message_end', sessionId: this.sessionId, messageId: item.id, message });
    }
  }
  unfinishedTools(turnId: string): CodexItem[] {
    return [...this.tools.values()].filter(entry => entry.turnId === turnId).map(entry => entry.item);
  }
  interruptedItems(turnId: string): CodexItem[] {
    const tools = this.unfinishedTools(turnId).map(item => ({ ...item, hiveInterrupted: true }));
    return [...tools, ...[...this.running.values()].map(({ messageId, message }) => {
      const content = (message as { content: { text?: string; thinking?: string }[] }).content[0];
      return content?.thinking !== undefined
        ? { type: 'reasoning', id: messageId, summary: [content.thinking], content: [], hiveInterrupted: true }
        : { type: 'agentMessage', id: messageId, text: content?.text ?? '', hiveInterrupted: true };
    })];
  }
  delta(method: string, params: Record<string, unknown>): void {
    const id = params.itemId;
    if (typeof id !== 'string' || typeof params.delta !== 'string') return;
    if (method === 'item/agentMessage/delta' || method === 'item/reasoning/summaryTextDelta' || method === 'item/reasoning/textDelta') {
      const kind = method === 'item/agentMessage/delta' ? 'text' : 'thinking';
      const current = this.running.get(id);
      const message = current?.message as { content?: { type: string; text?: string; thinking?: string }[] } | undefined;
      const text = `${message?.content?.[0]?.text ?? message?.content?.[0]?.thinking ?? ''}${params.delta}`;
      this.running.set(id, { messageId: id, message: { role: 'assistant', content: [kind === 'text'
        ? { type: 'text', text } : { type: 'thinking', thinking: text }] } });
      this.emit({ type: 'assistant_delta', sessionId: this.sessionId, messageId: id, kind, contentIndex: 0, delta: params.delta });
    } else if (method === 'item/commandExecution/outputDelta' || method === 'item/fileChange/outputDelta') {
      const pending = this.tools.get(id);
      if (!pending) return;
      const output = `${typeof pending.item.aggregatedOutput === 'string' ? pending.item.aggregatedOutput : ''}${params.delta}`;
      const entry = { turnId: pending.turnId, item: { ...pending.item, aggregatedOutput: output } };
      this.tools.set(id, entry);
      this.items.set(id, entry);
      this.emit({ type: 'tool_update', sessionId: this.sessionId, toolCallId: id,
        toolName: toolName(pending.item), result: { content: [{ type: 'text', text: output }] } });
    }
  }
}

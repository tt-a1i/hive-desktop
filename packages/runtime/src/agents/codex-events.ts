import type { PiEvent } from './types.js';
import { normalizeCodexItem, toolName, type CodexDelivery, type CodexItem } from './codex-history.js';

export class CodexEvents {
  private readonly items = new Map<string, { item: CodexItem; turnId: string }>();
  private readonly running = new Map<string, { messageId: string; message: unknown }>();
  constructor(private readonly sessionId: string, private readonly emit: (event: PiEvent) => void,
    private readonly delivery: (turnId: string) => CodexDelivery | undefined) {}

  restore(items: { item: CodexItem; turnId: string }[]): void {
    this.items.clear();
    this.running.clear();
    for (const entry of items) this.items.set(entry.item.id, entry);
  }
  snapshot() {
    const entries = [...this.items.values()];
    const activeMessage = [...this.running.values()].at(-1);
    return { history: entries.map(({ item, turnId }) => normalizeCodexItem(item, this.delivery(turnId))),
      historyMessageIds: entries.map(({ item }) => item.id), ...(activeMessage && { activeMessage }) };
  }
  item(item: CodexItem, turnId: string, completed: boolean): void {
    const message = normalizeCodexItem(item, this.delivery(turnId));
    if (['agentMessage', 'reasoning', 'plan'].includes(item.type) && !completed) {
      this.running.set(item.id, { messageId: item.id, message });
    } else if (!['userMessage', 'agentMessage', 'reasoning', 'plan'].includes(item.type)) {
      if (!completed) this.emit({ type: 'tool_start', sessionId: this.sessionId, toolCallId: item.id,
        toolName: toolName(item), args: item.arguments ?? item });
      else this.emit({ type: 'tool_end', sessionId: this.sessionId, toolCallId: item.id,
        toolName: toolName(item), result: message, isError: message.isError === true });
    }
    if (completed) {
      this.running.delete(item.id);
      this.items.set(item.id, { item, turnId });
      this.emit({ type: 'message_end', sessionId: this.sessionId, messageId: item.id, message });
    }
  }
  interruptedItems(): CodexItem[] {
    return [...this.running.values()].map(({ messageId, message }) => {
      const content = (message as { content: { text?: string; thinking?: string }[] }).content[0];
      return content?.thinking !== undefined
        ? { type: 'reasoning', id: messageId, summary: [content.thinking], content: [], hiveInterrupted: true }
        : { type: 'agentMessage', id: messageId, text: content?.text ?? '', hiveInterrupted: true };
    });
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
      this.emit({ type: 'tool_update', sessionId: this.sessionId, toolCallId: id,
        toolName: method.split('/')[1]!, result: { content: [{ type: 'text', text: params.delta }] } });
    }
  }
}

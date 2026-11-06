import type { DeliveryPresentation } from '../delivery-presentation.js';

export type CodexItem = { id: string; type: string; [key: string]: unknown };
export type CodexDelivery = { turnId: string; deliveryId: string; presentation: DeliveryPresentation };
export type CodexMetadata = { version: 1; context: { text: string; customType: string }[]; deliveries: CodexDelivery[]; completed: string[] };
export type CodexTurn = { id: string; status: string; error?: { message: string } | null; items: CodexItem[] };
export type CodexThread = { id: string; path: string | null; historyMode: 'legacy' | 'paginated'; turns: CodexTurn[] };
export type CodexThreadResponse = { thread: CodexThread; model: string; modelProvider: string };

export function toolName(item: CodexItem): string {
  return typeof item.tool === 'string' ? item.tool : item.type;
}

/** Presentation projection only: Codex owns its rollout and all prompt history. */
export function normalizeCodexItem(item: CodexItem, delivery?: CodexDelivery): Record<string, unknown> {
  const rawBackendItem = item;
  const timestamp = typeof item.startedAtMs === 'number' ? item.startedAtMs : undefined;
  switch (item.type) {
    case 'hiveProjectionNotice': return { role: 'notice', content: [{ type: 'text', text: item.text }], rawBackendItem };
    case 'userMessage': {
      const content = Array.isArray(item.content) ? item.content.map((part: Record<string, unknown>) =>
        part.type === 'text' ? { type: 'text', text: part.text } : part) : [];
      if (!delivery) return { role: 'user', content, timestamp, rawBackendItem };
      const presentation = { ...delivery.presentation, deliveryId: delivery.deliveryId };
      return { role: 'custom', customType: 'hive-delivery', display: false, content, timestamp, rawBackendItem,
        details: presentation, presentation };
    }
    case 'agentMessage': case 'plan':
      return { role: 'assistant', content: [{ type: 'text', text: item.text ?? '' }], timestamp, rawBackendItem };
    case 'reasoning':
      return { role: 'assistant', content: [{ type: 'thinking', thinking:
        [...(Array.isArray(item.summary) ? item.summary : []), ...(Array.isArray(item.content) ? item.content : [])].join('\n') }], timestamp, rawBackendItem };
    default:
      return { role: 'toolResult', toolCallId: item.id, toolName: toolName(item),
        content: [{ type: 'text', text: typeof item.aggregatedOutput === 'string' ? item.aggregatedOutput : JSON.stringify(item) }],
        isError: item.status === 'failed' || item.success === false, timestamp, rawBackendItem };
  }
}

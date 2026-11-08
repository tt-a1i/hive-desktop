import type { DeliveryPresentation } from '../delivery-presentation.js';

export type CodexItem = { id: string; type: string; [key: string]: unknown };
export type CodexDelivery = { turnId: string; deliveryId: string; presentation: DeliveryPresentation };
export type CodexMetadata = { version: 1; context: { text: string; customType: string }[]; deliveries: CodexDelivery[]; completed: string[] };
export type CodexTurn = { id: string; status: string; error?: { message: string } | null; items: CodexItem[] };
export type CodexThread = { reasoningEffort?: string | null; model?: string; id: string; path: string | null; historyMode: 'legacy' | 'paginated'; turns: CodexTurn[] };
export type CodexThreadResponse = { thread: CodexThread; model: string; modelProvider: string };

export function isCodexToolItem(item: CodexItem): boolean {
  return !['userMessage', 'agentMessage', 'reasoning', 'plan', 'hiveProjectionNotice'].includes(item.type);
}

export function toolName(item: CodexItem): string {
  return typeof item.tool === 'string' ? item.tool : item.type;
}

export function codexToolInput(item: CodexItem): unknown {
  if (item.arguments !== undefined) return item.arguments;
  switch (item.type) {
    case 'commandExecution': return { command: item.command, cwd: item.cwd };
    case 'fileChange': return { changes: item.changes };
    case 'webSearch': return { query: item.query, action: item.action };
    default: return undefined;
  }
}

function toolOutput(item: CodexItem): string {
  if (typeof item.aggregatedOutput === 'string') return item.aggregatedOutput;
  if (item.type === 'dynamicToolCall' && Array.isArray(item.contentItems)
    && item.contentItems.every(part => part?.type === 'inputText' && typeof part.text === 'string')) {
    return item.contentItems.map(part => part.text).join('\n\n');
  }
  if (item.type === 'mcpToolCall') {
    const result = item.error ?? item.result;
    if (result != null) return JSON.stringify(result, null, 2);
  }
  return JSON.stringify(item, null, 2);
}

/** Presentation projection only: Codex owns its rollout and all prompt history. */
export function normalizeCodexItem(item: CodexItem, delivery?: CodexDelivery): Record<string, unknown> {
  const rawBackendItem = item;
  const timestamp = typeof item.startedAtMs === 'number' ? item.startedAtMs : undefined;
  switch (item.type) {
    case 'hiveProjectionNotice': return { role: 'notice', content: [{ type: 'text', text: item.text }], rawBackendItem };
    case 'userMessage': {
      const content = Array.isArray(item.content) ? item.content.map((part: Record<string, unknown>) =>
        part.type === 'text' ? { type: 'text', text: part.text } : part.type==='image' && typeof part.url==='string' && /^data:(image\/(?:png|jpeg|gif|webp));base64,/.test(part.url)
          ? {type:'image',mimeType:part.url.slice(5,part.url.indexOf(';')),data:part.url.slice(part.url.indexOf(',')+1)} : part) : [];
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
      return { role: 'toolResult', toolCallId: item.id, toolName: toolName(item), toolInput: codexToolInput(item),
        toolStatus: item.hiveInterrupted === true ? 'interrupted' : item.status, content: [{ type: 'text', text: toolOutput(item) }],
        isError: item.status === 'failed' || item.success === false, timestamp, rawBackendItem };
  }
}

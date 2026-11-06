import type { SessionEntry } from '@earendil-works/pi-coding-agent';
import { isDeliveryPresentation } from '../delivery-presentation.js';

export function externalPiBranch(entries: readonly SessionEntry[], leafId: string | null): SessionEntry[] {
  const byId = new Map(entries.map(entry => [entry.id, entry]));
  const branch: SessionEntry[] = [];
  const visited = new Set<string>();
  let id = leafId;
  while (id !== null) {
    if (visited.has(id)) throw new Error('External Pi session branch contains a cycle');
    visited.add(id);
    const entry = byId.get(id);
    if (!entry) throw new Error('External Pi session branch parent is missing');
    branch.push(entry); id = entry.parentId;
  }
  return branch.reverse();
}
export function externalPiMessage(entry: SessionEntry): Record<string, unknown> | undefined {
  let message: Record<string, unknown>;
  if (entry.type === 'message') message = { ...entry.message };
  else if (entry.type === 'custom_message') message = { role: 'custom', customType: entry.customType, content: entry.content,
    display: entry.display, details: entry.details, timestamp: Date.parse(entry.timestamp) };
  else return;
  if (message.role === 'custom' && message.customType === 'hive-delivery' && isDeliveryPresentation(message.details)
    && 'deliveryId' in message.details && typeof message.details.deliveryId === 'string') message.presentation = message.details;
  return { ...message, rawBackendItem: entry, source: { backend: 'pi-rpc', entryId: entry.id } };
}

/** Pi RPC updates omit cumulative SDK messages; reconstruct only the current content block. */
export function applyExternalPiUpdate(message: Record<string, unknown>, event: Record<string, unknown>): Record<string, unknown> {
  const content = Array.isArray(message.content) ? [...message.content] as Record<string, unknown>[] : [];
  const update = event.assistantMessageEvent as Record<string, unknown>;
  const index = update.contentIndex;
  if (typeof index !== 'number') return message;
  const previous = content[index];
  switch (update.type) {
    case 'text_start': content[index] = { type: 'text', text: '' }; break;
    case 'thinking_start': content[index] = { type: 'thinking', thinking: '' }; break;
    case 'text_delta': content[index] = { ...previous, type: 'text', text: String(previous?.text ?? '') + String(update.delta) }; break;
    case 'thinking_delta': content[index] = { ...previous, type: 'thinking', thinking: String(previous?.thinking ?? '') + String(update.delta) }; break;
    case 'text_end': content[index] = { type: 'text', text: update.content }; break;
    case 'thinking_end': content[index] = { type: 'thinking', thinking: update.content }; break;
    case 'toolcall_start': content[index] = { type: 'toolCall', id: update.id, name: update.toolName, arguments: {} }; break;
    case 'toolcall_end': content[index] = update.toolCall as Record<string, unknown>; break;
  }
  return { ...message, content, ...(event.usage !== undefined && { usage: event.usage }) };
}

import type { SessionMessage } from '@anthropic-ai/claude-agent-sdk';
import type { ClaudeMetadata } from './claude-metadata.js';
export type ClaudeHistoryItem = { id: string; message: Record<string, unknown> };
export function projectClaudeMessage(source: Pick<SessionMessage, 'type' | 'uuid' | 'session_id' | 'message' | 'parent_tool_use_id'>, metadata: ClaudeMetadata): ClaudeHistoryItem[] {
  if (source.parent_tool_use_id !== null || !isObject(source.message)) return [];
  const raw = source.message;
  const blocks = typeof raw.content === 'string' ? [{ type: 'text', text: raw.content }] : Array.isArray(raw.content) ? raw.content.filter(isObject) : [];
  const base = { rawBackendItem: source, source: { backend: 'claude-sdk', nativeUuid: source.uuid }, ...(raw.model !== undefined && { model: raw.model }) };
  if (source.type === 'system') return [{ id: source.uuid, message: { role: 'notice', content: blocks, ...base } }];
  const toolResults = blocks.filter(block => block.type === 'tool_result').map(block => ({ id: String(block.tool_use_id), message: {
    role: 'toolResult', toolCallId: block.tool_use_id, content: typeof block.content === 'string' ? [{ type: 'text', text: block.content }] : block.content ?? [], isError: block.is_error === true, ...base } }));
  const content = blocks.filter(block => block.type !== 'tool_result').map(block => block.type === 'tool_use'
    ? { type: 'toolCall', id: block.id, name: block.name, arguments: block.input } : block);
  if (toolResults.length && content.length === 0) return toolResults;
  const delivery = metadata.deliveries.find(entry => entry.uuid === source.uuid);
  const context = metadata.context.find(entry => entry.uuid === source.uuid);
  const presentation = delivery && { ...delivery.presentation, deliveryId: delivery.deliveryId };
  const message = delivery || context ? { role: 'custom', customType: delivery ? 'hive-delivery' : context!.customType,
    display: false, content, ...(presentation && { details: presentation, presentation }), ...base }
    : { role: source.type, content, ...base };
  return [{ id: source.uuid, message }, ...toolResults];
}
export function isObject(value: unknown): value is Record<string, unknown> { return !!value && typeof value === 'object' && !Array.isArray(value); }

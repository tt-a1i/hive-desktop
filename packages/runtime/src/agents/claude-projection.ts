import type { SDKPartialAssistantMessage } from '@anthropic-ai/claude-agent-sdk';
import { isObject, isClaudeNoQueryPlaceholder, projectClaudeMessage, type ClaudeHistoryItem } from './claude-history.js';
import type { ClaudeMetadata } from './claude-metadata.js';
type Source = Parameters<typeof projectClaudeMessage>[0];
/** SDK assistant envelopes may each contain one block of the same native API message. */
export class ClaudeProjection {
  private rows = new Map<string, ClaudeHistoryItem>();
  private groups = new Map<string, Source[]>();
  private aliases = new Map<string, string>();
  private previous: Source | undefined;
  private apiId: string | undefined;
  private active: { messageId: string; message: Record<string, unknown> } | undefined;
  constructor(private readonly metadata: () => ClaudeMetadata) {}
  clear(): void { this.rows.clear(); this.groups.clear(); this.previous = undefined; }
  endStream(): void { this.active = undefined; this.apiId = undefined; }
  hasNativeUuid(uuid: string): boolean { return [...this.rows.values()].some(item => {
    const source = item.message.source as { nativeUuid: string; nativeUuids?: string[] }; return source.nativeUuid === uuid || source.nativeUuids?.includes(uuid);
  }); }
  snapshot() { return { history: [...this.rows.values()].map(item => item.message), historyMessageIds: [...this.rows.keys()], ...(this.active && { activeMessage: structuredClone(this.active) }) }; }
  put(source: Source): ClaudeHistoryItem[] {
    if (source.parent_tool_use_id !== null) return [];
    const hiddenContextUuid = this.previous?.type === 'user' ? this.previous.uuid : undefined;
    this.previous = source;
    if (hiddenContextUuid && isClaudeNoQueryPlaceholder(source) && this.metadata().context.some(entry => entry.uuid === hiddenContextUuid)) {
      return this.insert(projectClaudeMessage(source, this.metadata(), hiddenContextUuid));
    }
    if (source.type !== 'assistant' || !isObject(source.message) || typeof source.message.id !== 'string') return this.insert(projectClaudeMessage(source, this.metadata()));
    const apiId = source.message.id;
    const group = this.groups.get(apiId) ?? [];
    const index = group.findIndex(entry => entry.uuid === source.uuid);
    if (index === -1) group.push(source); else group[index] = source;
    this.groups.set(apiId, group);
    let content: unknown[] = [];
    for (const entry of group) {
      const incoming = isObject(entry.message) && Array.isArray(entry.message.content) ? entry.message.content : [];
      if ((content.length === 0 || content.length < incoming.length) && content.every((block, i) => JSON.stringify(block) === JSON.stringify(incoming[i]))) content = incoming;
      else content = [...content, ...incoming];
    }
    const first = group[0]!;
    const merged = { ...source, uuid: first.uuid, message: { ...source.message, content } };
    const item = projectClaudeMessage(merged, this.metadata())[0];
    if (!item) return [];
    item.id = this.aliases.get(apiId) ?? first.uuid;
    item.message.rawBackendItem = first;
    item.message.rawBackendItems = [...group];
    item.message.source = { backend: 'claude-sdk', nativeUuid: first.uuid, nativeUuids: group.map(entry => entry.uuid), apiMessageId: apiId };
    return this.insert([item]);
  }
  stream(message: SDKPartialAssistantMessage): { messageId: string; kind: 'text' | 'thinking'; contentIndex: number; delta: string } | undefined {
    const event = message.event;
    if (event.type === 'message_start') {
      this.apiId = event.message.id;
      const id = this.aliases.get(this.apiId) ?? message.uuid;
      this.aliases.set(this.apiId, id);
      this.active = { messageId: id, message: { role: 'assistant', content: [], model: event.message.model } };
      return;
    }
    if (!this.active) return;
    const content = this.active.message.content as Record<string, unknown>[];
    if (event.type === 'content_block_start') {
      const block = event.content_block;
      content[event.index] = block.type === 'tool_use' ? { type: 'toolCall', id: block.id, name: block.name, arguments: block.input } : { ...block };
    } else if (event.type === 'content_block_delta' && (event.delta.type === 'text_delta' || event.delta.type === 'thinking_delta')) {
      const thinking = event.delta.type === 'thinking_delta';
      const delta = event.delta.type === 'thinking_delta' ? event.delta.thinking : event.delta.text;
      const key = thinking ? 'thinking' : 'text';
      content[event.index] = { type: thinking ? 'thinking' : 'text', [key]: String(content[event.index]?.[key] ?? '') + delta };
      return { messageId: this.active.messageId, kind: thinking ? 'thinking' : 'text', contentIndex: event.index, delta };
    } else if (event.type === 'message_stop') this.active = undefined;
    return;
  }
  private insert(items: ClaudeHistoryItem[]): ClaudeHistoryItem[] { for (const item of items) this.rows.set(item.id, item); return items; }
}

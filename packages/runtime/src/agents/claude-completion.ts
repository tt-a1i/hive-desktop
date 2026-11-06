import type { SessionMessage } from '@anthropic-ai/claude-agent-sdk';
import { isObject } from './claude-history.js';
export type ClaudeTerminal = { subtype: string; is_error: boolean; uuid: string; user_message_uuid?: string; user_message_uuids?: string[]; terminal_reason?: string; queued_turn_count?: number };
/** Requires an observed terminal result and its real assistant/input on the native active chain. */
export function claudeCompletionAssistant(messages: readonly SessionMessage[], inputUuid: string, observedAssistants: ReadonlySet<string>, terminal: ClaudeTerminal): string {
  if (terminal.is_error || terminal.subtype !== 'success' || (terminal.terminal_reason && terminal.terminal_reason !== 'completed') || (terminal.queued_turn_count ?? 0) > 0
    || !(terminal.user_message_uuid === inputUuid || terminal.user_message_uuids?.includes(inputUuid))) throw new Error('Claude result is not a successful terminal for this input');
  const index = messages.findIndex(message => message.uuid === inputUuid && message.type === 'user');
  if (index === -1) throw new Error('Claude delivery input is not durable on the active chain');
  const subsequent = messages.slice(index + 1);
  if (subsequent.some(message => message.type === 'user' && (!isObject(message.message) || !Array.isArray(message.message.content)
    || message.message.content.some(block => !isObject(block) || block.type !== 'tool_result')))) throw new Error('Claude delivery was superseded by a later input');
  const assistant = subsequent.findLast(message => message.type === 'assistant');
  if (!assistant || !observedAssistants.has(assistant.uuid)) throw new Error('Claude terminal assistant is not durable on the active chain');
  return assistant.uuid;
}

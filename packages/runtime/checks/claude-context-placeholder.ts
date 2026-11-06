/** Structured native-record replay only; no Claude/model request. */
import assert from 'node:assert/strict';
import type { SessionMessage } from '@anthropic-ai/claude-agent-sdk';
import { ClaudeProjection } from '../src/agents/claude-projection.js';
import { claudeCompletionAssistant } from '../src/agents/claude-completion.js';
import type { ClaudeMetadata } from '../src/agents/claude-metadata.js';
const metadata: ClaudeMetadata = { version: 1, context: [{ uuid: 'hive-hidden', customType: 'hive-desktop', text: 'Hidden context' }], deliveries: [], completed: [] };
const source = (type: 'user' | 'assistant', uuid: string, message: Record<string, unknown>): SessionMessage => ({ type, uuid, message,
  session_id: 'fixture-session', parent_tool_use_id: null, parent_agent_id: null });
const context = source('user', 'hive-hidden', { role: 'user', content: 'Hidden context' });
const placeholder = source('assistant', 'native-placeholder', { role: 'assistant', id: 'native-synthetic-id', model: '<synthetic>',
  stop_reason: 'stop_sequence', stop_sequence: '', usage: { input_tokens: 0, output_tokens: 0 }, content: [{ type: 'text', text: 'No response requested.' }] });
const projection = new ClaudeProjection(() => metadata);
projection.put(context); projection.put(placeholder);
const hidden = projection.snapshot();
assert.equal(hidden.history.length, 2, 'Retain the native placeholder, without deleting provenance');
assert.equal(hidden.history[1]?.role, 'custom');
assert.equal(hidden.history[1]?.display, false);
assert.equal(hidden.history[1]?.customType, 'hive-context-placeholder');
assert.deepEqual(hidden.history[1]?.rawBackendItem, placeholder);
assert.deepEqual(hidden.history[1]?.source, { backend: 'claude-sdk', nativeUuid: placeholder.uuid, hiddenContextUuid: context.uuid });
assert.deepEqual(hidden.historyMessageIds, [context.uuid, placeholder.uuid]);
assert.equal(projection.hasNativeUuid(placeholder.uuid), true);

const realUser = source('user', 'real-user', { role: 'user', content: 'No response requested.' });
const realAssistant = source('assistant', 'real-assistant', { role: 'assistant', id: 'real-api-id', model: 'fixture-real-provider-no-inference',
  content: [{ type: 'text', text: 'No response requested.' }] });
projection.put(realUser); projection.put(realAssistant);
assert.equal(projection.snapshot().history.at(-2)?.role, 'user');
assert.equal(projection.snapshot().history.at(-1)?.role, 'assistant');
assert.deepEqual(projection.snapshot().history.at(-1)?.content, [{ type: 'text', text: 'No response requested.' }]);
const unrelated = new ClaudeProjection(() => metadata);
unrelated.put(context); unrelated.put(realUser); unrelated.put(placeholder);
assert.equal(unrelated.snapshot().history.at(-1)?.role, 'assistant', 'A synthetic marker after ordinary input must not be attributed to Hive hidden context');
const error = new ClaudeProjection(() => metadata);
error.put(context); error.put({ ...placeholder, uuid: 'native-error', error: 'api_error' } as SessionMessage);
assert.equal(error.snapshot().history.at(-1)?.role, 'assistant', 'Keep native error assistants');
const tool = source('user', 'native-tool-result', { role: 'user', content: [{ type: 'tool_result', tool_use_id: 'tool-1', content: 'No response requested.' }] });
projection.put(tool);
assert.equal(projection.snapshot().history.at(-1)?.role, 'toolResult');
assert.deepEqual(projection.snapshot().history.at(-1)?.content, [{ type: 'text', text: 'No response requested.' }]);
const terminal = { subtype: 'success', is_error: false, uuid: 'terminal', user_message_uuid: realUser.uuid, terminal_reason: 'completed' };
assert.throws(() => claudeCompletionAssistant([realUser, placeholder], realUser.uuid, new Set([placeholder.uuid]), terminal));
assert.equal(claudeCompletionAssistant([realUser, realAssistant], realUser.uuid, new Set([realAssistant.uuid]), terminal), realAssistant.uuid);
projection.clear(); projection.put(placeholder);
assert.equal(projection.snapshot().history[0]?.role, 'assistant', 'A refresh must reset prior context association');
console.log('Claude hidden-context placeholders retain native provenance without fake assistant bubbles; same-text real messages/errors/tools and completion boundaries preserved. No inference.');

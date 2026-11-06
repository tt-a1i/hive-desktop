/** Native Claude CLI + official SDK + real Hive HTTP/SQLite, without activating any member or querying a model. */
import assert from 'node:assert/strict';
import { mkdir, mkdtemp, writeFile } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { getSessionMessages } from '@anthropic-ai/claude-agent-sdk';
import { createDesktopRuntime } from '../src/desktop-runtime.js';
import { DesktopSettings, DesktopSettingsValidationError } from '../src/desktop-settings.js';
import type { ChatEvent, Chats } from '../src/chats.js';
import type { ClaudeSessionHandle } from '../src/agents/claude.js';

await mkdir(resolve('../../.artifacts'), { recursive: true });
const directory = await mkdtemp(resolve('../../.artifacts/claude-runtime-'));
const dataDir = join(directory, 'state');
const command = '/Users/tsk/.npm-global/bin/claude';
const events: ChatEvent[] = [];
const options = { executableEnvironment: {} };
type Runtime = Awaited<ReturnType<typeof createDesktopRuntime>>;
const handles = (chats: Chats) => (chats as unknown as { live: Map<string, { handle: ClaudeSessionHandle }> }).live;
let runtime: Runtime | undefined;
try {
  runtime = await createDesktopRuntime(dataDir, event => events.push(event), options);
  await runtime.hive.request('PUT', '/api/settings/app-state/desktop_executables', { value: JSON.stringify({ piCommand: null, codexCommand: null }) });
  assert.equal((await runtime.settings.get()).claudeCommand, null);
  await runtime.settings.update({ piCommand: null, codexCommand: null, claudeCommand: command });
  await runtime.settings.update({ piCommand: null, codexCommand: null });
  assert.equal((await runtime.settings.get()).claudeCommand, command, 'Old two-field clients must preserve the Claude setting');
  const env = await new DesktopSettings(runtime.hive, { HIVE_CLAUDE_COMMAND: '/env/Claude Agent/claude' }).get();
  assert.deepEqual(env.effective.claude, { command: '/env/Claude Agent/claude', source: 'env' });
  await assert.rejects(runtime.settings.update({ piCommand: null, codexCommand: null, claudeCommand: 'bad\npath' }), DesktopSettingsValidationError);
  assert.equal((await runtime.settings.get()).claudeCommand, command);
  const workspace = await runtime.hive.request('POST', '/api/workspaces', {
    path: directory, name: 'Unbound Claude controls only', autostart_orchestrator: false,
  }) as { id: string };
  const catalog = await runtime.agentModels.list(workspace.id, 'claude');
  assert.ok(catalog.models.length > 0);
  assert.ok(catalog.models.every(model => model.provider === 'claude'));
  assert.deepEqual(runtime.chats.list(workspace.id), []);
  const chat = await runtime.chats.create(workspace.id, 'claude');
  const handle = handles(runtime.chats).get(chat.id)!.handle;
  assert.equal(chat.backend, 'claude');
  assert.equal(chat.agentId, undefined);
  assert.equal(chat.sessionFile, null);
  assert.equal(chat.piSessionId, undefined);
  assert.equal(chat.externalPiSessionId, undefined);
  assert.equal(chat.codexThreadId, undefined);
  assert.equal(chat.nativeclaudeSessionId, handle.sessionId);
  assert.equal(handle.hasContext('hive-desktop-claude'), true);
  const firstToolName = handle.teamToolName;
  assert.ok(chat.model);
  // Reselect only the existing native model using its control RPC; never submit a prompt.
  await runtime.chats.setModel(chat.id, 'claude', chat.model.id);
  const native = await getSessionMessages(handle.sessionId, { dir: directory });
  assert.ok(native.length >= 2);
  assert.ok(native.every(message => message.type === 'user'), 'All native entries must be shouldQuery:false hidden context, with no assistant');
  assert.ok(chat.history.every(message => (message as { role?: string }).role === 'custom'));
  const priorIds = chat.historyMessageIds;
  const agentId = `${workspace.id}:orchestrator`;
  runtime.agents.configureAgentLaunch(workspace.id, agentId, { command: 'hive:claude', args: ['--provider', 'claude', '--model', chat.model.id] });
  const inherited = runtime.agents.resolveSpawnLaunchConfig?.(workspace.id);
  assert.equal(inherited?.command, 'hive:claude');
  assert.deepEqual(inherited?.args, ['--provider', 'claude', '--model', chat.model.id]);
  assert.deepEqual(await runtime.hive.request('GET', `/api/ui/workspaces/${workspace.id}/runs`), []);
  assert.deepEqual(await runtime.hive.request('GET', `/api/ui/workspaces/${workspace.id}/dispatches`), []);
  assert.equal(runtime.agents.peekAgentToken(agentId), undefined);
  await runtime.close(); runtime = undefined;

  runtime = await createDesktopRuntime(dataDir, event => events.push(event), options);
  const restored = await runtime.chats.open(chat.id);
  assert.equal(restored.nativeclaudeSessionId, chat.nativeclaudeSessionId);
  assert.deepEqual(restored.historyMessageIds.slice(0, priorIds.length), priorIds);
  const restoredHandle = handles(runtime.chats).get(chat.id)!.handle;
  assert.notEqual(restoredHandle.teamToolName, firstToolName);
  assert.equal(restored.agentId, undefined);
  assert.equal(restored.runId, null);
  assert.equal(restored.busy, false);
  assert.equal(restored.pending, 0);
  const pid = restoredHandle.processId!;
  assert.ok(pid);
  process.kill(pid, 'SIGKILL');
  // open() awaits the shared unexpected-exit cleanup before creating a replacement.
  const deadline = Date.now() + 5000;
  while (!events.some(event => event.type === 'error' && event.sessionId === chat.id) && Date.now() < deadline) {
    await new Promise(resolve => setTimeout(resolve, 10));
  }
  assert.ok(events.some(event => event.type === 'error' && event.sessionId === chat.id));
  const reopened = await runtime.chats.open(chat.id);
  assert.equal(reopened.nativeclaudeSessionId, chat.nativeclaudeSessionId);
  assert.notEqual(handles(runtime.chats).get(chat.id)!.handle.processId, pid);
  assert.throws(() => process.kill(pid, 0));
  await runtime.settings.update({ piCommand: null, codexCommand: null, claudeCommand: join(directory, 'Missing Claude Agent', 'claude') });
  await assert.rejects(runtime.agentModels.list(workspace.id, 'claude'));
  assert.equal((await runtime.chats.open(chat.id)).nativeclaudeSessionId, chat.nativeclaudeSessionId);
  await runtime.settings.update({ piCommand: null, codexCommand: null, claudeCommand: null });
  assert.equal((await runtime.settings.get()).effective.claude.source, 'path');
  const finalNative = await getSessionMessages(chat.nativeclaudeSessionId!, { dir: directory });
  const syntheticPlaceholders = finalNative.filter(message => message.type === 'assistant');
  for (const message of syntheticPlaceholders) {
    const raw = message.message as { model?: string; usage?: { input_tokens: number; output_tokens: number }; content?: unknown };
    assert.equal(raw.model, '<synthetic>');
    assert.equal(raw.usage?.input_tokens, 0);
    assert.equal(raw.usage?.output_tokens, 0);
    assert.deepEqual(raw.content, [{ type: 'text', text: 'No response requested.' }]);
  }
  assert.ok(finalNative.every(message => message.type === 'user' || syntheticPlaceholders.includes(message)));
  assert.equal(events.some(event => event.type === 'assistant_delta' || event.type === 'tool_start'), false);
  assert.deepEqual(await runtime.hive.request('GET', `/api/ui/workspaces/${workspace.id}/runs`), []);
  assert.deepEqual(await runtime.hive.request('GET', `/api/ui/workspaces/${workspace.id}/dispatches`), []);
  await writeFile(join(directory, 'receipt.json'), JSON.stringify({ noInference: true, memberActivated: false, hiveRuns: 0, dispatches: 0,
    catalog, chat, restored, reopened, inheritedLaunch: inherited, initialNativeMessages: native, nativeMessages: finalNative,
    initialNativeAssistantCount: 0, nativeSyntheticPlaceholderCount: syntheticPlaceholders.length, nativeModelGeneratedAssistantCount: 0,
    modelControlOnly: true, unexpectedDeathReopenedSameNativeIdentity: true, events }, null, 2));
  console.log(`CLAUDE_RUNTIME_CONTROLS_OK ${directory}`);
} catch (error) {
  await writeFile(join(directory, 'failure.json'), JSON.stringify({ message: error instanceof Error ? error.message : String(error), stack: error instanceof Error ? error.stack : undefined, events }, null, 2));
  throw error;
} finally {
  await runtime?.close();
  console.log(`Evidence retained: ${directory}`);
}

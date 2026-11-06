/** Two real empty-chat resumes, official SDK/original CLI; no activation, prompt, or inference. */
import assert from 'node:assert/strict';
import { mkdir, mkdtemp, writeFile } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { getSessionMessages } from '@anthropic-ai/claude-agent-sdk';
import { createDesktopRuntime } from '../src/desktop-runtime.js';
import { isClaudeNoQueryPlaceholder } from '../src/agents/claude-history.js';
import type { ChatEvent } from '../src/chats.js';

await mkdir(resolve('../../.artifacts'), { recursive: true });
const directory = await mkdtemp(resolve('../../.artifacts/claude-placeholder-resume-'));
const dataDir = join(directory, 'state');
const events: ChatEvent[] = [];
const options = { executableEnvironment: {} };
type Runtime = Awaited<ReturnType<typeof createDesktopRuntime>>;
let runtime: Runtime | undefined;
try {
  runtime = await createDesktopRuntime(dataDir, event => events.push(event), options);
  await runtime.settings.update({ piCommand: null, codexCommand: null, claudeCommand: '/Users/tsk/.npm-global/bin/claude' });
  const workspace = await runtime.hive.request('POST', '/api/workspaces', {
    path: directory, name: 'Empty Claude resume visibility', autostart_orchestrator: false,
  }) as { id: string };
  const created = await runtime.chats.create(workspace.id, 'claude');
  assert.equal(created.history.some(message => (message as { role?: string }).role === 'assistant'), false);
  const rounds = [];
  for (let round = 1; round <= 2; round++) {
    await runtime.close(); runtime = undefined;
    runtime = await createDesktopRuntime(dataDir, event => events.push(event), options);
    const opened = await runtime.chats.open(created.id);
    const native = await getSessionMessages(opened.nativeclaudeSessionId!, { dir: directory });
    const placeholders = native.filter(message => message.type === 'assistant');
    assert.ok(placeholders.length > 0, 'Exercise real native resume placeholders');
    assert.ok(placeholders.every(isClaudeNoQueryPlaceholder));
    assert.equal(opened.history.some(message => (message as { role?: string }).role === 'assistant'), false);
    for (const raw of placeholders) {
      const projected = opened.history.find(message => (message as { source?: { nativeUuid?: string } }).source?.nativeUuid === raw.uuid) as Record<string, unknown> | undefined;
      assert.ok(projected);
      assert.equal(projected.role, 'custom');
      assert.equal(projected.display, false);
      assert.equal(projected.customType, 'hive-context-placeholder');
      assert.deepEqual(projected.rawBackendItem, raw);
      const contextUuid = (projected.source as { hiddenContextUuid: string }).hiddenContextUuid;
      assert.ok(opened.history.some(message => (message as { source?: { nativeUuid?: string }; role?: string }).source?.nativeUuid === contextUuid
        && (message as { role?: string }).role === 'custom'));
    }
    assert.equal(opened.nativeclaudeSessionId, created.nativeclaudeSessionId);
    assert.equal(opened.agentId, undefined);
    assert.equal(opened.runId, null);
    assert.equal(opened.pending, 0);
    assert.deepEqual(await runtime.hive.request('GET', `/api/ui/workspaces/${workspace.id}/runs`), []);
    rounds.push({ round, opened, native, nativePlaceholderCount: placeholders.length, visibleAssistantCount: 0 });
  }
  assert.equal(events.some(event => event.type === 'assistant_delta' || event.type === 'tool_start'), false);
  await writeFile(join(directory, 'receipt.json'), JSON.stringify({ noInference: true, memberActivated: false,
    created, rounds, rawSyntheticProvenanceRetained: true, events }, null, 2));
  console.log(`CLAUDE_PLACEHOLDER_RESUME_OK ${directory}`);
} catch (error) {
  await writeFile(join(directory, 'failure.json'), JSON.stringify({ message: error instanceof Error ? error.message : String(error), stack: error instanceof Error ? error.stack : undefined, events }, null, 2));
  throw error;
} finally {
  await runtime?.close();
  console.log(`Evidence retained: ${directory}`);
}

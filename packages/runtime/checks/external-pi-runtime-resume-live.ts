/** One explicit Terra turn against a previous team proof, to verify fresh-tool rebinding after restart. */
import assert from 'node:assert/strict';
import { readFile, writeFile } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { createDesktopRuntime } from '../src/desktop-runtime.js';
import type { ChatEvent, Chats } from '../src/chats.js';
import type { PiSessionHandle } from '../src/agents/types.js';
if (!process.argv.includes('--live')) throw new Error('Pass --live and the existing team-proof directory');
const directory = resolve(process.argv[process.argv.indexOf('--live') + 1]!);
const previous = JSON.parse(await readFile(join(directory, 'receipt.json'), 'utf8')) as { workspaceId: string; lead: { id: string; externalPiSessionId: string } };
const events: ChatEvent[] = [];
const runtime = await createDesktopRuntime(join(directory, 'state'), event => events.push(event));
try {
  const chat = await runtime.chats.open(previous.lead.id);
  await runtime.chats.setModel(chat.id, 'openai', 'gpt-6.1-terra');
  const handle = (runtime.chats as unknown as { live: Map<string, { handle: PiSessionHandle & { teamToolName: string } }> }).live.get(chat.id)!.handle;
  const name = handle.teamToolName;
  await runtime.chats.prompt(chat.id, '这是会话重启后的实际工具核验：使用当前进程注册的 Hive team 工具执行 team list，不要派新单、不要新建成员。确认结果后回复 EXTERNAL_PI_REBOUND_OK。');
  const deadline = Date.now() + 180_000;
  let final;
  while (Date.now() < deadline) {
    await new Promise(resolve => setTimeout(resolve, 500));
    const snapshot = await runtime.chats.open(chat.id);
    if (!snapshot.busy && snapshot.history.some(message => {
      const value = message as { role?: string; content?: { type: string; text?: string }[] };
      return value.role === 'assistant' && value.content?.some(part => part.type === 'text' && part.text?.includes('EXTERNAL_PI_REBOUND_OK'));
    })) { final = snapshot; break; }
    if (!snapshot.busy && events.some(event => event.type === 'error')) throw new Error('Resumed external Pi turn failed');
  }
  assert.ok(final, 'Resumed external Pi did not finish');
  assert.equal(final.externalPiSessionId, previous.lead.externalPiSessionId);
  assert.equal(final.model?.provider, 'openai');
  assert.equal(final.model?.id, 'gpt-6.1-terra');
  assert.ok(events.some(event => event.type === 'tool_start' && event.toolName === name
    && (event.args as { args?: string[] }).args?.includes('list')));
  assert.equal(runtime.chats.list(previous.workspaceId).length, 2);
  for (const message of final.history) {
    const item = message as { role?: string; provider?: string; model?: string };
    if (item.role === 'assistant') {
      assert.equal(item.provider, 'openai');
      assert.equal(item.model, 'gpt-6.1-terra');
    }
  }
  await writeFile(join(directory, 'resume-receipt.json'), JSON.stringify({ model: final.model, currentToolName: name, final }, null, 2));
  console.log(`EXTERNAL_PI_REBOUND_OK ${directory}`);
} catch (error) {
  await writeFile(join(directory, 'resume-failure.json'), JSON.stringify({ message: error instanceof Error ? error.message : String(error) }, null, 2));
  throw error;
} finally {
  await writeFile(join(directory, 'resume-events.json'), JSON.stringify(events, null, 2));
  await runtime.close();
}

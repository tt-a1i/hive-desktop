/** Explicit opt-in: real native Terra through Pi/Codex, actual Hive upload and user input. */
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdir, mkdtemp, readFile, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { createDesktopRuntime } from '../src/desktop-runtime.js';
import type { ChatEvent } from '../src/chats.js';

if (!process.argv.includes('--live')) throw new Error('Pass --live for a real gpt-6.1-terra image request');
const input = resolve(process.argv[process.argv.indexOf('--live') + 1] ?? '');
assert.ok(input.endsWith('.png'), 'Provide the generated blue-left/pink-right PNG');
const backendIndex = process.argv.indexOf('--backend');
const selectedBackend = backendIndex < 0 ? 'pi' : process.argv[backendIndex + 1];
assert.ok(selectedBackend && ['pi', 'codex', 'pi-rpc'].includes(selectedBackend), 'Use --backend pi, codex or pi-rpc');
const backend = selectedBackend as 'pi' | 'codex' | 'pi-rpc';
const directory = await mkdtemp(resolve('../../.artifacts/image-attachments/live-'));
const project = join(directory, 'project');
await mkdir(project);
const events: ChatEvent[] = [];
let unexpectedTool = false;
const runtime = await createDesktopRuntime(join(directory, 'state'), (event) => {
  events.push(event);
  if (event.type === 'tool_start') unexpectedTool = true;
});
let chatId: string | undefined;
try {
  const workspace = (await runtime.hive.request('POST', '/api/workspaces', {
    path: project,
    name: 'Terra image proof',
    autostart_orchestrator: false,
  })) as { id: string };
  const chat = await runtime.chats.create(workspace.id, backend);
  chatId = chat.id;
  await runtime.chats.setModel(chat.id, 'openai', 'gpt-6.1-terra');
  const image = await runtime.chats.attachments.add(workspace.id, input);
  await runtime.chats.prompt(
    chat.id,
    '只看所附图片，辨认左半边和右半边分别是什么颜色。只按“左：颜色；右：颜色”回答。不要调用任何工具、读取文件、创建成员或派任务。',
    [image.attachment.id],
  );
  const deadline = Date.now() + 120_000;
  for (;;) {
    const snapshot = await runtime.chats.open(chat.id);
    if (unexpectedTool) throw new Error('Unexpected tool call during image-only inference check');
    const responses = snapshot.history.filter((message: any) => message.role === 'assistant') as any[];
    if (!snapshot.busy && responses.length) {
      const response = responses.at(-1)!;
      assert.equal(snapshot.model?.provider, 'openai');
      assert.equal(snapshot.model?.id, 'gpt-6.1-terra');
      if (backend !== 'codex') {
        assert.equal(response.model, 'gpt-6.1-terra');
        assert.equal(response.stopReason, 'stop', response.errorMessage);
      } else {
        assert.ok(events.some(event => event.type === 'settled' && !event.aborted), 'Codex turn must settle successfully');
        assert.equal(snapshot.deliveryFailure, undefined);
      }
      assert.equal(snapshot.pending, 0, 'Image delivery must complete successfully');
      const text = response.content
        .filter((part: any) => part.type === 'text')
        .map((part: any) => part.text)
        .join('');
      assert.match(text, /左[^；;\n]*蓝/);
      assert.match(text, /右[^；;\n]*粉/);
      const original = await readFile(input);
      const receipt = {
        backend,
        provider: 'openai',
        model: snapshot.model!.id,
        modelSelection: snapshot.model,
        vendorInference: true,
        originalSha256: createHash('sha256').update(original).digest('hex'),
        attachment: image.attachment,
        text,
        toolCalls: 0,
        pending: snapshot.pending,
        usage: response.usage,
      };
      await writeFile(join(directory, 'snapshot.json'), JSON.stringify(snapshot, null, 2));
      await writeFile(join(directory, 'receipt.json'), JSON.stringify(receipt, null, 2));
      console.log(`IMAGE_ATTACHMENTS_LIVE_OK ${directory}\n${text}`);
      break;
    }
    if (!snapshot.busy && snapshot.deliveryFailure) throw new Error(`Image delivery failed: ${snapshot.deliveryFailure.message}`);
    assert.ok(Date.now() < deadline, 'Real Terra image request timed out');
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
} catch (error) {
  await writeFile(join(directory, 'failure.json'), JSON.stringify({
    backend, provider: 'openai', model: 'gpt-6.1-terra',
    message: error instanceof Error ? error.message : String(error),
    unexpectedTool,
  }, null, 2));
  throw error;
} finally {
  if (chatId) await runtime.chats.abort(chatId);
  await writeFile(join(directory, 'events.json'), JSON.stringify(events, null, 2));
  await runtime.close();
  console.log(`Evidence retained: ${directory}`);
}

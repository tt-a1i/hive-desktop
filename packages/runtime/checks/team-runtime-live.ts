import assert from 'node:assert/strict';
import { mkdir, mkdtemp, readFile, writeFile } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { createDesktopRuntime } from '../src/desktop-runtime.js';
import type { ChatEvent } from '../src/chats.js';

// Explicit opt-in: real requests, only Pi native OpenAI/ChatGPT GPT-6.1 Terra.
if (!process.argv.includes('--live')) throw new Error('Pass --live to run real GPT-6.1 Terra requests');
const artifacts = resolve('../../.artifacts');
await mkdir(artifacts, { recursive: true });
const directory = await mkdtemp(join(artifacts, 'team-live-'));
const project = join(directory, 'project');
await mkdir(project);
const events: ChatEvent[] = [];
const runtime = await createDesktopRuntime(join(directory, 'state'), event => {
  events.push(event);
  if (['tool_start', 'settled', 'error', 'state'].includes(event.type)) console.error(JSON.stringify(event));
});
try {
  const workspace = await runtime.hive.request('POST', '/api/workspaces', {
    path: project, name: 'Team live proof', autostart_orchestrator: false,
  }) as { id: string };
  const lead = await runtime.chats.create(workspace.id);
  await runtime.chats.setModel(lead.id, 'openai', 'gpt-6.1-terra');
  await runtime.chats.prompt(lead.id, '这是一次真实 Hive 桌面协作验证。使用 team 工具创建一名名叫 proof-worker 的 coder Worker，不传 --cli，让其继承内置后端和模型。给它派单：在工作区创建 team-proof.txt，内容精确为 HIVE_TEAM_TERRA_OK，读取核对，再遵循 Hive 当前协议显式 team report（包含派单 id 和所需 seen）；不要关闭其他会话，不要启动额外成员。主控不要代做文件。收到成员汇报后读取文件核验，最终回复 HIVE_TEAM_COMPLETE。使用 team 工具而不是 bash 中的 team 命令；需要用法时调用 team guide/help。');
  const deadline = Date.now() + 300_000;
  let final: unknown;
  while (Date.now() < deadline) {
    await new Promise(resolve => setTimeout(resolve, 1000));
    const snapshot = await runtime.chats.open(lead.id);
    const completed = snapshot.history.some(message => {
      if (!message || typeof message !== 'object') return false;
      const item = message as { role?: string; content?: { type: string; text?: string }[] };
      return item.role === 'assistant' && Array.isArray(item.content) && item.content.some(part => part.type === 'text' && part.text?.includes('HIVE_TEAM_COMPLETE'));
    });
    if (!snapshot.busy && completed) { final = snapshot; break; }
    const errors = events.filter(event => event.type === 'error');
    if (errors.length && !snapshot.busy) throw new Error(`Agent error: ${JSON.stringify(errors)}`);
  }
  assert.ok(final, 'Orchestrator did not receive worker report and finish in 300 seconds');
  assert.equal(await readFile(join(project, 'team-proof.txt'), 'utf8'), 'HIVE_TEAM_TERRA_OK');
  const chats = runtime.chats.list(workspace.id);
  assert.equal(chats.length, 2, 'One orchestrator and one structured worker');
  const worker = chats.find(chat => chat.id !== lead.id)!;
  assert.ok(worker.agentId && worker.agentId !== `${workspace.id}:orchestrator`);
  const workerSnapshot = await runtime.chats.open(worker.id);
  assert.ok(JSON.stringify(workerSnapshot.history).includes('HIVE_TEAM_TERRA_OK'));
  assert.equal(worker.model?.provider, 'openai');
  assert.equal(worker.model?.id, 'gpt-6.1-terra');
  const dispatches = await runtime.hive.request('GET', `/api/ui/workspaces/${workspace.id}/dispatches`) as { state: string; report_text: string | null }[];
  assert.equal(dispatches.length, 1);
  assert.equal(dispatches[0]?.state, 'reported');
  assert.ok(dispatches[0]?.report_text?.includes('team-proof.txt'));
  for (const snapshot of [final as { history: unknown[] }, workerSnapshot]) {
    for (const message of snapshot.history) {
      const item = message as { role?: string; model?: string };
      if (item.role === 'assistant') assert.equal(item.model, 'gpt-6.1-terra');
    }
  }
  const team = await runtime.hive.request('GET', `/api/ui/workspaces/${workspace.id}/team`);
  await writeFile(join(directory, 'receipt.json'), JSON.stringify({ workspaceId: workspace.id, chats, dispatches, team, lead: final, worker: workerSnapshot }, null, 2));
  console.log(`TEAM_LIVE_OK ${directory}`);
} finally {
  await writeFile(join(directory, 'events.json'), JSON.stringify(events, null, 2));
  await runtime.close();
  console.log(`Evidence retained: ${directory}`);
}

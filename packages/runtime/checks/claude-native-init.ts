/** No user messages are ever sent: official SDK control/init only, no inference. */
import assert from 'node:assert/strict';
import { mkdir, mkdtemp, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { query } from '@anthropic-ai/claude-agent-sdk';
const directory = await mkdtemp(join(tmpdir(), 'hive-claude-native-init-'));
let finish!: () => void;
const waiting = new Promise<void>(resolve => { finish = resolve; });
async function* input() { await waiting; }
const controller = new AbortController();
const timer = setTimeout(() => controller.abort(), 30000);
const running = query({ prompt: input(), options: { cwd: directory,
  pathToClaudeCodeExecutable: process.env.HIVE_CLAUDE_COMMAND ?? '/Users/tsk/.npm-global/bin/claude', abortController: controller } });
const received: { type: string; subtype?: string }[] = [];
const consume = (async () => { for await (const message of running) received.push({ type: message.type, ...('subtype' in message && { subtype: message.subtype }) }); })();
void consume.catch(() => {});
try {
  const init = await running.initializationResult();
  const models = await running.supportedModels();
  const servers = await running.mcpServerStatus();
  assert.ok(models.length > 0);
  await running.interrupt();
  await mkdir(resolve('../../.artifacts/claude'), { recursive: true });
  const receipt = { pass: true, inference: false, sentUserMessages: 0, directory,
    sdkVersion: '0.3.317', command: process.env.HIVE_CLAUDE_COMMAND ?? '/Users/tsk/.npm-global/bin/claude',
    commandCount: init.commands.length, models: models.map(model => ({ value: model.value, displayName: model.displayName })),
    mcpServers: servers.map(server => ({ name: server.name, status: server.status })), received };
  await writeFile(resolve('../../.artifacts/claude/native-init.json'), JSON.stringify(receipt, null, 2));
  console.log(JSON.stringify({ pass: true, inference: false, modelCount: models.length, commandCount: init.commands.length }));
} finally { clearTimeout(timer); finish(); running.close(); await consume.catch(() => {}); }

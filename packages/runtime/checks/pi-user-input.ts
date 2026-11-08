import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { once } from 'node:events';
import { mkdir, mkdtemp, writeFile, realpath } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { randomUUID } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { createDesktopRuntime } from '../src/desktop-runtime.js';

// Real SDK + original external CLI + Hive HTTP/SQLite. The loopback provider
// records Terra requests and serves controlled success/failure responses; no vendor endpoint is used.
const directory = await mkdtemp(join(tmpdir(), 'hive-pi-user-input-'));
const project = join(directory, 'project');
const agent = join(directory, 'agent');
const marker = randomUUID();
const skill = `SKILL_${marker}`;
const template = `TEMPLATE_${marker}`;
const transformed = `TRANSFORMED_${marker}`;
await mkdir(agent);
await mkdir(join(project, '.pi', 'skills', 'native-input'), { recursive: true });
await mkdir(join(project, '.pi', 'prompts'), { recursive: true });
await mkdir(join(project, '.pi', 'extensions'), { recursive: true });
await writeFile(join(project, '.pi', 'skills', 'native-input', 'SKILL.md'), `---\nname: native-input\ndescription: Local input contract\n---\n${skill}\n`);
await writeFile(join(project, '.pi', 'prompts', 'native-template.md'), `---\ndescription: Local template contract\n---\n${template}\n$@\n`);
await writeFile(join(project, '.pi', 'extensions', 'native-input.ts'), `export default function(pi){pi.on('input',event=>{if(event.text.startsWith('consume-input'))return {action:'handled'};if(event.text.startsWith('rewrite-input'))return {action:'transform',text:${JSON.stringify(transformed)},images:event.images};});pi.registerCommand('local-input',{description:'Local success probe',handler(){}});pi.registerCommand('fail-input',{description:'Local failure probe',handler(){throw new Error('Native input command failed');}});}`);
let phase = '';
const requests: { phase: string; body: any }[] = [];
let releaseHold!: () => void;
let held: Promise<void>;
const responses = new Set<import('node:http').ServerResponse>();
const endpoint = createServer(async (request, response) => {
  const chunks: Buffer[] = [];
  for await (const chunk of request) chunks.push(Buffer.from(chunk));
  requests.push({ phase, body: JSON.parse(Buffer.concat(chunks).toString('utf8')) });
  if (phase.endsWith('success')) {
    response.writeHead(200, { 'content-type': 'text/event-stream' });
    const item = { id: 'msg_input_success', type: 'message', role: 'assistant', status: 'completed', content: [{ type: 'output_text', text: `SUCCESS_${marker}`, annotations: [] }] };
    const stream = [
      { type: 'response.created', response: { id: 'resp_input_success' } },
      { type: 'response.output_item.added', output_index: 0, item: { ...item, content: [] } },
      { type: 'response.output_text.delta', output_index: 0, content_index: 0, delta: item.content[0]!.text },
      { type: 'response.output_item.done', output_index: 0, item },
      { type: 'response.completed', response: { id: 'resp_input_success', status: 'completed', output: [item], usage: { input_tokens: 1, output_tokens: 1, total_tokens: 2 } } },
    ];
    for (const event of stream) response.write(`event: ${event.type}\ndata: ${JSON.stringify(event)}\n\n`);
    response.end();
    return;
  }
  if (phase.endsWith('cancel')) { responses.add(response); response.once('close', () => responses.delete(response)); releaseHold(); return; }
  response.writeHead(400, { 'content-type': 'application/json' });
  response.end(JSON.stringify({ error: { type: 'invalid_request_error', message: 'Local input probe refuses inference' } }));
});
endpoint.listen(0, '127.0.0.1');
await once(endpoint, 'listening');
const port = (endpoint.address() as import('node:net').AddressInfo).port;
await writeFile(join(agent, 'settings.json'), JSON.stringify({ defaultProvider: 'native-input', defaultModel: 'gpt-6.1-terra', defaultProjectTrust: 'always', retry: { enabled: false } }));
await writeFile(join(agent, 'models.json'), JSON.stringify({ providers: { 'native-input': { api: 'openai-responses', apiKey: 'local-only', baseUrl: `http://127.0.0.1:${port}/v1`, models: [{ id: 'gpt-6.1-terra', name: 'Terra local input', input: ['text', 'image'], contextWindow: 128000, maxTokens: 1024 }] } } }));
// Only the test-owned agentDir trusts this one test-owned project. Production
// keeps the original CLI's trust policy and user configuration untouched.
process.env.PI_CODING_AGENT_DIR = agent;
process.env.PI_OFFLINE = '1';
const events: any[] = [];
const options = { executableEnvironment: { HIVE_PI_COMMAND: process.env.HIVE_PI_COMMAND ?? 'pi' } };
let runtime = await createDesktopRuntime(join(directory, 'state'), event => events.push(event), options);
const saved: { chatId: string; deliveryId: string; snapshotId: string; source: unknown; role: string; text: string }[] = [];
const image = { type: 'image', mimeType: 'image/png', data: 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGP4z8AAAAMBAQDJ/pLvAAAAAElFTkSuQmCC' };
function latestInput() { return JSON.stringify(requests.at(-1)!.body.input.filter((item: any) => item.role === 'user').at(-1)); }
function owner(id: string): any { return (runtime.chats as any).live.get(id); }
try {
  const workspace = await runtime.hive.request('POST', '/api/workspaces', { path: project, name: 'Native Pi input', autostart_orchestrator: false }) as { id: string };
  for (const backend of ['pi', 'pi-rpc'] as const) {
    const chat = await runtime.chats.create(workspace.id, backend);
    await runtime.chats.setModel(chat.id, 'native-input', 'gpt-6.1-terra');
    const catalog = await runtime.chats.commands(chat.id);
    for (const [name, source, path] of [['skill:native-input', 'skill', '.pi/skills/native-input/SKILL.md'], ['native-template', 'prompt', '.pi/prompts/native-template.md'], ['local-input', 'extension', '.pi/extensions/native-input.ts']] as const) {
      const entry = catalog.commands.find(command => command.name === name);
      assert.ok(entry, `Native ${backend} catalogue must expose loaded ${name}`);
      assert.equal(entry.source, source);
      assert.equal(await realpath(entry.sourceInfo.path), await realpath(join(project, path)));
    }
    assert.equal(catalog.commands.some(command => command.name.startsWith('hive-desktop-')), false, 'Private transport command is not a user command');
    const attachment = await runtime.chats.attachments.addClipboard(workspace.id, image);
    for (const [name, text, token, local] of [
      ['skill', '/skill:native-input skill-argument', skill, false],
      ['template', '/native-template template-argument', template, false],
      ['success', '/native-template', template, false],
      ['rewrite', 'rewrite-input original text', transformed, false],
      ['missing-template', '/missing-template literal-input', '/missing-template literal-input', false],
      ['local-command', '/local-input', '', true],
      ['handled', 'consume-input original text', '', true],
      ['command-error', '/fail-input', '', true],
    ] as const) {
      phase = `${backend}-${name}`;
      const count = requests.length;
      await runtime.chats.prompt(chat.id, text, name === 'skill' ? [attachment.attachment.id] : undefined);
      await owner(chat.id).queue.idle();
      const state = await runtime.chats.open(chat.id);
      const matches = state.history.filter((message: any) => message.presentation?.displayText === text) as any[];
      assert.equal(matches.length, 1, 'One original input row, never expanded-text duplicates');
      const row = matches[0];
      const id = row.presentation.deliveryId;
      const index = state.history.indexOf(row);
      saved.push({ chatId: chat.id, deliveryId: id, snapshotId: state.historyMessageIds[index]!, source: row.source, role: row.role, text });
      assert.equal(requests.length, count + (local ? 0 : 1));
      if (!local) {
        assert.ok(latestInput().includes(token));
        assert.equal(row.role, 'user');
        assert.equal(row.rawBackendItem.message.role, 'user');
        assert.equal(row.source.entryId, row.rawBackendItem.id);
        assert.ok(events.some(event => event.type === 'message_end' && event.message.presentation?.deliveryId === id && event.message.role === 'user'));
      } else {
        assert.equal(row.applicationEvent, 'native-user-input');
        assert.equal(row.rawBackendItem.type, 'custom', 'A locally handled input is an application record, not a forged SDK user message');
      }
      if (name === 'skill') {
        assert.ok(latestInput().includes(`data:image/png;base64,${image.data}`));
        assert.equal(row.presentation.attachments[0].id, attachment.attachment.id);
      }
      if (name === 'handled' || name === 'local-command' || name === 'success') {
        assert.equal(state.pending, 0);
        assert.ok(state.completedDeliveryIds.includes(id));
        if (name === 'success') {
          assert.ok(state.history.some((message: any) => message.role === 'assistant' && JSON.stringify(message.content).includes(`SUCCESS_${marker}`)), 'Completion requires the native final assistant response');
          assert.equal(state.deliveryFailure, undefined);
        }
      } else {
        assert.equal(state.pending, 1);
        assert.equal(state.completedDeliveryIds.includes(id), false);
        assert.ok(state.deliveryFailure);
        await runtime.chats.abort(chat.id);
      }
    }
    for (const origin of ['dispatch', 'report', 'recovery'] as const) {
      phase = `${backend}-${origin}`;
      const count = requests.length;
      const queue = owner(chat.id).queue;
      await queue.enqueue('/skill:native-input automatic-input', true, { version: 1, origin, displayText: `Automatic ${origin}` });
      const id = queue.pending[0].id;
      queue.resume();
      await queue.idle();
      assert.equal(requests.length, count + 1);
      assert.ok(latestInput().includes('/skill:native-input automatic-input'));
      assert.equal(latestInput().includes(skill), false, 'Automatic collaboration input must bypass native user commands');
      assert.equal(owner(chat.id).handle.completedDeliveryIds().includes(id), false);
      await runtime.chats.abort(chat.id);
    }
    phase = `${backend}-cancel`;
    held = new Promise(resolve => { releaseHold = resolve; });
    await runtime.chats.prompt(chat.id, '/native-template cancelled-input');
    await held;
    const before = await runtime.chats.open(chat.id);
    const row = before.history.findLast((message: any) => message.presentation?.displayText === '/native-template cancelled-input') as any;
    assert.ok(row);
    await runtime.chats.abort(chat.id);
    const after = await runtime.chats.open(chat.id);
    assert.equal(after.pending, 0);
    assert.equal(after.completedDeliveryIds.includes(row.presentation.deliveryId), false);
  }
  const otherProject = join(directory, 'other-project');
  await mkdir(otherProject);
  const otherWorkspace = await runtime.hive.request('POST', '/api/workspaces', { path: otherProject, name: 'No project commands', autostart_orchestrator: false }) as {id:string};
  for (const backend of ['pi', 'pi-rpc'] as const) {
    const other = await runtime.chats.create(otherWorkspace.id, backend);
    const catalog = await runtime.chats.commands(other.id);
    assert.equal(catalog.commands.some(command => ['skill:native-input','native-template','local-input'].includes(command.name)), false, 'Another workspace must not inherit the first project command catalogue');
  }
  await runtime.close();
  runtime = await createDesktopRuntime(join(directory, 'state'), () => {}, options);
  for (const record of saved) {
    const state = await runtime.chats.open(record.chatId);
    if (record.text.startsWith('/skill:')) {
      const catalog = await runtime.chats.commands(record.chatId);
      assert.ok(catalog.commands.some(command => command.name === 'skill:native-input'));
    }
    const index = state.historyMessageIds.indexOf(record.snapshotId);
    assert.ok(index >= 0, 'Native/application input ID must survive cold runtime restart');
    const row = state.history[index] as any;
    assert.equal(row.presentation.deliveryId, record.deliveryId);
    assert.equal(row.presentation.displayText, record.text);
    assert.equal(row.role, record.role);
    assert.deepEqual(row.source, record.source);
  }
  assert.ok(requests.every(request => request.body.model === 'gpt-6.1-terra'));
  const proof = fileURLToPath(new URL('../../../.artifacts/pi-user-input/', import.meta.url));
  await mkdir(proof, { recursive: true });
  await writeFile(join(proof, 'input-contract-receipt.json'), JSON.stringify({ directory, pass: true, originalExternalPi: options.executableEnvironment.HIVE_PI_COMMAND, vendorInference: false, model: 'gpt-6.1-terra', requests: requests.length, saved, testProjectTrustOnly: true }, null, 2));
  console.log(`PI_USER_INPUT_OK: real SDK/original RPC + Hive HTTP/SQLite; skills/templates/images, input handlers and local errors, automatic-source isolation, cancellation, native roles and cold ID recovery. Loopback only. ${directory}`);
} finally {
  await runtime.close();
  for (const response of responses) response.end();
  endpoint.closeAllConnections();
  endpoint.close();
}

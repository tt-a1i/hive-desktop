import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdir, readFile, writeFile, access, readdir } from 'node:fs/promises';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
const [root, resources, mode = 'seed'] = process.argv.slice(2);
const data = join(root, 'state'), project = join(root, 'project');
const fixture = join(root, 'question-seed.json');
Object.assign(process.env, { HIVE_QUESTION_FIXTURE: root, CODEX_HOME: join(root, 'codex-home'), HOME: root, USERPROFILE: root, PI_OFFLINE: '1' });
const content = Buffer.from('WINDOWS QUESTION ATTACHMENT\nExact bytes: 0123456789\n', 'utf8');
const sha = bytes => createHash('sha256').update(bytes).digest('hex');
const exists = async path => { try { await access(path); return true; } catch (error) { if (error.code === 'ENOENT') return false; throw error; } };
if (mode === 'seed') {
  await Promise.all([project, data, join(root, 'codex-home')].map(path => mkdir(path, { recursive: true })));
  const source = join(project, 'question-proof.txt'); await writeFile(source, content);
  const { createDesktopRuntime } = await import(pathToFileURL(join(resources, 'runtime', 'dist', 'desktop-runtime.js')).href);
  let runtime;
  try {
    runtime = await createDesktopRuntime(data, () => {}, { executableEnvironment: {} });
    const workspace = await runtime.hive.request('POST', '/api/workspaces', { path: project, name: 'Question attachment QA', autostart_orchestrator: false });
    const peer = join(project, 'app-server.cjs');
    await writeFile(peer, await readFile(new URL('./windows-question-peer.cjs', import.meta.url)));
    const { profile } = await runtime.profiles.create({ driver: 'codex', label: 'Controlled question peer', launch: { command: process.execPath, args: [peer] } }, (await runtime.profiles.list()).storeVersion);
    const chat = await runtime.chats.createRecord(workspace.id, 'codex', { profile: profile.ref });
    await runtime.chats.open(chat.id); await runtime.chats.setModel(chat.id, 'openai', 'gpt-6.1-terra');
    runtime.hive.saveNavigation(JSON.stringify({ workspace: workspace.id, sessions: { [workspace.id]: chat.id } }));
    await writeFile(join(data, 'desktop-language.json'), JSON.stringify('en'));
    await writeFile(fixture, JSON.stringify({ workspaceId: workspace.id, sessionId: chat.id, project, source, sha256: sha(content), bytesBase64: content.toString('base64'),
      environment: { HIVE_QUESTION_FIXTURE: root, CODEX_HOME: join(root, 'codex-home'), HOME: root, USERPROFILE: root } }, null, 2));
  } finally { await runtime?.close(); }
} else {
  const seed = JSON.parse(await readFile(fixture, 'utf8'));
  assert.equal(await exists(seed.source), false, 'Original must be moved before delivery');
  assert.deepEqual(await readFile(seed.source + '.held'), content);
  const receiptDir = join(data, 'chats', 'interaction-attachments');
  const receipts = await Promise.all((await readdir(receiptDir)).filter(name => name.endsWith('.json')).map(async name => JSON.parse(await readFile(join(receiptDir, name), 'utf8'))));
  assert.equal(receipts.length, 1);
  const receipt = receipts[0];
  assert.equal(receipt.workspaceId, seed.workspaceId); assert.equal(receipt.sessionId, seed.sessionId);
  assert.equal(receipt.nativeRequestId, 7); assert.equal(receipt.itemId, 'question-item');
  assert.equal(receipt.status, 'callback-handed'); assert.equal(receipt.attachments[0].questionId, 'public');
  const rows = (await readFile(join(root, 'question-responses.jsonl'), 'utf8')).trim().split('\n').map(line => JSON.parse(line));
  assert.equal(rows.length, 1, 'Original native question must be answered exactly once');
  const row = rows[0]; assert.equal(row.id, 7); assert.equal(row.references.length, 1);
  assert.equal(row.references[0].name, 'question-proof.txt');
  assert.equal(row.references[0].path, receipt.attachments[0].filePath);
  assert.ok(row.references[0].path.startsWith(join(data, 'hive', 'uploads') + (process.platform === 'win32' ? '\\' : '/')));
  assert.equal(row.sha256, seed.sha256); assert.equal(row.bytesBase64, seed.bytesBase64);
  assert.deepEqual(await readFile(row.references[0].path), content);
  assert.match(row.result.answers.public.answers[0], /Verify fixture/);
  const requests = (await readFile(join(root, 'question-peer-requests.jsonl'), 'utf8')).trim().split('\n').map(line => JSON.parse(line));
  assert.equal(requests.filter(item => item.method === 'turn/start').length, 1);
  await writeFile(join(root, `question-${mode}.json`), JSON.stringify({ sessionId: seed.sessionId, questionId: 'public', nativeRequestId: 7,
    responseCount: rows.length, sourceMoved: true, privateSha256: row.sha256, privateBytesBase64: row.bytesBase64,
    privatePath: row.references[0].path, turnStarts: 1 }, null, 2));
}

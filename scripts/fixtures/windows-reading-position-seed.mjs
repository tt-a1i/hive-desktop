import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { DatabaseSync } from 'node:sqlite';

const [root, resources, mode = 'seed'] = process.argv.slice(2);
const state = join(root, 'state');
const project = join(root, 'project');
const info = join(root, 'archived-seed.json');
const digest = data => createHash('sha256').update(data).digest('hex');
function runs() {
  const db = new DatabaseSync(join(state, 'hive', 'runtime.sqlite'), { readOnly: true });
  try { return db.prepare('SELECT COUNT(*) AS count FROM agent_runs').get().count; }
  finally { db.close(); }
}
if (mode === 'verify' || mode === 'verify-partial') {
  const seed = JSON.parse(await readFile(info, 'utf8'));
  const original = await readFile(join(root, 'reading-original.jsonl'));
  const current = await readFile(seed.sessionFile);
  assert.ok(current.subarray(0, original.length).equals(original), 'Original transcript bytes must remain unchanged');
  const records = current.toString('utf8').split('\n').filter(Boolean).map(line => JSON.parse(line));
  assert.equal(records.filter(record => record.type === 'message').length, 81, 'No message may be appended or removed');
  for (let index = 0; index < 81; index++) {
    const entry = records.find(record => record.id === `row-${index}`);
    assert.equal(entry?.message?.content?.[0]?.text, `READING_ROW_${String(index).padStart(2, '0')}`);
  }
  const draft = JSON.parse(await readFile(join(state, 'desktop-drafts', `session-${seed.sessionId}.json`), 'utf8'));
  assert.equal(draft.input, seed.draft);
  let anchorBefore = null;
  let anchorAfter = null;
  if (mode === 'verify') {
    const entries = JSON.parse(await readFile(join(state, 'desktop-reading-positions.json'), 'utf8'));
    const anchors = entries.filter(entry => entry.workspace === seed.workspaceId && entry.session === seed.sessionId);
    assert.equal(anchors.length, 1);
    const saved = JSON.parse(await readFile(join(root, 'reading-baseline.json'), 'utf8'));
    assert.deepEqual(anchors[0].row, saved.anchor.row);
    assert.ok(Math.abs(anchors[0].offset - saved.anchor.offset) <= 2, 'Saved intra-row offset changed');
    anchorBefore = saved.anchor;
    anchorAfter = anchors[0];
  }
  assert.equal(runs(), seed.agentRunsBefore);
  await writeFile(join(root, 'archived-disk.json'), JSON.stringify({
    sessionId: seed.sessionId, originalBytes: original.length, originalSha256: digest(original),
    retainedPrefixSha256: digest(current.subarray(0, original.length)), messagesRetained: 81,
    anchorBefore, anchorAfter, anchorCompared: mode === 'verify', draftUnchanged: true,
    agentRunsBefore: seed.agentRunsBefore, agentRunsAfter: runs(),
  }, null, 2) + '\n');
} else {
  assert.equal(mode, 'seed');
  await mkdir(project, { recursive: true });
  await mkdir(join(root, 'offline-pi'), { recursive: true });
  const dist = join(resources, 'runtime', 'dist');
  const { createDesktopRuntime } = await import(pathToFileURL(join(dist, 'desktop-runtime.js')).href);
  const { ChatIndex } = await import(pathToFileURL(join(dist, 'chat-index.js')).href);
  let runtime;
  let workspace, target, other;
  try {
    runtime = await createDesktopRuntime(state, () => {});
    workspace = await runtime.hive.request('POST', '/api/workspaces', { path: project, name: 'Reading position QA', autostart_orchestrator: false });
    target = await runtime.chats.createRecord(workspace.id, 'pi');
    target = runtime.chats.rename(target.id, 'Reading target');
    other = await runtime.chats.createRecord(workspace.id, 'pi');
    other = runtime.chats.rename(other.id, 'Reading other');
    runtime.hive.saveNavigation(JSON.stringify({ workspace: workspace.id, sessions: { [workspace.id]: target.id } }));
  } finally { if (runtime) await runtime.close(); }
  const sessionFile = join(state, 'chats', `${target.id}.jsonl`);
  const entries = [{ type: 'session', id: target.id, version: 3, timestamp: '2026-11-14T00:00:00Z', cwd: project }];
  for (let index = 0; index < 81; index++) {
    entries.push({ type: 'message', id: `row-${index}`, parentId: index ? `row-${index - 1}` : null,
      timestamp: '2026-11-14T00:00:00Z', message: { role: 'user', timestamp: index,
        content: [{ type: 'text', text: `READING_ROW_${String(index).padStart(2, '0')}` }] } });
  }
  const original = Buffer.from(entries.map(entry => JSON.stringify(entry)).join('\n') + '\n');
  await writeFile(sessionFile, original);
  await writeFile(join(root, 'reading-original.jsonl'), original);
  const otherFile = join(state, 'chats', `${other.id}.jsonl`);
  await writeFile(otherFile, JSON.stringify({ type: 'session', id: other.id, version: 3, timestamp: '2026-11-14T00:00:00Z', cwd: project }) + '\n' +
    JSON.stringify({ type: 'message', id: 'other-row', parentId: null, timestamp: '2026-11-14T00:00:00Z', message: { role: 'user', timestamp: 1, content: [{ type: 'text', text: 'OTHER_CONVERSATION_SENTINEL' }] } }) + '\n');
  const index = new ChatIndex(join(state, 'chats'));
  index.save({ ...target, sessionFile });
  index.save({ ...other, sessionFile: otherFile });
  const draft = 'Reading position draft preserved';
  const draftDir = join(state, 'desktop-drafts');
  await mkdir(draftDir, { recursive: true });
  await writeFile(join(draftDir, `session-${target.id}.json`), JSON.stringify({ key: `session:${target.id}`, input: draft, failed: [] }));
  await writeFile(join(state, 'desktop-language.json'), JSON.stringify('en'));
  const agentRunsBefore = runs();
  assert.equal(agentRunsBefore, 0);
  await writeFile(info, JSON.stringify({ sessionId: target.id, workspaceId: workspace.id, otherSessionId: other.id,
    sessionFile, messageId: 'row-40', draft, worktree: project, historySha256: digest(original), agentRunsBefore }, null, 2) + '\n');
}

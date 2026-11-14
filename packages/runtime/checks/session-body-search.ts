/** Actual Hive workspaces + cold persisted transcripts. Never opens an Agent. */
import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { mkdtemp, mkdir, writeFile, readFile, appendFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { ChatIndex, type ChatRecord } from '../src/chat-index.js';
import { createDesktopRuntime } from '../src/desktop-runtime.js';

const directory = await mkdtemp(join(tmpdir(), 'hive-session-body-search-'));
const data = join(directory, 'state');
let runtime: Awaited<ReturnType<typeof createDesktopRuntime>> | undefined;
let events = 0;
try {
  const projects = [join(directory, '甲 project'), join(directory, '乙 project')];
  await Promise.all(projects.map(project => mkdir(project)));
  runtime = await createDesktopRuntime(data, () => events++);
  const workspaces: string[] = [];
  for (const [i, path] of projects.entries()) {
    const workspace = await runtime.hive.request('POST', '/api/workspaces', {
      path, name: `Corpus ${i}`, autostart_orchestrator: false,
    }) as {id: string};
    workspaces.push(workspace.id);
  }
  await runtime.close(); runtime = undefined;
  const index = new ChatIndex(join(data, 'chats'));
  const records: ChatRecord[] = [];
  async function seed(workspace: number, title: string, body: string, archived = false) {
    const id = randomUUID();
    const sessionFile = join(data, 'chats', `${id}.jsonl`);
    const record: ChatRecord = {id, workspaceId: workspaces[workspace]!, title, backend: 'pi',
      sessionFile, updatedAt: records.length, ...(archived && {archived})};
    await writeFile(sessionFile, [
      {type: 'session', id, version: 3, timestamp: '2026-11-14T00:00:00Z', cwd: projects[workspace]},
      {type: 'message', id: `${id}-message`, parentId: null, timestamp: '2026-11-14T00:00:00Z',
        message: {role: 'user', content: [{type: 'text', text: body}], timestamp: 1}},
    ].map(value => JSON.stringify(value)).join('\n') + '\n');
    records.push(record); index.save(record); return record;
  }
  for (let i = 0; i < 101; i++) await seed(0, `Ordinary ${i}`, 'Needle marker 世界 café');
  await seed(1, 'Other project', 'Needle marker 世界 café');
  const metadata = await seed(1, 'Metadata-only label', 'unrelated body');
  const archived = await seed(0, 'Archived ordinary', 'Needle marker 世界 café', true);
  const broken = await seed(0, 'Broken history', 'old');
  await writeFile(broken.sessionFile!, '{broken\n');
  const before = await readFile(join(data, 'chats', 'index.json'), 'utf8');
  runtime = await createDesktopRuntime(data, () => events++);
  let page = await runtime.chats.search('needle 世界', false, 0, workspaces[0]);
  assert.equal(page.total, 101);
  assert.equal(page.matches.length, 100);
  assert.equal(page.incompleteHistoryCount, 1);
  assert.equal(page.incompleteHistory[0]?.sessionId, broken.id);
  for (const hit of page.matches) {
    assert.equal(hit.session.workspaceId, workspaces[0]);
    assert.equal(hit.bodyMatch?.messageId, `${hit.session.id}-message`);
    assert.match(hit.bodyMatch!.snippet, /Needle marker 世界/);
  }
  const second = await runtime.chats.search('needle 世界', false, 100, workspaces[0]);
  assert.equal(second.matches.length, 1);
  assert.equal(new Set([...page.matches, ...second.matches].map(hit => hit.session.id)).size, 101);
  assert.equal((await runtime.chats.search('needle 世界')).total, 102);
  assert.equal((await runtime.chats.search('cafe\u0301', false, 0, workspaces[0])).total, 101);
  assert.equal((await runtime.chats.search('Metadata-only')).matches[0]?.session.id, metadata.id);
  const archives = await runtime.chats.search('needle 世界', true, 0, workspaces[0]);
  assert.deepEqual(archives.matches.map(hit => hit.session.id), [archived.id]);
  assert.equal(archives.incompleteHistoryCount, 0);
  const absent = await runtime.chats.search('genuinely-absent', false, 0, workspaces[0]);
  assert.equal(absent.total, 0);
  assert.equal(absent.incompleteHistoryCount, 1, 'Unreadable history cannot silently become no matches');
  assert.equal((await runtime.chats.search('', false, 0, workspaces[0])).incompleteHistoryCount, 0);
  assert.equal((await runtime.chats.search('needle', false, 0, 'missing-workspace')).incompleteHistoryCount, 0);
  const changed = records[0]!;
  await appendFile(changed.sessionFile!, JSON.stringify({type: 'message', id: 'fresh-message',
    parentId: `${changed.id}-message`, timestamp: '2026-11-14T01:00:00Z',
    message: {role: 'assistant', content: [{type: 'text', text: 'newly appended unique body'}], timestamp: 2}}) + '\n');
  page = await runtime.chats.search('appended unique', false, 0, workspaces[0]);
  assert.equal(page.total, 1);
  assert.equal(page.matches[0]?.bodyMatch?.messageId, 'fresh-message');
  assert.equal(events, 0, 'Searching must not open, start or execute any Agent');
  assert.equal(await readFile(join(data, 'chats', 'index.json'), 'utf8'), before);
  await runtime.close(); runtime = undefined;
  runtime = await createDesktopRuntime(data, () => events++);
  const cold = await runtime.chats.search('appended unique', false, 0, workspaces[0]);
  assert.equal(cold.matches[0]?.bodyMatch?.messageId, 'fresh-message');
  assert.equal(events, 0);
  console.log('SESSION_BODY_SEARCH_OK: cold corpus, full body, Unicode, scope, archives, pagination, invalidation, warnings; no Agents/model calls.');
} finally {
  await runtime?.close();
  await rm(directory, {recursive: true, force: true});
}

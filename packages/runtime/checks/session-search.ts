/** Real Hive metadata and durable records. Search must not load any Agent session. */
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createDesktopRuntime } from '../src/desktop-runtime.js';
import type { ChatBackend } from '../src/chat-index.js';

const directory = await mkdtemp(join(tmpdir(), 'hive-session-search-'));
let runtime: Awaited<ReturnType<typeof createDesktopRuntime>> | undefined;
let events = 0;
try {
  runtime = await createDesktopRuntime(join(directory, 'state'), () => { events++; });
  await mkdir(join(directory, 'alpha'));
  const projects = [];
  for (const name of ['Beta 项目', 'Alpha 项目']) {
    projects.push(await runtime.hive.request('POST', '/api/workspaces', {
      path: name.startsWith('Beta') ? directory : join(directory, 'alpha'),
      name, autostart_orchestrator: false,
    }) as { id: string });
  }
  const ids = new Set<string>();
  for (const [i, backend] of (['pi', 'codex', 'pi-rpc', 'claude'] satisfies ChatBackend[]).entries()) {
    const record = await runtime.chats.createRecord(projects[i % 2].id, backend);
    runtime.chats.rename(record.id, backend + ' 发布核验 🐝');
    ids.add(record.id);
  }
  const before = projects.map(project => runtime!.chats.list(project.id));
  const all = await runtime.chats.search('');
  assert.deepEqual(new Set(all.matches.map(hit => hit.session.id)), ids);
  assert.equal(all.total, 4);
  assert.equal((await runtime.chats.search('BETA 发布')).total, 2);
  assert.deepEqual((await runtime.chats.search('aLpHa CODEX')).matches.map(hit => hit.session.backend), ['codex']);
  assert.equal((await runtime.chats.search('完全不存在')).total, 0);
  assert.equal((await runtime.chats.search('🐝')).total, 4);
  assert.equal((await runtime.chats.search('外部 Pi')).total, 1);
  assert.equal((await runtime.chats.search('Claude Code')).total, 1);
  assert.equal((await runtime.chats.search('内置 Pi')).total, 1);
  for (const invalid of [undefined, {}, 'x'.repeat(201)]) await assert.rejects(runtime.chats.search(invalid));
  assert.deepEqual(projects.map(project => runtime!.chats.list(project.id)), before);
  assert.equal(events, 0, 'Searching cold records must not load or execute an Agent');

  await runtime.close(); runtime = undefined;
  runtime = await createDesktopRuntime(join(directory, 'state'), () => { events++; });
  assert.deepEqual(new Set((await runtime.chats.search('发布')).matches.map(hit => hit.session.id)), ids);
  assert.equal(events, 0);
  const member = await runtime.hive.request('POST', `/api/workspaces/${projects[0].id}/workers`, { name: 'Original name', role: 'reviewer', autostart: false }) as { id: string };
  await runtime.hive.request('POST', `/api/workspaces/${projects[0].id}/agents/${member.id}/config`,
    { command: 'hive:pi', args: ['--provider', 'openai', '--model', 'gpt-6.1-terra'] });
  await runtime.hive.request('POST', `/api/workspaces/${projects[0].id}/agents/${member.id}/start`);
  await runtime.hive.request('PATCH', `/api/workspaces/${projects[0].id}/workers/${member.id}`, { name: '中文审查员' });
  const eventsBeforeSearch = events;
  const membersBeforeSearch = await runtime.hive.request('GET', `/api/ui/workspaces/${projects[0].id}/team`);
  const renamedMember = await runtime.chats.search('中文审查员');
  assert.equal(renamedMember.matches.length, 1);
  assert.equal(renamedMember.matches[0].title, '中文审查员');
  assert.equal(renamedMember.matches[0].session.agentId, member.id);
  runtime.chats.setPinned(renamedMember.matches[0].session.id,true);
  const pinnedMember=await runtime.chats.pinned();
  assert.equal(pinnedMember.length,1);
  assert.equal(pinnedMember[0].title,'中文审查员');
  assert.equal(pinnedMember[0].session.agentId,member.id);
  assert.deepEqual(await runtime.hive.request('GET', `/api/ui/workspaces/${projects[0].id}/team`), membersBeforeSearch);
  assert.equal(events, eventsBeforeSearch);
  for (let i = 0; i < 101; i++) {
    const record = await runtime.chats.createRecord(projects[0].id);
    runtime.chats.rename(record.id, '分页查找 ' + i);
    runtime.chats.setPinned(record.id,true);
  }
  const limited = await runtime.chats.search('分页查找');
  assert.equal(limited.total, 101);
  assert.equal(limited.matches.length, 100);
  assert.equal((await runtime.chats.search('分页查找 100')).matches[0].session.title, '分页查找 100');
  const pins=await runtime.chats.pinned();
  assert.equal(pins.length,102,'Global pins must not inherit the search result cap');
  assert.equal(new Set(pins.map(hit=>hit.session.id)).size,102);
  assert.equal(pins.filter(hit=>hit.session.agentId===member.id).length,1);
  await runtime.close();runtime=undefined;
  runtime=await createDesktopRuntime(join(directory,'state'),()=>{events++;});
  assert.deepEqual(await runtime.chats.pinned(),pins,'Global pinned identities and member titles survive restart');
  await runtime.removeWorkspace(projects[0].id);
  assert.deepEqual(await runtime.chats.pinned(),[],'Removed workspaces must disappear from global pins');
  console.log('SESSION_SEARCH_OK: cross-project metadata, all backends, restart, result limit and no Agent initialization.');
} finally {
  await runtime?.close();
  await rm(directory, { recursive: true, force: true });
}

/** Actual Hive workspaces + cold persisted transcripts. Never opens an Agent. */
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { mkdtemp, mkdir, writeFile, readFile, appendFile, rename, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { ChatIndex, type ChatRecord } from '../src/chat-index.js';
import { createDesktopRuntime } from '../src/desktop-runtime.js';
import { createDesktopSessionManager } from '../src/agents/pi-session-manager.js';

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
  const metadata = await seed(1, 'Re\u0301sume\u0301-only label', 'unrelated body');
  const archived = await seed(0, 'Archived ordinary', 'İ'.repeat(400) + ' Needle marker 世界 café ' + 'z'.repeat(500), true);
  const broken = await seed(0, 'Broken history', 'old');
  await writeFile(broken.sessionFile!, '{broken\n');
  const legacy = JSON.parse(await readFile(join(data, 'chats', 'index.json'), 'utf8')) as ChatRecord[];
  runtime = await createDesktopRuntime(data, () => events++);
  // Startup migrates legacy configuration references before serving requests.
  // Search must leave that initialized index unchanged, including cold reads.
  const before = await readFile(join(data, 'chats', 'index.json'), 'utf8');
  const initialized = JSON.parse(before) as ChatRecord[];
  assert.equal(initialized.length, legacy.length);
  for (const [i, entry] of initialized.entries()) {
    const {profile, legacyConfiguration, ...original} = entry;
    assert.deepEqual(profile, {id: 'default-pi', revision: 1});
    assert.equal(legacyConfiguration, true);
    assert.deepEqual(original, legacy[i]);
  }
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
  assert.equal((await runtime.chats.search('Résumé-only')).matches[0]?.session.id, metadata.id);
  const archives = await runtime.chats.search('needle 世界', true, 0, workspaces[0]);
  assert.deepEqual(archives.matches.map(hit => hit.session.id), [archived.id]);
  assert.equal(archives.incompleteHistoryCount, 0);
  assert.equal(archives.matches[0]?.bodyMatch?.messageId, `${archived.id}-message`);
  assert.ok(archives.matches[0]?.bodyMatch?.snippet.includes('Needle marker 世界 café'));
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
  assert.equal(await readFile(join(data, 'chats', 'index.json'), 'utf8'), before);

  // The saved transcript belongs to the conversation, even while its real Git
  // worktree is missing. Reading must not grant permission to run at the root.
  const project = projects[0]!;
  const git = (...args: string[]) => execFileSync('git', ['-C', project, ...args], {stdio: 'pipe'});
  git('init', '--template=');
  await writeFile(join(project, 'tracked.txt'), 'original project\n');
  git('add', 'tracked.txt');
  git('-c', 'user.name=Fixture', '-c', 'user.email=qa@example.invalid', '-c', 'commit.gpgSign=false', 'commit', '-m', 'fixture');
  const owned = await runtime.chats.createRecord(workspaces[0]!, 'pi', {isolation: 'worktree'});
  const {path: executionPath} = await runtime.chats.sessionExecution(owned.id);
  const sessionFile = join(data, 'chats', `${owned.id}.jsonl`);
  await writeFile(sessionFile, [
    {type: 'session', id: owned.id, version: 3, timestamp: '2026-11-14T00:00:00Z', cwd: executionPath},
    {type: 'message', id: 'owned-message', parentId: null, timestamp: '2026-11-14T00:00:00Z',
      message: {role: 'user', content: [{type: 'text', text: 'recoverable worktree transcript'}], timestamp: 1}},
  ].map(value => JSON.stringify(value)).join('\n') + '\n');
  await runtime.close(); runtime = undefined;
  new ChatIndex(join(data, 'chats')).save({...owned, sessionFile});
  await rename(executionPath, executionPath + '.fixture-missing');
  runtime = await createDesktopRuntime(data, () => events++);
  const saved = await runtime.chats.search('recoverable worktree transcript', false, 0, workspaces[0]);
  assert.equal(saved.matches[0]?.session.id, owned.id);
  assert.equal(saved.matches[0]?.bodyMatch?.messageId, 'owned-message');
  assert.ok(!saved.incompleteHistory.some(item => item.sessionId === owned.id));
  const snapshot = await runtime.chats.open(owned.id);
  assert.ok('readOnly' in snapshot && snapshot.readOnly);
  assert.equal(snapshot.executionPath, executionPath);
  assert.deepEqual(snapshot.historyMessageIds, ['owned-message']);
  await assert.rejects(runtime.chats.sessionExecution(owned.id));
  await assert.rejects(runtime.chats.prompt(owned.id, 'Sending must stay blocked'));
  await assert.rejects(runtime.chats.projectFilesSearch(owned.workspaceId, 'tracked', 0, owned.id));
  assert.equal(await readFile(join(project, 'tracked.txt'), 'utf8'), 'original project\n');
  assert.equal(events, 0, 'Cold read of a missing worktree must not start an Agent');
  await rename(executionPath + '.fixture-missing', executionPath);
  assert.equal((await runtime.chats.sessionExecution(owned.id)).path, executionPath);
  assert.deepEqual((await runtime.chats.projectFilesSearch(owned.workspaceId, 'tracked', 0, owned.id)).paths, ['tracked.txt']);

  const transcripts = join(directory, 'native-pi');
  const original = createDesktopSessionManager(executionPath, transcripts);
  const originalFile = original.getSessionFile()!;
  const originalId = original.getSessionId();
  const appendAnswer = (manager: typeof original, text: string) => manager.appendMessage({
    role: 'assistant', content: [{type: 'text', text}], api: 'openai-responses', model: 'fixture', provider: 'fixture',
    usage: {input: 0, output: 0, cacheRead: 0, cacheWrite: 0, totalTokens: 0, cost: {input: 0, output: 0, cacheRead: 0, cacheWrite: 0, total: 0}},
    stopReason: 'stop', timestamp: 2,
  });
  const firstId = original.appendMessage({role: 'user', content: 'Original branch', timestamp: 1});
  appendAnswer(original, 'Original continuation');
  const branchFile = original.createBranchedSession(firstId)!;
  const branch = createDesktopSessionManager(executionPath, transcripts, branchFile);
  const foreignText = 'Foreign branch search marker';
  const foreignId = appendAnswer(branch, foreignText);
  assert.notEqual(originalId, branch.getSessionId());
  const foreignBytes = await readFile(branchFile);
  const validBranch = await runtime.chats.createRecord(workspaces[0]!, 'pi');
  const external = await runtime.chats.createRecord(workspaces[0]!, 'pi-rpc');
  const {profile: acpProfile} = await runtime.profiles.create({
    driver: 'acp', label: 'Missing transcript check',
    launch: {command: process.execPath, args: ['-e', "require('node:fs').writeFileSync(process.argv[1], 'unexpected ACP process')", join(directory, 'acp-started')]},
  }, (await runtime.profiles.list()).storeVersion);
  const missingAcp = await runtime.chats.createRecord(workspaces[0]!, 'acp', {profile: acpProfile.ref});
  const newAcp = await runtime.chats.createRecord(workspaces[0]!, 'acp', {profile: acpProfile.ref});
  await runtime.close(); runtime = undefined;
  const sources = new ChatIndex(join(data, 'chats'));
  sources.save({...sources.get(owned.id), sessionFile: originalFile, piSessionId: originalId});
  sources.save({...validBranch, sessionFile: branchFile, piSessionId: branch.getSessionId()});
  sources.save({...external, sessionFile: branchFile, externalPiSessionId: originalId});
  sources.save({...missingAcp, acpSessionId: 'saved-acp-id'});
  await writeFile(originalFile, foreignBytes);
  await rename(executionPath, executionPath + '.fixture-missing');
  runtime = await createDesktopRuntime(data, () => events++);
  page = await runtime.chats.search(foreignText, false, 0, workspaces[0]);
  assert.deepEqual(page.matches.map(hit => [hit.session.id, hit.bodyMatch?.messageId]), [[validBranch.id, foreignId]]);
  for (const id of [owned.id, external.id])
    assert.match(page.incompleteHistory.find(item => item.sessionId === id)?.message ?? '', /original native session ID/);
  assert.match(page.incompleteHistory.find(item => item.sessionId === missingAcp.id)?.message ?? '', /no local transcript path/);
  assert.ok(!page.incompleteHistory.some(item => item.sessionId === newAcp.id));
  const unavailable = await runtime.chats.open(owned.id);
  assert.ok('readOnly' in unavailable && unavailable.readOnly);
  assert.match(unavailable.historyWarning ?? '', /original native session ID/);
  assert.deepEqual(unavailable.historyMessageIds, []);
  await rename(executionPath + '.fixture-missing', executionPath);
  const wrongIdentity = await runtime.chats.open(owned.id);
  assert.ok('readOnly' in wrongIdentity && wrongIdentity.readOnly);
  assert.equal(wrongIdentity.identityError?.code, 'PI_SESSION_IDENTITY_CONFLICT');
  assert.equal(wrongIdentity.piSessionId, originalId);
  assert.deepEqual(wrongIdentity.historyMessageIds, []);
  await assert.rejects(runtime.chats.prompt(owned.id, 'Must not run in the wrong native session'), /PI_SESSION_IDENTITY_CONFLICT/);
  assert.deepEqual(await readFile(originalFile), foreignBytes);
  const lostAcp = await runtime.chats.open(missingAcp.id);
  assert.ok('readOnly' in lostAcp && lostAcp.readOnly);
  assert.match(lostAcp.historyWarning ?? '', /no local transcript path/);
  await assert.rejects(runtime.chats.resume(missingAcp.id), /ACP_SESSION_IDENTITY_CONFLICT/);
  await assert.rejects(readFile(join(directory, 'acp-started')), {code: 'ENOENT'});
  assert.equal(events, 0, 'Opening and searching foreign or missing sources must not activate an Agent');
  console.log('SESSION_BODY_SEARCH_OK: cold corpus, full body, Unicode, scope, archives, pagination, invalidation, warnings, offline worktree and native transcript identity; no Agents/model calls.');
} finally {
  await runtime?.close();
  await rm(directory, {recursive: true, force: true});
}

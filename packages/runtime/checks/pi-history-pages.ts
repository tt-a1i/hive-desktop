import assert from 'node:assert/strict';
import { appendFile, mkdir, mkdtemp, realpath, rename, rm, stat, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { performance } from 'node:perf_hooks';
import { createDesktopRuntime } from '../src/desktop-runtime.js';
import type { ChatIndex } from '../src/chat-index.js';
import { PiHistoryPages } from '../src/agents/pi-history-pages.js';
import { externalPiBranch, externalPiMessage, piHistoryMessageId, readPiTranscript } from '../src/agents/external-pi-history.js';
import { piUserInputNote, piUserInputSources } from '../src/agents/pi-user-input.js';
import { desktopHistoryImage } from '../src/desktop-history.js';
import { readToolOutputByteWindow, sliceHistoryWindow, StaleHistorySnapshotError, type HistoryWindowOptions } from '../src/history-cursor.js';

const root = await realpath(await mkdtemp(join(tmpdir(), 'hive-pi-history-pages-')));
const project = join(root, 'project');
const file = join(root, 'native.jsonl');
const nativeId = 'native-page-fixture';
const toolText = '汉🚀'.repeat(27_000);
const secondToolBlock = '第二段🌐'.repeat(700);
const timestamp = '2026-11-21T09:00:00.000Z';
const header = () => ({type: 'session', id: nativeId, version: 3, timestamp, cwd: project});
const line = (row: unknown) => JSON.stringify(row) + '\r\n';
const entry = (id: string, parentId: string | null, fields: object) => ({id, parentId, timestamp, ...fields});
const marker = (deliveryId: string) => ({type: 'custom', customType: 'hive-user-input', data: {
  version: 1, origin: 'user', deliveryId, displayText: deliveryId, rawText: deliveryId,
}});
const message = (role: string, content: unknown, extra: object = {}) => ({type: 'message', message: {role, content, timestamp, ...extra}});
const rows = [
  entry('orphan', null, message('assistant', 'This is a separate root.')),
  entry('m0', null, message('user', 'Start')),
  entry('marker1', 'm0', marker('delivery-with-image')),
  entry('first-call', 'marker1', message('assistant', [{type: 'toolCall', id: 'unfinished', name: 'read'}])),
  entry('m1', 'first-call', message('user', [{type: 'image', mimeType: 'image/png', data: 'QUJD'}])),
  entry('marker2', 'm1', marker('handled-locally')),
  entry('handled', 'marker2', {type: 'custom', customType: 'hive-user-input-result', data: {deliveryId: 'handled-locally'}}),
  entry('discarded', 'm1', message('user', 'This abandoned branch must be absent.')),
  entry('call', 'handled', message('assistant', [{type: 'toolCall', id: 'tool-done', name: 'shell'}])),
  entry('result', 'call', message('toolResult', [{type: 'text', text: toolText}, {type: 'text', text: secondToolBlock}], {toolCallId: 'tool-done', isError: true})),
  ...Array.from({length: 360}, (_, index) =>
    entry('row-' + index, index === 0 ? 'result' : 'row-' + (index - 1),
      message('assistant', 'Chunk ' + index + ' ' + '日'.repeat(12_200)))),
  entry('tail', 'row-359', message('assistant', 'Text at EOF')),
];

let runtime: Awaited<ReturnType<typeof createDesktopRuntime>> | undefined;
try {
  await mkdir(project);
  await writeFile(file, line(header()) + '\r\n' + rows.map(line).join('').trimEnd());
  const fullBytes = (await stat(file)).size;
  const entries = await readPiTranscript(file, nativeId);
  const branch = externalPiBranch(entries, entries.at(-1)!.id);
  const inputs = piUserInputSources(branch);
  for (const backend of ['pi', 'pi-rpc'] as const) {
    const expected: unknown[] = [];
    const ids: string[] = [];
    for (const row of branch) {
      const note = inputs.notes.get(row.id);
      if (note) { expected.push(piUserInputNote(row, note, backend)); ids.push(note.deliveryId); }
      const display = externalPiMessage(row, inputs.users.get(row.id), backend);
      if (display) { expected.push(display); ids.push(piHistoryMessageId(row.id, display)); }
    }
    let indexingBytes = 0;
    let pageBytes = 0;
    const reader = new PiHistoryPages((phase, bytes) => {
      if (phase === 'index') indexingBytes += bytes;
      else pageBytes += bytes;
    });
    const source = {file, nativeId, backend, scope: backend + '-workspace/session'};
    const coldStart = performance.now();
    const [initial, same] = await Promise.all([reader.page(source, {limit: 3}), reader.page(source, {limit: 3})]);
    const coldDurationMs = Math.round((performance.now() - coldStart) * 100) / 100;
    assert.deepEqual(initial, same);
    assert.ok(initial.historyPage.snapshot);
    const snapshot = initial.historyPage.snapshot;
    const compare = async (options: HistoryWindowOptions) => {
      const {history, historyMessageIds, historyPage: {snapshot: version, ...historyPage}} = await reader.page(source, options);
      assert.equal(version, snapshot);
      assert.deepEqual({history, historyMessageIds, historyPage}, sliceHistoryWindow(expected, ids, options));
    };
    await compare({limit: 3});
    assert.equal(indexingBytes, fullBytes, 'A cold build reads the native journal once.');
    pageBytes = 0;
    const started = performance.now();
    const cursorOptions = {limit: 3, snapshot, beforeIndex: initial.historyPage.startIndex, beforeMessageId: initial.historyMessageIds[0]!};
    await compare(cursorOptions);
    await compare({limit: 2, afterIndex: 3, afterMessageId: ids[3]!, snapshot});
    for (const aroundMessageId of ['delivery-with-image', 'handled-locally', 'unfinished', 'tool-done', 'discarded']) {
      await compare({limit: 1, aroundMessageId, snapshot});
    }
    assert.equal(indexingBytes, fullBytes, 'Warm pages perform no additional journal scan.');
    assert.ok(pageBytes < fullBytes / 12, 'Warm pages read only selected rows and attached markers.');
    const warmPageReadBytes = pageBytes;
    const warmPageDurationMs = Math.round((performance.now() - started) * 100) / 100;
    pageBytes = 0;
    const imageRow = await reader.imageRow(source, backend === 'pi' ? 'delivery-with-image' : 'm1');
    assert.equal(desktopHistoryImage(imageRow, 0), 'QUJD');
    assert.equal(indexingBytes, fullBytes);
    const imageReadBytes = pageBytes;
    assert.ok(imageReadBytes < 1024);
    await assert.rejects(reader.page(source, {...cursorOptions, beforeMessageId: 'm0'}), StaleHistorySnapshotError);
    await assert.rejects(reader.page({...source, scope: 'another session'}, cursorOptions), StaleHistorySnapshotError);
    await assert.rejects(reader.page({...source, nativeId: 'different-native-id'}, {limit: 1}), /native session ID/);
    await appendFile(file, '\r\n' + line(entry('append', 'tail', message('assistant', 'Appended'))));
    await assert.rejects(reader.page(source, cursorOptions), StaleHistorySnapshotError);
    const appended = await reader.page(source, {limit: 1});
    assert.deepEqual(appended.historyMessageIds, ['append']);
    await appendFile(file, line(entry('duplicate-marker', 'append', marker('delivery-with-image')))
      + line(entry('duplicate-user', 'duplicate-marker', message('user', 'Duplicate alias'))));
    assert.equal(await reader.imageRow(source, 'delivery-with-image'), undefined);
    assert.equal(desktopHistoryImage(await reader.imageRow(source, 'm1'), 0), 'QUJD');
    await appendFile(file, line(entry('new-branch', 'm0', message('assistant', 'Changed branch'))));
    const rebranched = await reader.page(source, {limit: 20});
    assert.deepEqual(rebranched.historyMessageIds, ['m0', 'new-branch']);
    await writeFile(file, line(header()) + line(entry('reset-in-place', null, message('user', 'Truncated in place'))));
    await assert.rejects(reader.page(source, {snapshot: rebranched.historyPage.snapshot!}), StaleHistorySnapshotError);
    const truncated = await reader.page(source, {limit: 1});
    assert.deepEqual(truncated.historyMessageIds, ['reset-in-place']);
    const replacement = join(root, 'replacement.jsonl');
    await writeFile(replacement, line(header()) + line(entry('reset', null, message('user', 'Truncated'))));
    await rename(replacement, file);
    await assert.rejects(reader.page(source, {snapshot: truncated.historyPage.snapshot!}), StaleHistorySnapshotError);
    await writeFile(file, line(header()) + rows.map(line).join(''));
    console.log(JSON.stringify({backend, fullBytes, coldDurationMs, warmPageReadBytes, warmPageDurationMs, imageReadBytes}));
  }

  // The production cold reader handles initial focus, old-to-new navigation,
  // deferred image retrieval, tool output and legacy fallback without activation.
  const pi = join(root, 'pi');
  await mkdir(pi);
  process.env.PI_CODING_AGENT_DIR = pi;
  process.env.PI_OFFLINE = '1';
  runtime = await createDesktopRuntime(join(root, 'state'), () => {});
  const workspace = await runtime.hive.request('POST', '/api/workspaces', {path: project, name: 'Pi history paging', autostart_orchestrator: false}) as {id: string};
  const internals = runtime.chats as unknown as {index: ChatIndex; live: Map<string, unknown>};
  for (const backend of ['pi', 'pi-rpc'] as const) {
    const id = 'cold-' + backend;
    const native = backend === 'pi' ? {piSessionId: nativeId} : {externalPiSessionId: nativeId};
    const record = {id, workspaceId: workspace.id, backend, title: id, sessionFile: file, updatedAt: Date.now(), identityState: 'unknown' as const, ...native};
    internals.index.save(record);
    const full = await runtime.chats.open(id);
    const view = backend === 'pi' ? 10 : 20;
    const focused = await runtime.chats.openDesktopView(id, view, true, {limit: 4, aroundMessageId: 'delivery-with-image'});
    assert.equal(focused.readOnly, true);
    assert.deepEqual(focused.historyMessageIds, sliceHistoryWindow(full.history, full.historyMessageIds, {limit: 4, aroundMessageId: 'delivery-with-image'}).historyMessageIds);
    assert.ok('historyPage' in focused && focused.historyPage?.snapshot);
    const index = focused.historyPage.nextAfterIndex!;
    const after = await runtime.chats.readDesktopView(id, view, () => runtime!.chats.historyPage(id, {
      limit: 3, afterIndex: index, afterMessageId: focused.historyPage!.nextAfterMessageId!, snapshot: focused.historyPage!.snapshot!,
    }));
    assert.deepEqual(after.historyMessageIds, full.historyMessageIds.slice(index + 1, index + 4));
    assert.equal((await runtime.chats.historyImage(workspace.id, id, backend === 'pi' ? 'delivery-with-image' : 'm1', 0)).data, 'QUJD');
    assert.deepEqual(await runtime.chats.toolOutputSlice(id, {messageId: 'tool-done'}), readToolOutputByteWindow(toolText, 0));
    assert.deepEqual(await runtime.chats.toolOutputSlice(id, {messageId: 'tool-done', blockIndex: 1, byteOffset: 5, byteLength: 2048}),
      readToolOutputByteWindow(secondToolBlock, 5, 2048));
    assert.equal(internals.live.has(id), false);
  }
  internals.index.save({id:'no-native-id', workspaceId: workspace.id, backend:'pi', title:'legacy', sessionFile:file, updatedAt: Date.now(), identityState:'unknown'});
  const legacy = await runtime.chats.historyPage('no-native-id', {limit: 2});
  assert.equal(legacy.historyPage.snapshot, undefined);
  assert.deepEqual(legacy.historyMessageIds, ['row-359', 'tail']);

  const writable = await runtime.chats.createRecord(workspace.id, 'pi');
  const model = {provider:'openai', id:'gpt-6.1-terra', name:'Terra'};
  internals.index.save({...writable, sessionFile:file, piSessionId:nativeId, model,
    identityState:'known', originAgentId:`${workspace.id}:orchestrator`});
  const writableView = await runtime.chats.openDesktopView(writable.id, 30, true, {limit: 3, aroundMessageId:'tool-done'});
  assert.equal(writableView.readOnly, false);
  assert.ok('savedHistoryPassive' in writableView && writableView.savedHistoryPassive);
  assert.ok('passive' in writableView && writableView.passive);
  assert.ok('historyPage' in writableView && writableView.historyPage?.snapshot);
  assert.deepEqual(writableView.model, model);
  assert.equal(writableView.history.length, 3);
  assert.equal(internals.live.has(writable.id), false);
  const readDesktop = <T>(action: () => Promise<T>) => runtime!.chats.readDesktopView(writable.id, 30, action);
  assert.deepEqual((await readDesktop(() => runtime!.chats.mode(writable.id, undefined, false, true))).options, []);
  assert.deepEqual((await readDesktop(() => runtime!.chats.reasoning(writable.id, undefined, true))).model, model);
  assert.equal((await runtime.chats.commands(writable.id)).supported, false);
  assert.deepEqual((await runtime.chats.usage(writable.id)).reports, []);
  assert.deepEqual(await runtime.chats.toolOutputSlice(writable.id, {messageId:'tool-done'}), readToolOutputByteWindow(toolText, 0));
  assert.equal((await runtime.chats.historyImage(workspace.id, writable.id, 'delivery-with-image', 0)).data, 'QUJD');
  assert.equal(internals.live.has(writable.id), false);
  console.log('Pi history paging matched the full reader and stayed offline.');
} finally {
  await runtime?.close();
  await rm(root, {recursive:true, force:true});
}

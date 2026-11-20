/** Official SDK file reader against isolated synthetic transcripts. */
import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { appendFile, mkdir, mkdtemp, readFile, readdir, realpath, rm, stat, symlink, utimes, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { DatabaseSync } from 'node:sqlite';
import { promisify } from 'node:util';
import { sliceHistoryWindow, StaleHistorySnapshotError } from '../src/history-cursor.js';
import { claudeHistoryCacheDirectory, openClaudeSavedHistory, removeClaudeHistoryCache } from '../src/agents/claude-history-pages.js';
import { writeClaudeMetadata, type ClaudeMetadata } from '../src/agents/claude-metadata.js';
import { ClaudeProjection } from '../src/agents/claude-projection.js';
import { resolveClaudeTranscript } from '../src/agents/claude-transcript-source.js';
import { readSessionMessages } from '../src/agents/claude.js';

type Options = Parameters<typeof openClaudeSavedHistory>[0];
if (process.argv[2] === 'warm-index') {
  const input = JSON.parse(process.argv[3]!) as { options: Options; id: string };
  const source = await openClaudeSavedHistory(input.options);
  assert.ok(source, 'a cold application can open the unchanged display index with the SDK subprocess blocked');
  const page = await source.page({limit: 2});
  assert.equal(page.history.length, 2);
  const exact = await source.byId(input.id) as {content: {text: string}[]};
  console.log(JSON.stringify({ids: page.historyMessageIds, bytes: Buffer.byteLength(JSON.stringify(page)),
    total: page.historyPage.totalCount, toolBytes: Buffer.byteLength(exact.content[0]!.text)}));
} else {
  await verify();
}

async function verify() {
  const root = await realpath(await mkdtemp(join(tmpdir(), 'hive-claude-saved-pages-')));
  try {
    const cwd = join(root, 'workspace');
    const config = join(root, 'config');
    const projectKey = cwd.replace(/[^a-zA-Z0-9]/g, '-');
    const projectDir = join(config, 'projects', projectKey);
    await mkdir(cwd);
    await mkdir(projectDir, {recursive: true});
    const sessionId = randomUUID();
    const native = join(projectDir, `${sessionId}.jsonl`);
    const env = {...process.env, CLAUDE_CONFIG_DIR: config};
    delete env.CLAUDE_CODE_PROJECT_DIR_NAME;
    const workspaceId = 'claude-workspace';
    const options: Options = {cacheDirectory: claudeHistoryCacheDirectory(root, workspaceId), metadataDirectory: join(root, 'hive-metadata'),
      sessionId, scope: 'fixture-profile/revision/conversation', cwd, env, nativePath: native};
    let timestamp = Date.UTC(2026, 10, 20, 1);
    let parentUuid: string | null = null;
    const rows: unknown[] = [];
    const add = (type: 'user' | 'assistant', content: unknown, messageId?: string, sidechain = false) => {
      const uuid = randomUUID();
      rows.push({type, uuid, parentUuid, sessionId, cwd, isSidechain: sidechain, timestamp: new Date(timestamp++).toISOString(),
        message: {role: type, ...(messageId && {id: messageId, model: 'fixture-model'}), content}});
      parentUuid = uuid;
      return uuid;
    };
    const encode = (entries: readonly unknown[]) => entries.map(row => JSON.stringify(row) + '\n').join('');
    const first = add('user', 'first native input');
    const abandoned = add('assistant', [{type: 'text', text: 'abandoned branch'}], 'abandoned');
    parentUuid = first;
    const delivery = add('user', 'delivered input');
    const fragmented = add('assistant', [{type: 'thinking', thinking: 'thinking'}], 'fragmented');
    const firstToolCall = 'tool-use-with-result';
    const unresolved = 'tool-use-without-result';
    const fragmentTail = add('assistant', [
      {type: 'tool_use', id: firstToolCall, name: 'fixture', input: {only: 'local'}},
      {type: 'tool_use', id: unresolved, name: 'fixture', input: {only: 'local'}},
    ], 'fragmented');
    const toolText = '输出🙂'.repeat(18_000);
    add('user', [{type: 'tool_result', tool_use_id: firstToolCall, content: toolText, is_error: false},
      {type: 'text', text: 'a mixed user record'}]);
    for (let i = 0; i < 170; i++) {
      add('assistant', [{type: 'text', text: `old fixture row ${i} ${'x'.repeat(24_000)}`}], `older-${i}`);
    }
    const latest = [add('user', 'latest question'), add('assistant', [{type: 'text', text: 'latest answer'}], 'final')];
    const lastMain = parentUuid;
    const sidechain = add('assistant', [{type: 'text', text: 'ignore sidechain'}], 'sidechain', true);
    parentUuid = lastMain;
    await writeFile(native, encode(rows));
    const metadata: ClaudeMetadata = {version: 1, context: [], completed: [], deliveries: [
      {uuid: delivery, deliveryId: 'display-delivery-id', presentation: {version: 1, origin: 'dispatch', displayText: 'fixture dispatch'}},
    ]};
    writeClaudeMetadata(options.metadataDirectory, sessionId, metadata);
    const before = await stat(native);
    const official = await readSessionMessages(sessionId, cwd, env);
    assert.ok(official.some(row => row.uuid === fragmentTail));
    assert.ok(!official.some(row => row.uuid === abandoned || row.uuid === sidechain), 'use the SDK-selected branch');
    const projection = new ClaudeProjection(() => metadata);
    for (const row of official) projection.put(row);
    const full = projection.snapshot();

    const source = await openClaudeSavedHistory(options);
    assert.ok(source);
    const latestPage = await source.page({limit: 2});
    assert.deepEqual(latestPage.historyMessageIds, latest);
    assert.equal(latestPage.historyPage.totalCount, full.history.length);
    for (const window of [{limit: 3, aroundMessageId: unresolved}, {limit: 3, aroundMessageId: 'display-delivery-id'},
      {limit: 5, beforeIndex: latestPage.historyPage.startIndex, beforeMessageId: latest[0]!}]) {
      const expected = sliceHistoryWindow(full.history, full.historyMessageIds, window);
      assert.deepEqual(await source.page({...window, snapshot: source.snapshot}),
        {...expected, historyPage: {...expected.historyPage, snapshot: source.snapshot}});
    }
    const merged = await source.byId(fragmented) as {source: {nativeUuids: string[]}};
    assert.deepEqual(merged.source.nativeUuids, [fragmented, fragmentTail]);
    const earlier = await source.page({limit: 4, aroundMessageId: delivery, snapshot: source.snapshot});
    const forward = await source.page({limit: 2, afterIndex: earlier.historyPage.nextAfterIndex!,
      afterMessageId: earlier.historyPage.nextAfterMessageId!, snapshot: source.snapshot});
    assert.equal(forward.historyPage.startIndex, earlier.historyPage.nextAfterIndex! + 1);
    await assert.rejects(source.page({beforeIndex: latestPage.historyPage.startIndex, beforeMessageId: first,
      snapshot: source.snapshot}), StaleHistorySnapshotError);
    assert.equal(await source.byId(fragmentTail), undefined);

    // Block SDK initialization on subsequent reads. A freshly started process
    // must seek the persisted index, and avoid deserializing an unrelated row.
    const guard = join(root, 'block-sdk-child.cjs');
    await writeFile(guard, "throw new Error('SDK subprocess must not start for an indexed page');\n");
    const noSdkEnv = {...env, NODE_OPTIONS: `--require "${guard}"`};
    const artifacts = await readdir(options.cacheDirectory);
    assert.equal(artifacts.length, 1);
    assert.match(artifacts[0]!, /\.index$/);
    const database = new DatabaseSync(join(options.cacheDirectory, artifacts[0]!));
    let plan: string;
    try {
      plan = String(database.prepare('EXPLAIN QUERY PLAN SELECT id, body FROM messages WHERE position >= ? AND position < ? ORDER BY position').get(0, 2)?.detail);
      assert.match(plan, /SEARCH messages USING INTEGER PRIMARY KEY/);
      database.prepare('UPDATE messages SET body = ? WHERE id = ?').run('unused row must not be parsed', first);
    } finally { database.close(); }
    const sourceLoader = import.meta.filename.endsWith('.ts') ? ['--import', 'tsx'] : [];
    const {stdout} = await promisify(execFile)(process.execPath, [...sourceLoader, import.meta.filename, 'warm-index',
      JSON.stringify({options: {...options, env: noSdkEnv}, id: firstToolCall})]);
    const cold = JSON.parse(stdout) as {ids: string[]; bytes: number; toolBytes: number; total: number};
    assert.deepEqual(cold.ids, latest);
    assert.equal(cold.total, full.history.length);
    assert.equal(cold.toolBytes, Buffer.byteLength(toolText));
    assert.ok(cold.bytes < 4096 && before.size > 4 * 1024 * 1024);
    assert.equal(await readFile(native, 'utf8'), encode(rows));
    assert.equal((await stat(native)).mtimeMs, before.mtimeMs);

    // A metadata edit changes projected contents without editing native bytes.
    metadata.deliveries[0]!.presentation.displayText = 'updated fixture dispatch';
    writeClaudeMetadata(options.metadataDirectory, sessionId, metadata);
    await assert.rejects(source.page({limit: 2}), StaleHistorySnapshotError);
    await assert.rejects(openClaudeSavedHistory({...options, snapshot: source.snapshot}), StaleHistorySnapshotError);
    const nextIndex = await openClaudeSavedHistory(options);
    assert.ok(nextIndex);
    const changedDelivery = await nextIndex.byId(delivery) as {presentation: {displayText: string}};
    assert.equal(changedDelivery.presentation.displayText, 'updated fixture dispatch');

    const newHead = add('user', 'new native branch head');
    await appendFile(native, encode(rows.slice(-1)));
    await assert.rejects(nextIndex.validate(), StaleHistorySnapshotError);
    const appendedIndex = await openClaudeSavedHistory(options);
    assert.ok(appendedIndex);
    assert.deepEqual((await appendedIndex.page({limit: 1})).historyMessageIds, [newHead]);
    const unchangedLength = await stat(native);
    await writeFile(native, encode(rows).replace('new native branch head', 'new native branch edit'));
    await utimes(native, unchangedLength.atime, unchangedLength.mtime);
    await assert.rejects(appendedIndex.validate(), StaleHistorySnapshotError);

    assert.equal(await openClaudeSavedHistory({...options, nativePath: join(projectDir, 'another.jsonl')}), undefined);
    const alternate = join(config, 'projects', 'override-project');
    await mkdir(alternate);
    await writeFile(join(alternate, `${sessionId}.jsonl`), encode(rows));
    const override = {...env, CLAUDE_CODE_PROJECT_DIR_NAME: 'override-project'};
    assert.ok((await readSessionMessages(sessionId, cwd, override)).length);
    assert.equal(await openClaudeSavedHistory({...options, env: override}), undefined);
    if (process.platform !== 'win32') {
      const linkedId = randomUUID();
      await symlink(native, join(projectDir, `${linkedId}.jsonl`));
      assert.equal(await resolveClaudeTranscript(linkedId, cwd, env), undefined);
    }
    const otherSession = randomUUID();
    await writeFile(join(projectDir, `${otherSession}.jsonl`), encode(rows));
    const {nativePath: _expect, ...legacyOptions} = options;
    assert.equal(await openClaudeSavedHistory({...legacyOptions, sessionId: otherSession}), undefined, 'native UUID mismatch stays on the legacy reader');
    await assert.rejects(readSessionMessages(sessionId, cwd, override, undefined, native), /original transcript is unavailable/);

    // Keep the UUID and path while replacing the file as the SDK subprocess
    // starts. A preflight-only guard would expose the replacement's messages.
    const swap = join(root, 'swap-native-source.cjs');
    const replacement = encode([{type: 'user', uuid: randomUUID(), parentUuid: null, sessionId, cwd, isSidechain: false,
      timestamp: new Date(timestamp++).toISOString(), message: {role: 'user', content: 'replacement source'}}]);
    await writeFile(swap, `const fs = require('node:fs');
      fs.renameSync(${JSON.stringify(native)}, ${JSON.stringify(join(root, 'original-source.jsonl'))});
      fs.writeFileSync(${JSON.stringify(native)}, ${JSON.stringify(replacement)});`);
    await assert.rejects(readSessionMessages(sessionId, cwd, {...env, NODE_OPTIONS: `--require "${swap}"`}, undefined, native),
      /original transcript changed during history read/);
    assert.match(JSON.stringify(await readSessionMessages(sessionId, cwd, env)), /replacement source/);
    const otherCache = claudeHistoryCacheDirectory(root, 'another-workspace');
    await mkdir(otherCache, {recursive: true});
    await writeFile(join(otherCache, 'another.index'), 'unrelated');
    await removeClaudeHistoryCache(root, workspaceId);
    await assert.rejects(stat(options.cacheDirectory), {code: 'ENOENT'});
    assert.equal(await readFile(join(otherCache, 'another.index'), 'utf8'), 'unrelated');
    assert.equal(await readFile(native, 'utf8'), replacement);
    console.log(JSON.stringify({pass: true, sdkRows: official.length, displayRows: cold.total, nativeBytes: before.size,
      latestResponseBytes: cold.bytes, exactToolBytes: cold.toolBytes, rowQueryPlan: plan,
      coldPageWithoutSdk: true, fallbackBoundaries: true, replacementSourceRejected: true, workspaceCacheRemoved: true, noInference: true}));
  } finally {
    await rm(root, {recursive: true, force: true});
  }
}

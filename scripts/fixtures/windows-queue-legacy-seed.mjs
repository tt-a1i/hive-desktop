// Real Hive/SQLite and the normal Codex adapter, with a local JSONL peer.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { fileURLToPath, pathToFileURL } from 'node:url';

const [root, resources, mode] = process.argv.slice(2);
const data = join(root, 'state'), project = join(root, 'project');
const peer = fileURLToPath(new URL('../../packages/runtime/checks/fixtures/codex-queue-peer.cjs', import.meta.url));
const command = join(root, process.platform === 'win32' ? 'peer.cmd' : 'peer');
const originalText = 'Original legacy Codex message 40LEGACY';
const draft = 'Independent composer draft 40DRAFT';
const nativePath = join(project, 'queue-peer-thread.json'), queriesPath = join(project, 'queue-peer-queries.jsonl');
const seedPath = join(root, 'queue-seed.json');
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const json = async path => JSON.parse(await readFile(path, 'utf8'));
const jsonOrEmpty = path => json(path).catch(error => { if (error.code === 'ENOENT') return []; throw error; });
const save = (path, value) => writeFile(path, JSON.stringify(value));
const queries = async () => (await readFile(queriesPath, 'utf8')).trim().split('\n').map(JSON.parse);
const environment = { HIVE_CODEX_COMMAND: command, CODEX_HOME: join(root, 'codex-home') };
const { createDesktopRuntime } = await import(pathToFileURL(join(resources, 'runtime/dist/desktop-runtime.js')).href);
Object.assign(process.env, environment, { PI_OFFLINE: '1', PI_CODING_AGENT_DIR: join(root, 'offline-pi') });

const wait = async predicate => {
  const end = Date.now() + 15000;
  while (!await predicate()) {
    assert(Date.now() < end, 'The controlled prior turn did not complete');
    await delay(50);
  }
};

if (mode === 'seed') {
  await Promise.all([project, data, process.env.PI_CODING_AGENT_DIR, environment.CODEX_HOME]
    .map(path => mkdir(path, { recursive: true })));
  const quote = value => `'${value.replaceAll("'", "'\\''")}'`;
  await writeFile(command, process.platform === 'win32'
    ? `@"${process.execPath}" "${peer}" %*\r\n`
    : `#!/bin/sh\nexec ${quote(process.execPath)} ${quote(peer)} "$@"\n`, { mode: 0o700 });
  await save(join(project, 'queue-peer-control.json'), { complete: true });
  const attachmentSource = join(project, 'legacy-original-attachment.txt');
  await writeFile(attachmentSource, 'Original legacy attachment bytes 40LEGACY\n');
  const runtime = await createDesktopRuntime(data, () => {});
  let fixture;
  try {
    const workspace = await runtime.hive.request('POST', '/api/workspaces', {
      path: project, name: 'Legacy Codex recovery fixture', autostart_orchestrator: false,
    });
    const { id } = await runtime.chats.create(workspace.id, 'codex');
    await runtime.chats.prompt(id, 'Prior native thread anchor 40ANCHOR');
    await wait(async () => {
      const chat = await runtime.chats.open(id);
      return !chat.pending && chat.completedDeliveryIds.length === 1;
    });
    const { queue } = runtime.chats.live.get(id);
    await queue.pause();
    // Seeding only. Hold the original Hive input until the legacy spool is installed.
    queue.resume = () => {};
    const { attachment } = await runtime.chats.attachments.add(workspace.id, attachmentSource);
    await runtime.chats.prompt(id, originalText, [attachment.id]);
    const chat = await runtime.chats.open(id);
    const [original] = queue.pending;
    assert.equal(queue.pending.length, 1);
    assert.equal(original.id, chat.pendingUserInputs[0].deliveryId);
    assert.equal(original.presentation.displayText, originalText);
    assert.deepEqual(original.presentation.attachments.map(file => file.id), [attachment.id]);
    const { messageSequence } = original.presentation;
    const source = runtime.hive.getUserInput(workspace.id, chat.agentId, messageSequence);
    assert.equal(source?.type, 'user_input');
    const [copy] = await runtime.chats.attachments.get(workspace.id, [attachment.id]);
    const name = hash(id) + '.json';
    fixture = {
      sessionId: id, workspaceId: workspace.id, agentId: chat.agentId,
      originalSpoolFormat: 'pending/steerUser:true', deliveryId: original.id, originalText, draft, project, environment,
      attachmentId: attachment.id, attachmentName: attachment.name, attachmentPath: copy.filePath,
      attachmentSha256: hash(await readFile(copy.filePath)), source, presentation: original.presentation,
      spoolPath: join(runtime.chats.queueDir, name), decisionPath: join(data, 'chats', 'steer-dispositions', name),
      draftPath: join(data, 'desktop-drafts', `session-${id}.json`),
      nativePath, nativeBefore: await json(nativePath), nativeSha256: hash(await readFile(nativePath)),
      queriesPath, queriesBefore: await queries(),
    };
    assert.equal(fixture.queriesBefore.length, 1);
    runtime.hive.saveNavigation(JSON.stringify({ workspace: workspace.id, sessions: { [workspace.id]: id } }));
    await save(join(data, 'desktop-language.json'), 'en');
    await mkdir(join(data, 'desktop-drafts'), { recursive: true });
    await save(fixture.draftPath, { key: `session:${id}`, input: draft, failed: [] });
  } finally {
    await runtime.close();
  }
  const [item] = await json(fixture.spoolPath);
  const { steerQueueVersion, steerAttemptVersion, retryReviewVersion, ...original } = item;
  assert.equal(steerQueueVersion, undefined);
  assert.equal(steerAttemptVersion, undefined);
  assert.equal(retryReviewVersion, undefined);
  assert.equal(original.queueState, 'pending');
  await save(fixture.spoolPath, [{ ...original, steerUser: true }]);
  await save(seedPath, fixture);
} else {
  const fixture = await json(seedPath);
  const native = await json(nativePath), nativeQueries = await queries();
  const spool = await jsonOrEmpty(fixture.spoolPath), decisions = await jsonOrEmpty(fixture.decisionPath);
  const currentDraft = (await json(fixture.draftPath)).input;
  const attachmentSha256 = hash(await readFile(fixture.attachmentPath));
  const nativeSha256 = hash(await readFile(nativePath));
  const result = { mode, threadId: native.id, deliveryId: fixture.deliveryId, attachmentId: fixture.attachmentId,
    attachmentSha256, nativeSha256, nativeTurns: nativeQueries.length };
  if (mode === 'inspect') {
    await save(join(root, 'queue-inspect.json'), {
      ...result, spool, decisions, nativeQueries, draftPreserved: currentDraft === draft,
    });
  } else {
    assert.equal(currentDraft, draft);
    assert.equal(attachmentSha256, fixture.attachmentSha256);
    assert.equal(native.id, fixture.nativeBefore.id);
    assert.deepEqual(native.turns.slice(0, 1), fixture.nativeBefore.turns);
    assert.deepEqual(nativeQueries.slice(0, 1), fixture.queriesBefore);
    assert.equal((await readFile(join(project, 'queue-peer-steers.jsonl'), 'utf8').catch(error => {
      if (error.code === 'ENOENT') return '';
      throw error;
    })), '');
    if (mode === 'before' || mode === 'resolved') {
      assert.equal(nativeSha256, fixture.nativeSha256);
      assert.equal(nativeQueries.length, 1);
      assert.equal(spool.length, 1);
      const [item] = spool;
      assert.equal(item.id, fixture.deliveryId);
      assert.deepEqual(item.presentation, fixture.presentation);
      assert.equal(item.steerUser, true);
      assert.equal(item.steerQueueVersion, 1);
      if (mode === 'before') {
        assert.deepEqual(decisions, []);
        assert.equal(item.queueState, 'started');
        assert.equal(item.steerAttemptVersion, 1);
      } else {
        assert.equal(decisions.length, 1);
        const [decision] = decisions;
        assert.equal(decision.deliveryId, fixture.deliveryId);
        assert.equal(decision.disposition, 'retry-approved');
        assert.equal(decision.nativeOutcome, 'unknown');
        assert.deepEqual(decision.presentation, fixture.presentation);
        assert.equal(item.queueState, 'pending');
        assert.equal(item.steerAttemptVersion, undefined);
        assert.equal(item.retryReviewVersion, decision.queueVersion);
      }
    } else {
      assert.equal(mode, 'after');
      assert.equal(nativeQueries.length, 2);
      assert.equal(native.turns.length, 2);
      assert.equal(native.turns[1].status, 'completed');
      assert.equal(native.turns[1].id, nativeQueries[1].turnId);
      const wireText = nativeQueries[1].input.map(part => part.text ?? '').join('\n');
      assert(wireText.includes(originalText));
      assert(wireText.includes(JSON.stringify(fixture.attachmentPath).slice(1, -1)));
      assert(!wireText.includes(draft));
      assert.deepEqual(spool, []);
      const runtime = await createDesktopRuntime(data, () => {});
      try {
        const chat = await runtime.chats.open(fixture.sessionId);
        assert.equal(chat.pending, 0);
        assert.deepEqual(chat.completedDeliveryIds.filter(id => id === fixture.deliveryId), [fixture.deliveryId]);
        const history = chat.history.filter(row => row.presentation?.deliveryId === fixture.deliveryId);
        assert.equal(history.length, 1);
        assert.equal(history[0].presentation.displayText, originalText);
        assert.deepEqual(history[0].presentation.attachments.map(file => file.id), [fixture.attachmentId]);
        assert.deepEqual({ ...runtime.hive.getUserInput(fixture.workspaceId, fixture.agentId, fixture.presentation.messageSequence) },
          fixture.source);
        assert.deepEqual(await queries(), nativeQueries);
      } finally {
        await runtime.close();
      }
      result.originalSentExactlyOnce = true;
      result.originalHiveInputPreserved = true;
    }
    await save(join(root, `queue-${mode}.json`), { ...result, draftPreserved: true, originalNativeTurnsPreserved: true });
  }
}

// Real bundled Hive/SQLite/Codex adapter; existing controlled peer, no account or model.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { setTimeout as delay } from 'node:timers/promises';
const [root, resources, mode = 'seed'] = process.argv.slice(2);
const data = join(root, 'state'), project = join(root, 'project'), pi = join(root, 'offline-pi');
const originalText = 'Queue attachment original 40A7', editedText = 'Queue attachment edited 40A7';
const canceledText = 'Queue withdraw without delivery 40B8', draft = 'Independent composer draft 40C9';
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const peer = fileURLToPath(new URL('../../packages/runtime/checks/fixtures/codex-queue-peer.cjs', import.meta.url));
const command = join(root, process.platform === 'win32' ? 'peer.cmd' : 'peer');
Object.assign(process.env, {HIVE_DESKTOP_DATA_DIR: data, PI_CODING_AGENT_DIR: pi,
  PI_OFFLINE: '1', HIVE_CODEX_COMMAND: command, CODEX_HOME:join(root, 'codex-home')});
const { createDesktopRuntime } = await import(pathToFileURL(join(resources, 'runtime/dist/desktop-runtime.js')).href);
const queries = async () => (await readFile(join(project, 'queue-peer-queries.jsonl'), 'utf8').catch(error => {if(error.code==='ENOENT')return '';throw error;}))
  .trim().split('\n').filter(Boolean).map(line => JSON.parse(line));
let runtime;
try {
  if (mode === 'seed' || mode === 'stop-seed' || mode === 'declined-seed') {
    const stopRejection = mode !== 'seed';
    const rejectionCode = mode === 'declined-seed' ? -32600 : -32603;
    await Promise.all([project, data, pi].map(path => mkdir(path, {recursive: true})));
    const quote = value => `'${value.replaceAll("'", "'\\''")}'`;
    await writeFile(command, process.platform === 'win32'
      ? `@"${process.execPath}" "${peer}" %*\r\n`
      : `#!/bin/sh\nexec ${quote(process.execPath)} ${quote(peer)} "$@"\n`, {mode: 0o700});
    runtime = await createDesktopRuntime(data, () => {});
    const workspace = await runtime.hive.request('POST', '/api/workspaces', {
      path: project, name: 'Queued input fixture', autostart_orchestrator: false});
    const chat = await runtime.chats.create(workspace.id, 'codex');
    if (stopRejection) {
      await runtime.chats.resume(chat.id);
      await runtime.chats.live.get(chat.id).queue.pause();
      // Seeding only: hold the real durable queue until this runtime closes.
      // The separately launched desktop owns an unmodified queue instance.
      runtime.chats.live.get(chat.id).queue.resume=()=>{};
    } else {
    await runtime.chats.prompt(chat.id, 'Controlled held turn 40D0');
    const end = Date.now() + 8000;
    while (!(await queries().catch(error => {if (error.code === 'ENOENT') return []; throw error;})).length) {
      assert(Date.now() < end, 'Controlled first turn did not start'); await delay(20);
    }
    }
    const source = join(project, 'queue-proof.txt');
    await writeFile(source, 'Attachment bytes retained through native queue edit 40E1\n');
    const {attachment} = await runtime.chats.attachments.add(workspace.id, source);
    let primary;
    if (stopRejection) {
      const primaryPath=join(project,'primary-proof.txt');
      await writeFile(primaryPath,'Primary message attachment preserved after Stop 40F2\n');
      const primaryAttachment=(await runtime.chats.attachments.add(workspace.id,primaryPath)).attachment;
      await runtime.chats.prompt(chat.id,'Controlled primary before rejected steer 40F2',[primaryAttachment.id]);
      const [primaryCopy]=await runtime.chats.attachments.get(workspace.id,[primaryAttachment.id]);
      primary={attachment:primaryAttachment, sha256:digest(await readFile(primaryCopy.filePath))};
      await writeFile(join(project,'queue-peer-control.json'),JSON.stringify({rejectSteerWhileActive:true,rejectSteerCode:rejectionCode}));
      await writeFile(join(data,'desktop-general.json'),JSON.stringify({busy_send_mode:'steer_when_supported'}));
    } else {
      await runtime.chats.prompt(chat.id, originalText, [attachment.id]);
      await runtime.chats.prompt(chat.id, canceledText);
      await runtime.chats.abort(chat.id);
    }
    const opened = await runtime.chats.open(chat.id);
    const first = stopRejection ? opened.pendingUserInputs[0] : opened.pendingUserInputs.find(item => item.text === originalText);
    const second = opened.pendingUserInputs.find(item => item.text === canceledText);
    assert(first?.editable && (stopRejection || second?.editable));
    assert.equal(opened.pendingUserInputs.length, stopRejection ? 1 : 2);
    const [copy] = await runtime.chats.attachments.get(workspace.id, [attachment.id]);
    const fixture = {workspaceId: workspace.id, sessionId: chat.id, project, originalText, editedText,
      canceledText, draft, deliveryId: first.deliveryId, canceledId: second?.deliveryId,
      attachmentId: attachment.id, attachmentSha256: digest(await readFile(copy.filePath)),
      queriesBefore: await queries(), environment: {HIVE_CODEX_COMMAND: command, CODEX_HOME:join(root, 'codex-home')}};
    if(stopRejection)Object.assign(fixture,{stopRejection:true,rejectionCode,primaryId:first.deliveryId,primaryText:first.text,
      primaryAttachmentId:primary.attachment.id,primaryAttachmentSha256:primary.sha256,
      spoolPath:join(runtime.chats.queueDir,digest(chat.id)+'.json'),
      rejectionPath:join(project,'queue-peer-rejected-steer.json'),snapshotPath:join(root,'queue-before-stop.json')});
    assert.equal(fixture.queriesBefore.length, stopRejection ? 0 : 1);
    runtime.hive.saveNavigation(JSON.stringify({workspace:workspace.id, sessions:{[workspace.id]:chat.id}}));
    await writeFile(join(data, 'desktop-language.json'), JSON.stringify('en'));
    await mkdir(join(data, 'desktop-drafts'), {recursive: true});
    await writeFile(join(data, 'desktop-drafts', `session-${chat.id}.json`), JSON.stringify({
      key:`session:${chat.id}`, input:draft, failed:[],...(stopRejection&&{attachments:[attachment]})}));
    await writeFile(join(root, 'queue-seed.json'), JSON.stringify(fixture));
  } else {
    const fixture = JSON.parse(await readFile(join(root, 'queue-seed.json'), 'utf8'));
    runtime = await createDesktopRuntime(data, () => {});
    const opened = await runtime.chats.open(fixture.sessionId);
    const [copy] = await runtime.chats.attachments.get(fixture.workspaceId, [fixture.attachmentId]);
    assert.equal(digest(await readFile(copy.filePath)), fixture.attachmentSha256);
    const savedDraft = JSON.parse(await readFile(join(data, 'desktop-drafts', `session-${fixture.sessionId}.json`), 'utf8'));
    const nativeQueries = await queries();
    let stopEvidence;
    if(fixture.stopRejection && mode!=='inspect') {
      const snapshot=JSON.parse(await readFile(fixture.snapshotPath,'utf8'));
      assert.equal(snapshot.length,2);
      const original=snapshot.find(item=>item.presentation.displayText===fixture.originalText);
      assert(original && original.presentation.attachments[0].id===fixture.attachmentId);
      assert(snapshot.some(item=>item.id===fixture.primaryId && item.presentation.attachments[0].id===fixture.primaryAttachmentId));
      const rejection=JSON.parse(await readFile(fixture.rejectionPath,'utf8'));
      assert.equal(rejection.errorCode,fixture.rejectionCode??-32603);
      assert(JSON.stringify(rejection.input).includes(fixture.originalText));
      const interrupts=(await readFile(join(project,'queue-peer-interrupts.jsonl'),'utf8')).trim().split('\n').map(JSON.parse);
      assert.equal(interrupts.length,1);assert.equal(interrupts[0].turnId,rejection.turnId);assert(interrupts[0].rejectionAlreadySent);
      const [primaryCopy]=await runtime.chats.attachments.get(fixture.workspaceId,[fixture.primaryAttachmentId]);
      assert.equal(digest(await readFile(primaryCopy.filePath)),fixture.primaryAttachmentSha256);
      const primaryHistory=opened.history.filter(item=>item.presentation?.deliveryId===fixture.primaryId);
      assert.equal(primaryHistory.length,1);assert.equal(primaryHistory[0].presentation.attachments[0].id,fixture.primaryAttachmentId);
      assert(JSON.stringify(nativeQueries[0].input).includes(fixture.primaryText));
      stopEvidence={originalDeliveryId:original.id,primaryId:fixture.primaryId,primaryAttachmentId:fixture.primaryAttachmentId,
        primaryAttachmentSha256:fixture.primaryAttachmentSha256,rejectionCode:rejection.errorCode,rejectionBeforeStop:true,primarySentOnce:true};
      if(mode==='before') {
        fixture.deliveryId=original.id;fixture.queriesBefore=nativeQueries;
        assert.equal(nativeQueries.length,1);
        await writeFile(join(root,'queue-seed.json'),JSON.stringify(fixture));
      } else assert.equal(fixture.deliveryId,original.id);
    }
    if (mode === 'inspect') {
      const diagnostic = async path => readFile(path,'utf8').catch(error => ({readError:String(error)}));
      await writeFile(join(root, 'queue-inspect.json'), JSON.stringify({
        pending:opened.pendingUserInputs, queries:nativeQueries, draft:savedDraft.input,
        ...(fixture.stopRejection&&{beforeStopSnapshot:await diagnostic(fixture.snapshotPath),
          rejection:await diagnostic(fixture.rejectionPath),interrupts:await diagnostic(join(project,'queue-peer-interrupts.jsonl'))}),
        attachmentSha256:digest(await readFile(copy.filePath))}));
      // This diagnostic records the actual state and does not claim acceptance.
      process.exitCode = 0;
    } else {
    assert.equal(savedDraft.input, fixture.draft);
    if (mode === 'before') {
      assert.equal(opened.pendingUserInputs.length, 1);
      const first = opened.pendingUserInputs[0];
      assert.equal(first.deliveryId, fixture.deliveryId);
      assert.equal(first.text, fixture.editedText);
      assert.equal(first.attachments[0].id, fixture.attachmentId);
      assert.equal(first.editable, true);
      assert.deepEqual(nativeQueries, fixture.queriesBefore, 'Editing/canceling/reopen must not send a native turn');
      // The existing peer completes the next turn, only after explicit Resume.
      await writeFile(join(project, 'queue-peer-control.json'), JSON.stringify({complete:true}));
    } else {
      assert.equal(mode, 'after');
      assert.deepEqual(opened.pendingUserInputs, []);
      assert.equal(nativeQueries.length, fixture.queriesBefore.length + 1);
      assert.deepEqual(nativeQueries.slice(0, -1), fixture.queriesBefore);
      const input = JSON.stringify(nativeQueries.at(-1).input);
      assert(input.includes(fixture.editedText));
      assert(!input.includes(fixture.originalText));
      assert(!JSON.stringify(nativeQueries).includes(fixture.canceledText));
      const history = opened.history.filter(item => item.customType === 'hive-delivery');
      const delivered = history.filter(item => item.presentation?.deliveryId === fixture.deliveryId);
      assert.equal(delivered.length, 1);
      assert.equal(delivered[0].presentation.displayText, fixture.editedText);
      assert.equal(delivered[0].presentation.attachments[0].id, fixture.attachmentId);
      assert.equal(history.filter(item => item.presentation?.deliveryId === fixture.canceledId).length, 0);
    }
    await writeFile(join(root, `queue-${mode}.json`), JSON.stringify({
      mode, originalDeliveryId:fixture.deliveryId, attachmentId:fixture.attachmentId,
      attachmentSha256:fixture.attachmentSha256, draftPreserved:true,
      ...(stopEvidence&&{stopEvidence}),
      nativeTurns:nativeQueries.length, pending:opened.pendingUserInputs,
      ...(!fixture.stopRejection&&{canceledInputNeverSent:true}), editedInputSentExactlyOnce:mode === 'after'}));
    }
  }
} finally { await runtime?.close(); }

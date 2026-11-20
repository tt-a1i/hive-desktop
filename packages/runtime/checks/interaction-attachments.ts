/** Controlled Codex JSONL peer and real Hive uploads/privateFS; no model calls. */
import assert from 'node:assert/strict';
import {mkdtemp, mkdir, writeFile, readFile, readdir, rename, unlink} from 'node:fs/promises';
import {join, resolve} from 'node:path';
import {createDesktopRuntime} from '../src/desktop-runtime.js';
import {CodexTransport} from '../src/agents/codex-transport.js';
import {createHash, randomUUID} from 'node:crypto';
import {DatabaseSync} from 'node:sqlite';
import fs from 'node:fs';
import {syncBuiltinESMExports} from 'node:module';
import {QuestionAttachmentCopyError} from '../src/image-attachments.js';
import type {PiSessionHandle} from '../src/agents/types.js';

await mkdir(resolve('.artifacts'), {recursive:true});
const dir = await mkdtemp(resolve('.artifacts/question-attachments-'));
const peer = await readFile(new URL('./fixtures/codex-question-attachment-peer.cjs', import.meta.url), 'utf8');
let runtime = await createDesktopRuntime(join(dir,'state'),()=>{}, {executableEnvironment:{}});
const live = () => (runtime.chats as unknown as {live:Map<string,{handle:PiSessionHandle}>}).live;
async function wait<T>(f:()=>T|Promise<T>, p:(v:T)=>boolean):Promise<T>{const deadline=Date.now()+8000;for(;;){const v=await f();if(p(v))return v;if(Date.now()>deadline)throw new Error('fixture timed out');await new Promise(r=>setTimeout(r,10));}}
async function rows(cwd:string){try{return (await readFile(join(cwd,'responses.jsonl'),'utf8')).trim().split('\n').map(x=>JSON.parse(x));}catch(e){if((e as NodeJS.ErrnoException).code==='ENOENT')return [];throw e;}}
let count=0;
async function start(mode = 'question'){
 const cwd = join(dir, `project-${++count}`);
 await mkdir(cwd);
 await writeFile(join(cwd,'package.json'),'{"type":"commonjs"}');
 await writeFile(join(cwd,'app-server'),peer);
 const ws = await runtime.hive.request('POST','/api/workspaces',{path:cwd,name:'fixture',autostart_orchestrator:false}) as {id:string};
 const record = await runtime.chats.create(ws.id,'codex');
 const handle = live().get(record.id)!.handle;
 const done = handle.prompt(mode);void done.catch(()=>{});
 const snapshot = await wait(()=>runtime.chats.open(record.id),v=>v.interactions.length===1);
 return {cwd,ws,record,handle,done,card:snapshot.interactions[0]!};
}
const answer={answers:{public:{answers:['ANSWER-SENTINEL']},secret:{answers:['SECRET-SENTINEL']}}};
async function checkCopyLossBetweenQuestions() {
  const {ws, record, card, cwd, done} = await start();
  const firstPath = join(dir, 'delayed-first.txt');
  const secondPath = join(dir, 'delayed-second.txt');
  await writeFile(firstPath, 'first copied content');
  await writeFile(secondPath, 'second copied content');

  const uploads = runtime.chats.attachments;
  const first = await uploads.add(ws.id, firstPath);
  const second = await uploads.add(ws.id, secondPath);
  const mapping = {public: [first.attachment.id], extra: [second.attachment.id]};
  const reply = {answers: {public: {answers: ['first answer']}, extra: {answers: ['second answer']}}};
  const {receipt: draft} = await runtime.chats.saveInteractionAttachments(record.id, card.id, mapping);
  const firstCopy = draft.attachments.find(item => item.questionId === 'public')!.filePath;
  const secondCopy = draft.attachments.find(item => item.questionId === 'extra')!.filePath;
  const submit = () => runtime.chats.respondInteraction(record.id, card.id, reply, mapping);

  const resolveReferences = uploads.questionReferences.bind(uploads);
  let markSecondReached!: () => void;
  let resumeResolution!: () => void;
  const secondReached = new Promise<void>(resolve => { markSecondReached = resolve; });
  const paused = new Promise<void>(resolve => { resumeResolution = resolve; });
  uploads.questionReferences = async (workspace, ids) => {
    if (Array.isArray(ids) && ids.includes(second.attachment.id)) {
      markSecondReached();
      await paused;
    }
    return resolveReferences(workspace, ids);
  };
  try {
    const submission = submit();
    // Propagate early errors instead of waiting indefinitely for the second question.
    await Promise.race([secondReached, submission.then(() => assert.fail('Submitted before resolving both questions'))]);
    await rename(firstCopy, firstCopy + '.held');
    resumeResolution();
    await assert.rejects(submission, error => error instanceof QuestionAttachmentCopyError && error.attachmentId === first.attachment.id);
    assert.deepEqual(await rows(cwd), []);
    assert.deepEqual(runtime.chats.interactionAttachmentStatus(record.id, card.id), {safeToRetry: true});
    assert.deepEqual(runtime.chats.listInteractionAttachments(record.id).receipts, [JSON.parse(JSON.stringify(draft))]);
    await rename(firstCopy + '.held', firstCopy);
  } finally {
    resumeResolution();
    uploads.questionReferences = resolveReferences;
  }

  assert.equal((await submit()).receipt.status, 'callback-handed');
  const wire = await wait(() => rows(cwd), value => value.length === 1);
  const answers = wire[0].result.answers;
  // The native answer contains JSON-encoded paths, so backslashes in Windows
  // paths are escaped. Parse the records and compare their original values.
  const references = (text: string) => text.split('\n').filter(line => line.startsWith('{')).map(line => JSON.parse(line));
  const windowsReference = {name: 'sample.txt', mimeType: 'application/octet-stream', path: 'C:\\fixture\\nested\\sample.txt'};
  assert.deepEqual(references(`Answer\n${JSON.stringify(windowsReference)}`), [windowsReference]);
  assert.deepEqual(references(answers.public.answers[0]), [
    {name: first.attachment.name, mimeType: first.attachment.mimeType, path: firstCopy},
  ]);
  assert.deepEqual(references(answers.extra.answers[0]), [
    {name: second.attachment.name, mimeType: second.attachment.mimeType, path: secondCopy},
  ]);
  await assert.rejects(submit());
  await writeFile(join(cwd, 'finish'), '');
  await done;
  assert.equal((await rows(cwd)).length, 1);
}

async function checkCopyLossDuringProjectVerification() {
  const {ws, record, card, cwd, done} = await start();
  const plainSource = join(cwd, 'plain.txt');
  await writeFile(plainSource, 'ordinary');
  await writeFile(join(cwd, 'snapshot.txt'), 'snapshot');
  const ordinary = await runtime.chats.attachments.add(ws.id, plainSource);
  const project = await runtime.chats.projectFilesAttach(ws.id, 'snapshot.txt');
  const mapping = {public: [ordinary.attachment.id, project.attachment.id]};
  const draft = (await runtime.chats.saveInteractionAttachments(record.id, card.id, mapping)).receipt;
  const originalStream = fs.createReadStream;
  let duringProjectHash: (() => void) | undefined;
  fs.createReadStream = ((...args: Parameters<typeof originalStream>) => {
    const stream = originalStream(...args);
    if (args[0] === project.filePath && duringProjectHash) {
      stream.once('data', duringProjectHash);
      duringProjectHash = undefined;
    }
    return stream;
  }) as typeof fs.createReadStream;
  syncBuiltinESMExports();
  try {
    const submit = () => runtime.chats.respondInteraction(record.id, card.id, answer, mapping);
    const checkRejected = async (mutate: () => void) => {
      duringProjectHash = mutate;
      await assert.rejects(submit, error => error instanceof QuestionAttachmentCopyError && error.attachmentId === ordinary.attachment.id);
      assert.equal(duringProjectHash, undefined, 'The failure must occur after the first attachment was hashed');
      assert.deepEqual(await rows(cwd), []);
      assert.deepEqual(runtime.chats.interactionAttachmentStatus(record.id, card.id), {safeToRetry: true});
      assert.deepEqual(runtime.chats.listInteractionAttachments(record.id).receipts, [JSON.parse(JSON.stringify(draft))]);
    };
    await checkRejected(() => fs.writeFileSync(ordinary.filePath!, 'replaced'));
    await writeFile(ordinary.filePath!, 'ordinary');
    await checkRejected(() => fs.unlinkSync(ordinary.filePath!));
  } finally {
    fs.createReadStream = originalStream;
    syncBuiltinESMExports();
  }
  await writeFile(ordinary.filePath!, 'ordinary');
  await runtime.chats.respondInteraction(record.id, card.id, answer, mapping);
  assert.equal((await wait(() => rows(cwd), value => value.length === 1)).length, 1);
  await assert.rejects(runtime.chats.respondInteraction(record.id, card.id, answer, mapping));
  await writeFile(join(cwd, 'finish'), '');
  await done;
  assert.equal((await rows(cwd)).length, 1);
}

async function checkProjectSnapshotReplacement() {
  const {ws, record, card, cwd, done} = await start();
  const source = join(cwd, 'snapshot.txt');
  await writeFile(source, 'original');
  const project = await runtime.chats.projectFilesAttach(ws.id, 'snapshot.txt');
  const mapping = {public: [project.attachment.id]};
  const draft = (await runtime.chats.saveInteractionAttachments(record.id, card.id, mapping)).receipt;
  await writeFile(project.filePath!, 'replaced');
  await assert.rejects(runtime.chats.prompt(record.id, 'send corrupted snapshot', [project.attachment.id]), /项目文件附件 .* 的副本不可用或已更改/);
  await assert.rejects(runtime.chats.respondInteraction(record.id, card.id, answer, mapping),
    error => error instanceof QuestionAttachmentCopyError && error.attachmentId === project.attachment.id);
  assert.deepEqual(await rows(cwd), []);
  assert.deepEqual(runtime.chats.interactionAttachmentStatus(record.id, card.id), {safeToRetry: true});
  assert.deepEqual(runtime.chats.listInteractionAttachments(record.id).receipts, [JSON.parse(JSON.stringify(draft))]);
  await writeFile(project.filePath!, 'original');
  const uploads = runtime.chats.attachments;
  const resolve = uploads.questionReferences.bind(uploads);
  uploads.questionReferences = async (...args: Parameters<typeof resolve>) => {
    const refs = await resolve(...args);
    await writeFile(project.filePath!, 'replaced');
    return refs;
  };
  try {
    await assert.rejects(runtime.chats.respondInteraction(record.id, card.id, answer, mapping),
    error => error instanceof QuestionAttachmentCopyError && error.attachmentId === project.attachment.id);
    assert.deepEqual(await rows(cwd), []);
    assert.deepEqual(runtime.chats.listInteractionAttachments(record.id).receipts, [JSON.parse(JSON.stringify(draft))]);
  } finally { uploads.questionReferences = resolve; }
  await writeFile(project.filePath!, 'original');
  await runtime.chats.respondInteraction(record.id, card.id, answer, mapping);
  assert.equal((await wait(() => rows(cwd), value => value.length === 1)).length, 1);
  await assert.rejects(runtime.chats.respondInteraction(record.id, card.id, answer, mapping));
  await writeFile(join(cwd, 'finish'), '');
  await done;
  assert.equal((await rows(cwd)).length, 1);
}

// The migration must recognize uploads whose rows and data predate the desktop
// source checksum, including rows with the older, unmodified UUID format.
async function checkLegacyUploadMigration() {
  const run = await start();
  const endpoint = `/api/workspaces/${run.ws.id}/uploads`;
  const legacy = await runtime.hive.request('POST', endpoint, {
    filename: 'legacy.txt', mime_type: 'text/plain', data: Buffer.from('original').toString('base64'),
    captureDesktopSourceDigest: true, // Never accepted from the public upload API.
  }) as {id: string};
  const before = (await runtime.hive.locateUpload(run.ws.id, legacy.id))!;
  assert.equal(before.sourceDigestVersion, 0);
  assert.equal(before.sourceSha256, null);
  const history = (await runtime.chats.saveInteractionAttachments(run.record.id, run.card.id, {public: [legacy.id]})).receipt;
  await run.handle.abort(); await run.done;
  await runtime.close();
  const database = new DatabaseSync(join(dir, 'state', 'hive', 'runtime.sqlite'));
  try {
    database.exec('ALTER TABLE workspace_uploads DROP COLUMN desktop_source_digest_version; ALTER TABLE workspace_uploads DROP COLUMN desktop_source_sha256');
  } finally { database.close(); }
  await writeFile(before.localPath, 'replaced');
  runtime = await createDesktopRuntime(join(dir, 'state'), () => {}, {executableEnvironment: {}});
  const migrated = (await runtime.hive.locateUpload(run.ws.id, legacy.id))!;
  assert.deepEqual(migrated, before);
  const [reference] = await runtime.chats.attachments.questionReferences(run.ws.id, [legacy.id]);
  assert.equal(reference?.verifiedCopy, undefined);
  assert.deepEqual(runtime.chats.listInteractionAttachments(run.record.id).receipts, [JSON.parse(JSON.stringify(history))]);
}

async function checkUploadSourceIntegrity() {
  const {ws, record, card, cwd, done} = await start();
  const textPath = join(cwd, 'source.txt');
  const imagePath = join(cwd, 'source.png');
  const original = Buffer.from('original');
  const png = Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGP4z8AAAAMBAQDJ/pLvAAAAAElFTkSuQmCC', 'base64');
  await writeFile(textPath, original);
  await writeFile(imagePath, png);
  const ordinary = await runtime.chats.attachments.add(ws.id, textPath);
  const selectedImage = await runtime.chats.attachments.add(ws.id, imagePath);
  const pastedImage = await runtime.chats.attachments.addClipboard(ws.id, selectedImage.image);
  const inputs = [
    {staged: ordinary, bytes: original, replacement: Buffer.from('replaced')},
    {staged: selectedImage, bytes: png, replacement: Buffer.from(png.map((byte, index) => index === png.length - 1 ? byte ^ 1 : byte))},
    {staged: pastedImage, bytes: png, replacement: Buffer.from(png.map((byte, index) => index === png.length - 1 ? byte ^ 1 : byte))},
  ];
  await unlink(textPath); await unlink(imagePath);
  const refs = [];
  const changed = /copy is unavailable or changed/;
  for (const {staged, bytes, replacement} of inputs) {
    const id = staged.attachment.id;
    const upload = (await runtime.hive.locateUpload(ws.id, id))!;
    assert.match(id, /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/);
    assert.deepEqual([upload.sourceDigestVersion, upload.sourceSha256], [1, createHash('sha256').update(bytes).digest('hex')]);
    assert.equal(JSON.stringify(staged).includes(upload.sourceSha256!), false);
    const map = {public: [id]};
    // The first check should use the input bytes even before any receipt exists.
    await writeFile(upload.localPath, replacement);
    await assert.rejects(runtime.chats.saveInteractionAttachments(record.id, card.id, map), changed);
    await assert.rejects(runtime.chats.respondInteraction(record.id, card.id, answer, map), changed);
    assert.deepEqual(runtime.chats.listInteractionAttachments(record.id).receipts, []);
    await writeFile(upload.localPath, bytes);
    refs.push(upload);
  }
  const endpoint = `/api/workspaces/${ws.id}/uploads`;
  const publicUploads = JSON.stringify(await runtime.hive.request('GET', endpoint));
  assert.ok(publicUploads.includes(ordinary.attachment.id));
  assert.equal(publicUploads.includes(refs[0]!.sourceSha256!), false);
  const mapping = {public: inputs.map(({staged}) => staged.attachment.id)};
  const draft = (await runtime.chats.saveInteractionAttachments(record.id, card.id, mapping)).receipt;
  for (const upload of refs) assert.equal(JSON.stringify(draft).includes(upload.sourceSha256!), false);
  const database = new DatabaseSync(join(dir, 'state', 'hive', 'runtime.sqlite'));
  try {
    const replaceDigest = database.prepare('UPDATE workspace_uploads SET desktop_source_sha256 = ? WHERE workspace_id = ? AND id = ?');
    const id = ordinary.attachment.id;
    assert.throws(() => replaceDigest.run(null, ws.id, id), /CHECK constraint/);
    database.exec('PRAGMA ignore_check_constraints = ON');
    for (const missingOrDamaged of [null, 'broken', 'f'.repeat(64)]) {
      replaceDigest.run(missingOrDamaged, ws.id, id);
      await assert.rejects(runtime.chats.saveInteractionAttachments(record.id, card.id, mapping), changed);
      await assert.rejects(runtime.chats.respondInteraction(record.id, card.id, answer, mapping), changed);
      assert.deepEqual(runtime.chats.listInteractionAttachments(record.id).receipts, [JSON.parse(JSON.stringify(draft))]);
    }
    replaceDigest.run(refs[0]!.sourceSha256, ws.id, id);
    database.exec('PRAGMA ignore_check_constraints = OFF');
    // Inject a database failure after Hive writes the file. It must remove the
    // private copy and expose neither a new upload nor incomplete evidence.
    const folder = resolve(refs[0]!.localPath, '..');
    const existing = await readdir(folder);
    database.exec(`CREATE TRIGGER fail_fixture_upload BEFORE INSERT ON workspace_uploads WHEN NEW.original_name = 'failed.txt'
      BEGIN SELECT RAISE(ABORT, 'upload fixture'); END`);
    const failedPath = join(cwd, 'failed.txt');
    await writeFile(failedPath, 'retry this source');
    try { await assert.rejects(runtime.chats.attachments.add(ws.id, failedPath), {status: 500}); }
    finally { database.exec('DROP TRIGGER fail_fixture_upload'); }
    assert.deepEqual((await readdir(folder)).sort(), existing.sort());
    assert.equal(JSON.stringify(await runtime.hive.request('GET', endpoint)).includes('failed.txt'), false);
    const retried = await runtime.chats.attachments.add(ws.id, failedPath);
    assert.deepEqual(await readFile(retried.filePath!), Buffer.from('retry this source'));
  } finally { database.close(); }

  const removed = (await runtime.chats.saveInteractionAttachments(record.id, card.id, {})).receipt;
  assert.deepEqual(removed.attachments, []);
  await writeFile(refs[0]!.localPath, inputs[0]!.replacement);
  await assert.rejects(runtime.chats.saveInteractionAttachments(record.id, card.id, mapping), changed);
  await assert.rejects(runtime.chats.respondInteraction(record.id, card.id, answer, mapping), changed);
  assert.deepEqual(runtime.chats.listInteractionAttachments(record.id).receipts, [JSON.parse(JSON.stringify(removed))]);
  assert.deepEqual(await rows(cwd), []);
  await writeFile(refs[0]!.localPath, original);
  assert.equal((await runtime.chats.respondInteraction(record.id, card.id, answer, mapping)).receipt.status, 'callback-handed');
  await wait(() => rows(cwd), value => value.length === 1);
  await assert.rejects(runtime.chats.respondInteraction(record.id, card.id, answer, mapping));
  await writeFile(join(cwd, 'finish'), ''); await done;
  assert.equal((await rows(cwd)).length, 1);
  return {workspaceId: ws.id, id: ordinary.attachment.id, expectedDigest: refs[0]!.sourceSha256};
}

try {
 await runtime.settings.update({piCommand:null,codexCommand:process.execPath});
 await checkLegacyUploadMigration();
 const checkedUpload = await checkUploadSourceIntegrity();
 await checkCopyLossBetweenQuestions();
 await checkProjectSnapshotReplacement();
 await checkCopyLossDuringProjectVerification();
 const a=await start(), other=await start();
 const file=join(dir,'document.txt'), image=join(dir,'image.png');const bytes=Buffer.from('file copy sentinel');const png=Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGP4z8AAAAMBAQDJ/pLvAAAAAElFTkSuQmCC','base64');await writeFile(file,bytes);await writeFile(image,png);
 const doc=await runtime.chats.attachments.add(a.ws.id,file), img=await runtime.chats.attachments.add(a.ws.id,image), foreign=await runtime.chats.attachments.add(other.ws.id,file);await unlink(file);await unlink(image);
 const mapping={public:[doc.attachment.id,img.attachment.id]};
 const recovered=await runtime.chats.attachments.get(a.ws.id,[img.attachment.id]);assert.deepEqual(recovered[0]!.image,img.image);
 for(const map of [{missing:[doc.attachment.id]},{secret:[doc.attachment.id]},{public:[doc.attachment.id,doc.attachment.id]},{public:[foreign.attachment.id]},{public:['unknown']},{public:[doc.attachment.id,randomUUID()]},{public:'bad'}]) await assert.rejects(runtime.chats.saveInteractionAttachments(a.record.id,a.card.id,map));
 assert.deepEqual(await rows(a.cwd),[]);
 const storage=(runtime.chats as unknown as {interactionAttachments:{write:(...args:any[])=>any}}).interactionAttachments;const storageWrite=storage.write;
 storage.write=()=>{throw new Error('injected receipt write failure');};try {await assert.rejects(runtime.chats.saveInteractionAttachments(a.record.id,a.card.id,mapping));await assert.rejects(runtime.chats.respondInteraction(a.record.id,a.card.id,answer,mapping));} finally {storage.write=storageWrite;}
 assert.deepEqual(await rows(a.cwd),[]);assert.deepEqual(runtime.chats.listInteractionAttachments(a.record.id).receipts,[]);
 const draft=(await runtime.chats.saveInteractionAttachments(a.record.id,a.card.id,mapping)).receipt;assert.equal(draft.status,'draft');assert.equal(draft.attachments.length,2);assert.deepEqual(await readFile(draft.attachments[0]!.filePath),bytes);assert.deepEqual(await readFile(draft.attachments[1]!.filePath),png);assert.equal(JSON.stringify(draft).includes('ANSWER-SENTINEL'),false);
 const original=runtime.chats.attachments.questionReferences.bind(runtime.chats.attachments);let entered!:()=>void,release!:()=>void;const enteredP=new Promise<void>(r=>entered=r),gate=new Promise<void>(r=>release=r);let calls=0;
 runtime.chats.attachments.questionReferences=async (...args:Parameters<typeof original>)=>{calls++;entered();await gate;return original(...args);};
 await assert.rejects(runtime.chats.respondInteraction(a.record.id,a.card.id,undefined,mapping));await assert.rejects(runtime.chats.respondInteraction(a.record.id,a.card.id,{answers:{bad:{answers:['bad']}}},mapping));assert.equal(calls,0,'malformed native response must fail before resolving uploads');
 const pending=runtime.chats.respondInteraction(a.record.id,a.card.id,answer,mapping);await enteredP;assert.deepEqual(runtime.chats.interactionAttachmentStatus(a.record.id,a.card.id),{safeToRetry:false});await assert.rejects(runtime.chats.respondInteraction(a.record.id,a.card.id,answer,mapping));await assert.rejects(runtime.chats.respondInteraction(a.record.id,a.card.id,answer));release();const sent=await pending;runtime.chats.attachments.questionReferences=original;
 assert.equal(sent.receipt.status,'callback-handed');const wire=await wait(()=>rows(a.cwd),v=>v.length===1);assert.equal(wire[0].result.answers.secret.answers[0],'SECRET-SENTINEL');const text=wire[0].result.answers.public.answers[0];assert.ok(text.startsWith('ANSWER-SENTINEL\n'));for(const ref of draft.attachments)assert.ok(text.includes(JSON.stringify({name:ref.attachment.name,mimeType:ref.attachment.mimeType,path:ref.filePath})));assert.equal(JSON.stringify(sent.receipt).includes('SENTINEL'),false);
 await assert.rejects(runtime.chats.respondInteraction(a.record.id,a.card.id,answer));await writeFile(join(a.cwd,'finish'),'');await a.done;
 // Damage an actual receipt belonging to another workspace. Both interactions
 // have the same JSON-RPC request ID as the damaged row.
 const receiptDirectory = join(dir, 'state', 'chats', 'interaction-attachments');
 const unrelatedPath = join(receiptDirectory, `${sent.receipt.id}.json`);
 const originalUnrelated = await readFile(unrelatedPath);
 const secondHistory = await start();
 const secondSource = join(dir, 'second-project-upload.txt');
 await writeFile(secondSource, 'second project');
 const secondUpload = await runtime.chats.attachments.add(secondHistory.ws.id, secondSource);
 const secondMapping = {public:[secondUpload.attachment.id]};
 const {receipt: secondDraft} = await runtime.chats.saveInteractionAttachments(secondHistory.record.id, secondHistory.card.id, secondMapping);
 await writeFile(unrelatedPath, '{');
 try {
   assert.deepEqual(runtime.chats.listInteractionAttachments(secondHistory.record.id), {
     receipts:[JSON.parse(JSON.stringify(secondDraft))], unreadableReceipts:1,
   });
   const secondCopy = secondDraft.attachments[0]!.filePath;
   await unlink(secondCopy);
   await assert.rejects(runtime.chats.respondInteraction(secondHistory.record.id, secondHistory.card.id, answer, secondMapping));
   assert.deepEqual(await rows(secondHistory.cwd), []);
   assert.deepEqual(runtime.chats.interactionAttachmentStatus(secondHistory.record.id, secondHistory.card.id), {safeToRetry:true});
   await writeFile(secondCopy, 'second project');
   const retry = await runtime.chats.respondInteraction(secondHistory.record.id, secondHistory.card.id, answer, secondMapping);
   assert.equal(retry.receipt.status, 'callback-handed');
   await wait(()=>rows(secondHistory.cwd), value=>value.length===1);
   await writeFile(join(secondHistory.cwd,'finish'),'');
   await secondHistory.done;
   assert.throws(()=>runtime.chats.interactionAttachmentStatus(secondHistory.record.id, secondHistory.card.id), /no pending Codex question/);
   const question = await start(), approval = await start('approval');
   assert.equal(question.card.nativeRequestId, sent.receipt.nativeRequestId);
   assert.equal(approval.card.nativeRequestId, sent.receipt.nativeRequestId);
   await runtime.chats.respondInteraction(question.record.id, question.card.id, answer);
   const decline = {decision:'decline'};
   await runtime.chats.respondInteraction(approval.record.id, approval.card.id, decline);
   for (const [run, expected] of [[question, answer], [approval, decline]] as const) {
     const replies = await wait(()=>rows(run.cwd), value=>value.length===1);
     assert.deepEqual(replies[0].result, expected);
     await writeFile(join(run.cwd,'finish'),'');
     await run.done;
     assert.equal((await rows(run.cwd)).length, 1);
   }
 } finally {
   await writeFile(unrelatedPath, originalUnrelated);
 }

 // Unknown state in the current question's own receipt still blocks delivery,
 // including a caller using the older RPC shape to omit attachments.
 const damaged = await start();
 const input = join(dir, 'damaged-source.txt');
 await writeFile(input, 'current question');
 const uploaded = await runtime.chats.attachments.add(damaged.ws.id, input);
 const damagedMapping = {public:[uploaded.attachment.id]};
 const {receipt: damagedDraft} = await runtime.chats.saveInteractionAttachments(damaged.record.id, damaged.card.id, damagedMapping);
 const damagedPath = join(receiptDirectory, `${damagedDraft.id}.json`);
 await writeFile(damagedPath, '{');
 assert.throws(()=>runtime.chats.interactionAttachmentStatus(damaged.record.id, damaged.card.id), SyntaxError);
 await assert.rejects(runtime.chats.respondInteraction(damaged.record.id, damaged.card.id, answer), SyntaxError);
 await assert.rejects(runtime.chats.respondInteraction(damaged.record.id, damaged.card.id, answer, damagedMapping), SyntaxError);
 await writeFile(damagedPath, JSON.stringify({...damagedDraft, attachments: null}));
 assert.deepEqual(runtime.chats.listInteractionAttachments(damaged.record.id), {receipts:[], unreadableReceipts:1});
 assert.throws(()=>runtime.chats.interactionAttachmentStatus(damaged.record.id, damaged.card.id), /Invalid question attachment receipt/);
 await writeFile(damagedPath, JSON.stringify({...damagedDraft, status: 'prepared'}));
 assert.deepEqual(runtime.chats.interactionAttachmentStatus(damaged.record.id, damaged.card.id), {safeToRetry:false});
 await assert.rejects(runtime.chats.respondInteraction(damaged.record.id, damaged.card.id, answer), /may already have been handed/);
 await assert.rejects(runtime.chats.respondInteraction(damaged.record.id, damaged.card.id, answer, damagedMapping), /may already have been handed/);
 assert.deepEqual(await rows(damaged.cwd), []);
 assert.equal((await runtime.chats.open(damaged.record.id)).interactions[0]?.state, 'pending');
 await writeFile(damagedPath, JSON.stringify(damagedDraft));
 await damaged.handle.abort();
 await damaged.done;

 for(const mode of ['native-resolve','turn-end','abort','exit']){console.log('CHECK',mode);const run=await start();const source=join(dir,`source-${mode}`);await writeFile(source,mode);const upload=await runtime.chats.attachments.add(run.ws.id,source);let enter!:()=>void,go!:()=>void;const reached=new Promise<void>(r=>enter=r),blocked=new Promise<void>(r=>go=r);runtime.chats.attachments.questionReferences=async (...args:Parameters<typeof original>)=>{enter();await blocked;return original(...args);};const op=runtime.chats.respondInteraction(run.record.id,run.card.id,answer,{public:[upload.attachment.id]});void op.catch(()=>{});await reached;if(mode==='abort')await run.handle.abort();else await writeFile(join(run.cwd,mode),'');if(mode==='exit')await assert.rejects(run.done);else await run.done;go();await assert.rejects(op);runtime.chats.attachments.questionReferences=original;assert.deepEqual(await rows(run.cwd),[]);assert.deepEqual(runtime.chats.listInteractionAttachments(run.record.id).receipts,[]);}
 const loss=await start();const lossFile=join(dir,'loss-file');await writeFile(lossFile,'loss');const lossUpload=await runtime.chats.attachments.add(loss.ws.id,lossFile);
 await assert.rejects(runtime.chats.respondInteraction(loss.record.id,loss.card.id,{answers:{public:{answers:[]}}},{public:[lossUpload.attachment.id]}));assert.deepEqual(runtime.chats.listInteractionAttachments(loss.record.id).receipts,[]);
 const transport=CodexTransport.prototype as unknown as {write:(message:unknown)=>Promise<void>};const originalWrite=transport.write;let injected=0;
 transport.write=async function(message:unknown){const value=message as {id?:unknown;result?:unknown;method?:unknown};if(value.id===7&&value.result!==undefined&&value.method===undefined){injected++;throw new Error('injected transport write failure');}return originalWrite.call(this,message);};
 try {await runtime.chats.respondInteraction(loss.record.id,loss.card.id,answer,{public:[lossUpload.attachment.id]});await assert.rejects(loss.done);assert.equal(injected,1);assert.deepEqual(await rows(loss.cwd),[]);assert.equal(runtime.chats.listInteractionAttachments(loss.record.id).receipts[0]!.status,'callback-handed');await assert.rejects(runtime.chats.respondInteraction(loss.record.id,loss.card.id,answer));} finally {transport.write=originalWrite;}
 console.log('CHECK cold reopen');
 await other.handle.abort();
 await other.done;
 assert.equal(runtime.chats.listInteractionAttachments(a.record.id).receipts[0]!.status,'callback-handed');
 await writeFile(unrelatedPath, 'null');
 try {
   await runtime.close();
   runtime = await createDesktopRuntime(join(dir,'state'),()=>{}, {executableEnvironment:{}});
   const coldUpload = await runtime.hive.locateUpload(checkedUpload.workspaceId, checkedUpload.id);
   assert.equal(coldUpload?.sourceDigestVersion, 1);
   assert.equal(coldUpload?.sourceSha256, checkedUpload.expectedDigest);
   await runtime.chats.open(secondHistory.record.id);
   const recovered = runtime.chats.listInteractionAttachments(secondHistory.record.id);
   assert.equal(recovered.unreadableReceipts, 1);
   assert.equal(recovered.receipts.length, 1);
   assert.equal(recovered.receipts[0]?.status, 'callback-handed');
   assert.equal((await rows(secondHistory.cwd)).length, 1);
 } finally {
   await writeFile(unrelatedPath, originalUnrelated);
 }
 assert.equal(runtime.chats.listInteractionAttachments(a.record.id).receipts[0]!.status,'callback-handed');
 await runtime.chats.open(a.record.id);
 assert.equal((await rows(a.cwd)).length,1);
 assert.equal(runtime.chats.listInteractionAttachments(loss.record.id).receipts[0]!.status,'callback-handed');
 assert.deepEqual(await rows(loss.cwd),[]);
 await writeFile(join(dir,'receipt.json'),JSON.stringify({pass:true,syntheticCodexPeer:true,realHiveUploads:true,noModelCalls:true,copyAndNativeImage:true,validation:true,concurrentSubmit:true,staleResolution:['native-resolve','turn-end','abort','exit'],injectedTransportWriteFailure:true,injectedReceiptWriteFailure:true,partialUploadFailure:true,lateCopyLossSafeRetry:true,uploadSourceDigest:true,legacySchemaMigration:true,unrelatedReceiptCorruptionIsolated:true,exactCorruptOrPreparedReceiptBlocksDelivery:true,unrelatedCorruptionAllowsSafeRetry:true,unrelatedCorruptionPreservesColdHistory:true,coldReopenNoReplay:true},null,2));console.log(`INTERACTION_ATTACHMENTS_OK ${dir}`);
} finally {await runtime.close();}

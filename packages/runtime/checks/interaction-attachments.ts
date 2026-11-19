/** Controlled Codex JSONL peer and real Hive uploads/privateFS; no model calls. */
import assert from 'node:assert/strict';
import {mkdtemp, mkdir, writeFile, readFile, rename, unlink} from 'node:fs/promises';
import {join, resolve} from 'node:path';
import {createDesktopRuntime} from '../src/desktop-runtime.js';
import {CodexTransport} from '../src/agents/codex-transport.js';
import {randomUUID} from 'node:crypto';
import type {PiSessionHandle} from '../src/agents/types.js';

await mkdir(resolve('.artifacts'), {recursive:true});
const dir = await mkdtemp(resolve('.artifacts/question-attachments-'));
const peer = String.raw`
const {createInterface}=require('node:readline');const {existsSync,appendFileSync}=require('node:fs');
const thread={id:require('node:path').basename(process.cwd()),path:null,historyMode:'legacy',turns:[]};
const send=x=>process.stdout.write(JSON.stringify(x)+'\n');const notify=(method,params)=>send({method,params:{threadId:thread.id,...params}});
let turn,answered=false;const end=()=>{turn.status='interrupted';notify('turn/completed',{turn});};
const timer=setInterval(()=>{if(!turn||turn.status!=='inProgress')return;if(existsSync('exit'))process.exit(7);if(existsSync('turn-end'))end();else if(existsSync('native-resolve')){notify('serverRequest/resolved',{requestId:7});end();}else if(answered&&existsSync('finish')){notify('serverRequest/resolved',{requestId:7});end();}},10);
(async()=>{for await(const line of createInterface({input:process.stdin})){const q=JSON.parse(line);if(!q.method){appendFileSync('responses.jsonl',JSON.stringify(q)+'\n');answered=true;continue;}const answer=result=>send({id:q.id,result});
if(q.method==='initialize')answer({});else if(q.method==='model/list')answer({data:[{id:'terra',model:'gpt-6.1-terra',displayName:'Terra',hidden:false}],nextCursor:null});else if(q.method==='thread/start')answer({thread,model:'gpt-6.1-terra',modelProvider:'openai'});else if(q.method==='thread/read')answer({thread});else if(q.method==='thread/resume')answer({thread,model:'gpt-6.1-terra',modelProvider:'openai'});else if(q.method==='thread/settings/update')answer({});else if(q.method==='turn/interrupt'){answer({});notify('serverRequest/resolved',{requestId:7});end();}else if(q.method==='turn/start'){
  turn={id:'turn-'+thread.turns.length,status:'inProgress',items:[]};thread.turns.push(turn);answer({turn});notify('turn/started',{turn});
  const identity={threadId:thread.id,turnId:turn.id,itemId:'item'};
  if(q.params.input[0].text==='approval'){
    send({id:7,method:'item/commandExecution/requestApproval',params:{...identity,startedAtMs:1,availableDecisions:['decline']}});
  }else{
    send({id:7,method:'item/tool/requestUserInput',params:{...identity,isBlocking:true,questions:[{id:'public',header:'Public',question:'Choose',isSecret:false,isOther:true,options:null},{id:'secret',header:'Secret',question:'Secret',isSecret:true,isOther:true,options:null},{id:'extra',header:'Extra',question:'Extra',isSecret:false,isOther:true,options:null}]}});
  }
}}
clearInterval(timer);})().catch(e=>{console.error(e);process.exit(1)});
`;
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
    await assert.rejects(submission, {code: 'ENOENT'});
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

try {
 await runtime.settings.update({piCommand:null,codexCommand:process.execPath});
 await checkCopyLossBetweenQuestions();
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
 await writeFile(join(dir,'receipt.json'),JSON.stringify({pass:true,syntheticCodexPeer:true,realHiveUploads:true,noModelCalls:true,copyAndNativeImage:true,validation:true,concurrentSubmit:true,staleResolution:['native-resolve','turn-end','abort','exit'],injectedTransportWriteFailure:true,injectedReceiptWriteFailure:true,partialUploadFailure:true,lateCopyLossSafeRetry:true,unrelatedReceiptCorruptionIsolated:true,exactCorruptOrPreparedReceiptBlocksDelivery:true,unrelatedCorruptionAllowsSafeRetry:true,unrelatedCorruptionPreservesColdHistory:true,coldReopenNoReplay:true},null,2));console.log(`INTERACTION_ATTACHMENTS_OK ${dir}`);
} finally {await runtime.close();}

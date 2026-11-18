import { randomUUID } from 'node:crypto';
import { join } from 'node:path';
import { realpath } from 'node:fs/promises';
import { CodeCheckpoints, type ExportedCodeCheckpoint } from './code-checkpoints.js';
import { HistoryOperationJournal, HistoryOperationError, historyHash, type HistoryOperation } from './history-operation-journal.js';
import { openCodexHistoryControl, type CodexHistory } from './agents/codex-history-control.js';
import type { CodexTurn } from './agents/codex-history.js';
import { createForkCodexMetadata, readCodexMetadata } from './agents/codex-metadata.js';
import { CodexProjection } from './agents/codex-projection.js';
import { createForkWorktree, publishForkWorktree, readWorktreeIntent, type ManagedWorktree, type WorktreeCreation } from './git-worktrees.js';
import { materializeCheckpoint, verifyMaterializedCheckpoint } from './checkpoint-materialize.js';
import type { ChatRecord } from './chat-index.js';
import { inspectCheckpointRestore, restoreCheckpoint } from './checkpoint-restore.js';

export interface HistoryDraft {text:string;attachmentIds:string[];missingAttachments:string[]}
export interface HistoryPort {
 source(workspaceId:string,sessionId:string,rewindId?:string):Promise<{record:ChatRecord;executionPath:string;projectRoot:string;command:string}>;
 nativeLaunch?(record:ChatRecord):Promise<{command:string;args?:string[];env?:NodeJS.ProcessEnv}>;
 lease(workspaceId:string):()=>void;
 quiescent(workspaceId:string,idleSourceId?:string):Promise<string>;
 setup(workspaceId:string):Promise<WorktreeCreation['setup']>;
 draft(workspaceId:string,sessionId:string,itemId:string,turnId:string,turns:CodexTurn[]):Promise<HistoryDraft>;
 publish(record:ChatRecord,owned:ManagedWorktree):Promise<unknown>;
 session(recordId:string):Promise<unknown>;
 retire(sessionId: string): Promise<void>;
 guardRewind(sessionId: string, operationId: string, blocked: boolean): Promise<void>;
}
type ForkInput={workspaceId:string;sessionId:string;boundary:{nativeUserItemId:string};code:{kind:'current'}|{kind:'checkpoint';checkpointId:string};edit?:boolean;rewind?:boolean};
type StoredPreview = {
 input: ForkInput;
 source: Awaited<ReturnType<HistoryPort['source']>>;
 boundaryTurnId: string;
 lastTurnId?: string;
 nativePath?: string;
 sourceFingerprint: string;
 prefixFingerprint: string;
 metadataFingerprint: string;
 ledgerFingerprint: string;
 codeVersion: string;
 checkpointId?: string;
 draft?: HistoryDraft;
 expiresAt: number;
};
// Existing operations may still need recovery after an upgrade. Their saved
// prefix provides the same verification as the digest on new operations.
type LegacyStoredPreview = Omit<StoredPreview, 'prefixFingerprint' | 'lastTurnId'> & { prefix: CodexTurn[] };
function fail(code:string,message:string):never{throw new HistoryOperationError(code,message);}
function itemBoundary(turns:CodexTurn[],itemId:string,edit:boolean){
 const index=turns.findIndex(t=>t.items.some(i=>i.id===itemId));if(index<0)fail('BOUNDARY_NOT_FOUND','The exact native user message is unavailable');
 const selected=turns[index]!;const users=selected.items.filter(i=>i.type==='userMessage');
 if(users.length!==1||users[0]!.id!==itemId)fail('AMBIGUOUS_BOUNDARY','This native turn has multiple user inputs; it cannot be edited or forked at an exact user boundary');
 if(!turns.every(t=>['completed','interrupted','failed'].includes(t.status)))fail('SOURCE_BUSY','Native history contains an unsettled turn');
 return {turn:selected,prefix:turns.slice(0,index+(edit?0:1))};
}
/** Manages explicit conversation history changes without rewriting Hive collaboration facts. */
export class SessionHistoryOperations {
 readonly journal:HistoryOperationJournal;private readonly previews=new Map<string,StoredPreview>();private readonly running=new Map<string,Promise<unknown>>();
 constructor(private readonly dataDir:string,private readonly port:HistoryPort){this.journal=new HistoryOperationJournal(dataDir);}
 private closing=false;private readonly actions=new Set<Promise<unknown>>();
 private track<T>(action:()=>Promise<T>):Promise<T>{if(this.closing)return Promise.reject(new HistoryOperationError('CLOSING','History operations are shutting down'));const task=action();this.actions.add(task);return task.finally(()=>this.actions.delete(task));}
 async close(){this.closing=true;await Promise.allSettled([...this.actions]);this.previews.clear();}
 checkpointPreview(workspaceId:string,sessionId:string){return this.track(()=>this.checkpointPreviewAction(workspaceId,sessionId));}
 checkpointCapture(workspaceId:string,sessionId:string,expectedVersion:string,requestId:string){return this.track(()=>this.checkpointCaptureAction(workspaceId,sessionId,expectedVersion,requestId));}
 preview(input:ForkInput){return this.track(()=>this.previewAction(input));}
 apply(workspaceId:string,sessionId:string,previewToken:string,requestId:string,rewind=false){return this.track(()=>this.applyAction(workspaceId,sessionId,previewToken,requestId,rewind));}
 recover(workspaceId:string,operationId:string){return this.track(()=>this.recoverAction(workspaceId,operationId));}

 private async control(source:Awaited<ReturnType<HistoryPort['source']>>){
  const launch=this.port.nativeLaunch?await this.port.nativeLaunch(source.record):{command:source.command};
  return openCodexHistoryControl({cwd:source.executionPath,...launch});
 }
 private checkpoints(source:Awaited<ReturnType<HistoryPort['source']>>){return new CodeCheckpoints({dataDir:this.dataDir,workspaceId:source.record.workspaceId,sessionId:source.record.id,executionPath:source.executionPath});}
 async capabilities(workspaceId:string,sessionId:string){try{const source=await this.port.source(workspaceId,sessionId);return {fork:source.record.backend==='codex',edit:source.record.backend==='codex',checkpoint:true,rewind:source.record.backend==='codex'&&!!source.record.codexThreadId,rewindCode:source.record.backend==='codex'&&source.record.execution?.kind==='worktree',...(source.record.backend!=='codex'&&{reason:'This backend does not expose a supported native history fork'})};}catch(error){return {fork:false,edit:false,checkpoint:false,rewind:false,reason:error instanceof Error?error.message:String(error)};}}
 private async checkpointPreviewAction(workspaceId:string,sessionId:string){const release=this.port.lease(workspaceId);try{await this.port.quiescent(workspaceId,sessionId);return await this.checkpoints(await this.port.source(workspaceId,sessionId)).preview();}finally{release();}}
 private async checkpointCaptureAction(workspaceId:string,sessionId:string,expectedVersion:string,requestId:string){const release=this.port.lease(workspaceId);try{await this.port.quiescent(workspaceId,sessionId);return await this.checkpoints(await this.port.source(workspaceId,sessionId)).capture({expectedVersion,requestId});}finally{release();}}
 async checkpointList(workspaceId:string,sessionId:string){return {checkpoints:await this.checkpoints(await this.port.source(workspaceId,sessionId)).list()};}
 private async previewAction(input:ForkInput){
  if(!input||typeof input.boundary?.nativeUserItemId!=='string'||!input.boundary.nativeUserItemId||!input.code||!['current','checkpoint'].includes(input.code.kind)||input.edit!==undefined&&typeof input.edit!=='boolean'||input.rewind&&input.edit)fail('INVALID_ARGUMENT','Supply an exact native user boundary and explicit code source');
  const release=this.port.lease(input.workspaceId);let control:Awaited<ReturnType<typeof openCodexHistoryControl>>|undefined;
  try{
   const source=await this.port.source(input.workspaceId,input.sessionId);if(source.record.backend!=='codex'||!source.record.codexThreadId)fail('UNSUPPORTED_BACKEND','A persisted Codex native thread is required');
   if(input.rewind&&input.code.kind==='checkpoint'&&!source.record.execution)fail('SHARED_CHECKOUT','Restoring files requires a private session worktree');
   const ledgerFingerprint=await this.port.quiescent(input.workspaceId,input.rewind?input.sessionId:undefined);const cp=this.checkpoints(source);const scan=await cp.preview();if(!scan.canCapture)fail('CODE_UNSUPPORTED',scan.unsupported.map(x=>x.message).join('; '));
   if(scan.workspace.projectPrefix)fail('CODE_SCOPE_UNSUPPORTED','History forks currently require the repository root as the project directory');
   const chosen=input.code.kind==='checkpoint'?(await cp.reader(input.code.checkpointId)).manifest:undefined;
   if(chosen&&chosen.preview.workspace.commonDir!==scan.workspace.commonDir)fail('CHECKPOINT_IDENTITY','Checkpoint belongs to another repository');
   control=await this.control(source);const read=await control.read(source.record.codexThreadId);if(await realpath(read.thread.cwd)!==await realpath(source.executionPath))fail('NATIVE_SOURCE_MISMATCH','Native source thread belongs to another execution directory');
   const metadata=readCodexMetadata(join(this.dataDir,'chats','codex'),source.record.codexThreadId);
   const metadataFingerprint=historyHash(metadata);
   const boundary=itemBoundary(read.turns,input.boundary.nativeUserItemId,!!(input.edit||input.rewind));
   if(input.rewind&&[...metadata.deliveries,...(metadata.inheritedDeliveries??[])].some(delivery=>
    delivery.turnId===boundary.turn.id&&(!delivery.itemId||delivery.itemId===input.boundary.nativeUserItemId)&&delivery.presentation.origin!=='user')){
    fail('NOT_USER_INPUT','Select an original user message to rewind');
   }
   const draft=input.edit?await this.port.draft(input.workspaceId,input.sessionId,input.boundary.nativeUserItemId,boundary.turn.id,read.turns):undefined;
   if(historyHash(read.turns)!==historyHash((await control.read(source.record.codexThreadId)).turns)||ledgerFingerprint!==await this.port.quiescent(input.workspaceId,input.rewind?input.sessionId:undefined))fail('STALE_SOURCE','History or collaboration state changed during preview');
   const previewToken=randomUUID(),codeRelation={kind:input.code.kind,...(chosen&&{checkpointId:chosen.checkpointId}),label:chosen?'Historical conversation + selected code checkpoint':'Historical conversation + current code'};
   const publicValue={previewToken,sessionId:input.sessionId,nativeUserItemId:input.boundary.nativeUserItemId,edit:!!input.edit,codeRelation,exclusions:chosen?.preview.exclusions??scan.exclusions,...(draft&&{draft}),...(input.rewind&&{rewind:true,removedTurns:read.turns.length-boundary.prefix.length}),warnings:['Ignored files remain in the original directory and are excluded from the new worktree.','Hive tasks, dispatches and reports keep their current state.']};
   const last = boundary.prefix.at(-1);
   const stored:StoredPreview={input:structuredClone(input),source,boundaryTurnId:boundary.turn.id,
    ...(last&&{lastTurnId:last.id}),nativePath:read.thread.path,sourceFingerprint:historyHash(read.turns),prefixFingerprint:historyHash(boundary.prefix),
    metadataFingerprint,ledgerFingerprint,codeVersion:scan.version,...(chosen&&{checkpointId:chosen.checkpointId}),
    ...(draft&&{draft}),expiresAt:Date.now()+600000};
   if(this.previews.size>=100)this.previews.delete(this.previews.keys().next().value!);this.previews.set(previewToken,stored);return publicValue;
  }finally{try{await control?.close();}finally{release();}}
 }
 private async result(op:HistoryOperation){
  const preview = op.preview as StoredPreview;
  let session:unknown;let backupSession:unknown;let sessionUnavailable:string|undefined;
  if(op.state==='ready'){try{
   session=await this.port.session(preview.input.rewind?op.sourceSessionId:op.childSessionId);
   if(preview.input.rewind) backupSession=await this.port.session(op.childSessionId);
  }catch(error){sessionUnavailable=error instanceof Error?error.message:String(error);}}
  return {operationId:op.operationId,state:op.state,recoverable:!!op.childThreadId&&!!op.checkpointId&&['nativeKnown','metadataReady','uncertain'].includes(op.state),
   ...(preview.input.rewind&&{rewind:true,sourceSessionId:op.sourceSessionId}),...(backupSession!==undefined&&{backupSession}),
   ...(op.error&&{error:op.error}),...(sessionUnavailable&&{sessionUnavailable}),...(session!==undefined?{session,...(preview.draft?{draft:preview.draft}:{})}:{})};
 }
 async get(workspaceId:string,operationId:string){return this.result(await this.journal.byId(workspaceId,operationId));}
 async list(workspaceId:string){return {operations:await Promise.all((await this.journal.list(workspaceId)).map(op=>this.result(op)))};}
 private async applyAction(workspaceId:string,sessionId:string,previewToken:string,requestId:string,rewind:boolean):Promise<unknown>{
  const requestHash=historyHash({workspaceId,sessionId,previewToken});const prior=await this.journal.get(requestId);
  if(prior){if(prior.requestHash!==requestHash||prior.workspaceId!==workspaceId||prior.sourceSessionId!==sessionId||!!(prior.preview as StoredPreview).input.rewind!==rewind)fail('REQUEST_CONFLICT','This request identity belongs to a different history operation');return this.running.get(requestId)??this.result(prior);}
  const preview=this.previews.get(previewToken);if(!preview||preview.expiresAt<Date.now()||preview.input.workspaceId!==workspaceId||preview.input.sessionId!==sessionId||!!preview.input.rewind!==rewind)fail('PREVIEW_EXPIRED','Preview expired; inspect the source again');
  const claim=await this.journal.claim({workspaceId,sourceSessionId:sessionId,requestId,requestHash,preview});if(!claim.fresh)return this.result(claim.operation);
  this.previews.delete(previewToken);
  const task=this.execute(claim.operation,preview).finally(()=>this.running.delete(requestId));this.running.set(requestId,task);return task;
 }
 private childRecord(op:HistoryOperation,preview:Pick<StoredPreview,'input'|'source'|'draft'>,owned:ManagedWorktree,turns:CodexTurn[]):ChatRecord {
  return {...(preview.draft&&{forkDraft:preview.draft}),id:op.childSessionId,workspaceId:op.workspaceId,title:preview.source.record.title+(preview.input.rewind?' · before rewind':' · fork'),backend:'codex',...(preview.source.record.profile&&{profile:preview.source.record.profile}),...(preview.source.record.legacyConfiguration&&{legacyConfiguration:true}),sessionFile:null,codexThreadId:op.childThreadId!,originAgentId:preview.source.record.originAgentId!,identityState:'known',updatedAt:Date.now(),...(preview.source.record.model&&{model:preview.source.record.model}),...(preview.source.record.reasoning&&{reasoning:preview.source.record.reasoning}),...(preview.source.record.nativeMode&&{nativeMode:preview.source.record.nativeMode}),execution:{kind:'worktree',path:owned.executionPath,branch:owned.branch},lineage:{operationId:op.operationId,parentSessionId:preview.source.record.id,nativeUserItemId:preview.input.boundary.nativeUserItemId,edit:!!preview.input.edit,codeRelation:{kind:preview.input.rewind?'current':preview.input.code.kind,checkpointId:op.checkpointId!},inheritedItemIds:turns.flatMap(t=>t.items.map(i=>i.id))}};
 }
 private async recoverAction(workspaceId:string,operationId:string):Promise<unknown>{
  let op=await this.journal.byId(workspaceId,operationId);
  if(this.running.has(op.requestId))return this.running.get(op.requestId);
  if(op.state==='ready')return this.result(op);
  if(!op.childThreadId||!op.checkpointId||!['nativeKnown','metadataReady','uncertain'].includes(op.state))return this.result(op);
  const task=((op.preview as StoredPreview).input.rewind?this.recoverRewind(op):this.recoverKnown(op)).finally(()=>this.running.delete(op.requestId));this.running.set(op.requestId,task);return task;
 }

 private async rewindCodeInput(op: HistoryOperation, preview: StoredPreview, cp: CodeCheckpoints) {
  if (!preview.checkpointId) return;
  return {dataRoot: this.dataDir, workspaceId: op.workspaceId, sessionId: op.sourceSessionId,
   operationId: op.operationId, checkpoints: cp, future: await cp.reader(op.checkpointId!), target: await cp.reader(preview.checkpointId)};
 }

 private async finishRewind(op: HistoryOperation, preview: StoredPreview, current: CodexHistory, backup: CodexHistory, owned: ManagedWorktree) {
  const source=await this.port.source(op.workspaceId,op.sourceSessionId,op.operationId);
  if(historyHash(source)!==historyHash(preview.source)||await this.port.quiescent(op.workspaceId,op.sourceSessionId)!==preview.ledgerFingerprint)fail('STALE_SOURCE','Source identity or collaboration changed during rewind');
  if (current.thread.id !== source.record.codexThreadId || current.thread.path === backup.thread.path
   || await realpath(current.thread.cwd) !== await realpath(source.executionPath)) fail('NATIVE_SOURCE_MISMATCH','The original native thread identity changed');
  const cp=this.checkpoints(source), observed=historyHash(current.turns);
  if (observed === preview.sourceFingerprint) {
   if (current.thread.path !== preview.nativePath || (await cp.preview()).version !== preview.codeVersion) fail('STALE_SOURCE','Source rollout or code changed; keep this rewind paused for inspection');
   const failed=await this.journal.save({...op,state:'failed',error:'Native history is unchanged. To try again, create and confirm a new preview.'});
   await this.port.guardRewind(op.sourceSessionId,op.operationId,false).catch(()=>{});
   return this.result(failed);
  }
  if (!op.rewindRequested || observed !== preview.prefixFingerprint || historyHash(backup.turns) !== preview.sourceFingerprint) fail('NATIVE_PREFIX_MISMATCH','The original and backup do not prove the confirmed rewind; no native request was repeated');
  const directory=join(this.dataDir,'chats','codex');
  if(historyHash(readCodexMetadata(directory,current.thread.id))!==preview.metadataFingerprint)fail('STALE_SOURCE','Original delivery metadata changed; it was not rewritten');
  const restore = await this.rewindCodeInput(op, preview, cp);
  if (restore) await restoreCheckpoint(restore);
  else if ((await cp.preview()).version !== preview.codeVersion) fail('STALE_SOURCE','Current code changed during the rewind');
  if(historyHash(readCodexMetadata(directory,current.thread.id))!==preview.metadataFingerprint)fail('STALE_SOURCE','Original delivery metadata changed; it was not rewritten');
  // Project the new branch but retain all native completion receipts and Hive facts.
  new CodexProjection(directory,current.thread.id).recover(current.turns,true);
  createForkCodexMetadata(directory,current.thread.id,backup.thread.id,source.record.id,backup.turns);
  if(await this.port.quiescent(op.workspaceId,op.sourceSessionId)!==preview.ledgerFingerprint)fail('STALE_SOURCE','Collaboration changed before rewind publication');
  await publishForkWorktree(this.dataDir,owned,op.operationId);
  await this.port.publish(this.childRecord(op,preview,owned,backup.turns),owned);
  const {error:_error,...done}=op;
  const ready=await this.journal.save({...done,state:'ready'});
  await this.port.guardRewind(op.sourceSessionId,op.operationId,false).catch(()=>{});
  return this.result(ready);
 }

 private async recoverRewind(initial: HistoryOperation) {
  const op=initial, preview=op.preview as StoredPreview;
  const release=this.port.lease(op.workspaceId);
  let control:Awaited<ReturnType<typeof openCodexHistoryControl>>|undefined;
  try {
   const source=await this.port.source(op.workspaceId,op.sourceSessionId,op.operationId);
   if(historyHash(source)!==historyHash(preview.source)||await this.port.quiescent(op.workspaceId,op.sourceSessionId)!==preview.ledgerFingerprint)fail('STALE_SOURCE','The original source or collaboration state changed');
   await this.port.retire(op.sourceSessionId);
   const checkpoint=await this.checkpoints(source).export(op.checkpointId!);
   const {record:owned,creation}=await readWorktreeIntent(this.dataDir,op.childSessionId);
   if(creation?.forkOperationId!==op.operationId)fail('FORK_IDENTITY','Backup worktree belongs to another operation');
   await verifyMaterializedCheckpoint({dataRoot:this.dataDir,record:owned,operationId:op.operationId,checkpoint});
   control=await this.control(source);
   const backup=await control.read(op.childThreadId!);
   if(backup.thread.forkedFromId!==source.record.codexThreadId||backup.thread.path===preview.nativePath||await realpath(backup.thread.cwd)!==await realpath(owned.executionPath)||historyHash(backup.turns)!==preview.sourceFingerprint)fail('NATIVE_PREFIX_MISMATCH','The known backup does not contain the complete original history');
   const current=await control.read(source.record.codexThreadId!);
   return await this.finishRewind(op,preview,current,backup,owned);
  }catch(error){return this.result(await this.journal.save({...op,state:'uncertain',error:error instanceof Error?error.message:String(error)}));}
  finally{try{await control?.close();}finally{release();}}
 }
 private async recoverKnown(initial:HistoryOperation){
  let op=initial;const workspaceId=op.workspaceId,operationId=op.operationId,childThreadId=op.childThreadId!,checkpointId=op.checkpointId!;
  const preview=op.preview as StoredPreview | LegacyStoredPreview;let release:(()=>void)|undefined;let control:Awaited<ReturnType<typeof openCodexHistoryControl>>|undefined;
  try{
   release=this.port.lease(workspaceId);
   const source=await this.port.source(workspaceId,op.sourceSessionId);
   if(historyHash(source)!==historyHash(preview.source)||await this.port.quiescent(workspaceId)!==preview.ledgerFingerprint)fail('STALE_SOURCE','The original source or collaboration state changed; the child remains pending inspection');
   const cp=this.checkpoints(source);if((await cp.preview()).version!==preview.codeVersion)fail('STALE_SOURCE','Source code changed; the child remains pending inspection');
   const checkpoint=await cp.export(checkpointId);const {record:owned,creation}=await readWorktreeIntent(this.dataDir,op.childSessionId);
   if(creation?.forkOperationId!==op.operationId)fail('FORK_IDENTITY','Worktree belongs to another operation');
   await verifyMaterializedCheckpoint({dataRoot:this.dataDir,record:owned,operationId,checkpoint});
   control=await this.control(source);
   const child=await control.read(childThreadId);
   const prefixFingerprint = 'prefixFingerprint' in preview ? preview.prefixFingerprint : historyHash(preview.prefix);
   if(child.thread.forkedFromId!==source.record.codexThreadId||await realpath(child.thread.cwd)!==await realpath(owned.executionPath)||historyHash(child.turns)!==prefixFingerprint)fail('NATIVE_PREFIX_MISMATCH','The known native child is not the exact selected prefix; no fork or revert was retried');
   const nativeSource=await control.read(source.record.codexThreadId!);
   if(child.thread.path===nativeSource.thread.path||historyHash(nativeSource.turns)!==preview.sourceFingerprint||historyHash(readCodexMetadata(join(this.dataDir,'chats','codex'),source.record.codexThreadId!))!==preview.metadataFingerprint)fail('STALE_SOURCE','Source history or display provenance changed');
   createForkCodexMetadata(join(this.dataDir,'chats','codex'),source.record.codexThreadId!,childThreadId,source.record.id,child.turns);
   op=await this.journal.save({...op,state:'metadataReady'});
   await verifyMaterializedCheckpoint({dataRoot:this.dataDir,record:owned,operationId,checkpoint});
   if(await this.port.quiescent(workspaceId)!==preview.ledgerFingerprint||(await cp.preview()).version!==preview.codeVersion)fail('STALE_SOURCE','Source changed before recovered publication');
   await publishForkWorktree(this.dataDir,owned,operationId);await this.port.publish(this.childRecord(op,preview,owned,child.turns),owned);
   const {error:previousError,...recovered}=op;op=await this.journal.save({...recovered,state:'ready'});return this.result(op);
  }catch(error){op=await this.journal.save({...op,state:'uncertain',error:error instanceof Error?error.message:String(error)});return this.result(op);}
  finally{try{await control?.close();}finally{release?.();}}
 }
 private async execute(initial:HistoryOperation,preview:StoredPreview){let op=initial;let release:(()=>void)|undefined;let control:Awaited<ReturnType<typeof openCodexHistoryControl>>|undefined;
  try{
   release=this.port.lease(op.workspaceId);
   const source=await this.port.source(op.workspaceId,op.sourceSessionId);if(historyHash(source)!==historyHash(preview.source)||await this.port.quiescent(op.workspaceId,preview.input.rewind?op.sourceSessionId:undefined)!==preview.ledgerFingerprint)fail('STALE_SOURCE','Source identity or collaboration state changed since preview');
   const cp=this.checkpoints(source);if((await cp.preview()).version!==preview.codeVersion)fail('STALE_SOURCE','Current code changed since preview');
   control=await this.control(source);if(historyHash((await control.read(source.record.codexThreadId!)).turns)!==preview.sourceFingerprint)fail('STALE_SOURCE','Native history changed since preview');
   let checkpoint:ExportedCodeCheckpoint;if(preview.checkpointId&&!preview.input.rewind)checkpoint=await cp.export(preview.checkpointId);else{const captured=await cp.capture({expectedVersion:preview.codeVersion,requestId:op.operationId});checkpoint=await cp.export(captured.checkpointId);}
   op=await this.journal.save({...op,checkpointId:checkpoint.manifest.checkpointId});
   if(preview.input.rewind){const restore=await this.rewindCodeInput(op,preview,cp);if(restore)await inspectCheckpointRestore(restore);}
   const setup=await this.port.setup(op.workspaceId);const creation:WorktreeCreation={version:setup?2:1,...(setup&&{setup}),forkOperationId:op.operationId,workspaceId:op.workspaceId,backend:'codex',...(source.record.profile&&{profile:source.record.profile}),...(source.record.legacyConfiguration&&{legacyConfiguration:true}),identityState:'known',originAgentId:source.record.originAgentId!,...(source.record.model&&{model:source.record.model})};
   const owned=await createForkWorktree({projectRoot:source.projectRoot,dataRoot:this.dataDir,chatId:op.childSessionId,creation,operationId:op.operationId,baseOid:checkpoint.manifest.preview.workspace.headOid!});
   await materializeCheckpoint({dataRoot:this.dataDir,record:owned,operationId:op.operationId,checkpoint});op=await this.journal.save({...op,state:'worktreeReady'});
   if(await this.port.quiescent(op.workspaceId,preview.input.rewind?op.sourceSessionId:undefined)!==preview.ledgerFingerprint||(await cp.preview()).version!==preview.codeVersion||historyHash((await control.read(source.record.codexThreadId!)).turns)!==preview.sourceFingerprint)fail('STALE_SOURCE','Source changed before native fork; the unpublished worktree is retained');
   const child=await control.fork({threadId:source.record.codexThreadId!,cwd:owned.executionPath,...(preview.input.rewind?{}:preview.lastTurnId?{lastTurnId:preview.lastTurnId}:{beforeFirstTurnId:preview.boundaryTurnId}),onNativeRequested:async()=>{op=await this.journal.save({...op,state:'nativeRequested'});},onNativeKnown:async childThreadId=>{op=await this.journal.save({...op,state:'nativeKnown',childThreadId});}});
   if(historyHash(child.turns)!==(preview.input.rewind?preview.sourceFingerprint:preview.prefixFingerprint))fail('NATIVE_PREFIX_MISMATCH','Native child does not contain the exact expected history');
   if(historyHash(readCodexMetadata(join(this.dataDir,'chats','codex'),source.record.codexThreadId!))!==preview.metadataFingerprint)fail('STALE_SOURCE','Native display provenance changed since preview');
   createForkCodexMetadata(join(this.dataDir,'chats','codex'),source.record.codexThreadId!,op.childThreadId!,source.record.id,child.turns);
   op=await this.journal.save({...op,state:'metadataReady'});
   if(await this.port.quiescent(op.workspaceId,preview.input.rewind?op.sourceSessionId:undefined)!==preview.ledgerFingerprint||(await cp.preview()).version!==preview.codeVersion)fail('STALE_SOURCE','Source changed before child publication; native child retained for inspection');
   await verifyMaterializedCheckpoint({dataRoot:this.dataDir,record:owned,operationId:op.operationId,checkpoint});
   if(preview.input.rewind){
    await this.port.retire(op.sourceSessionId);
    const current=await control.rewind({threadId:source.record.codexThreadId!,beforeTurnId:preview.boundaryTurnId,expectedSource:preview.sourceFingerprint,fingerprint:historyHash,onRequested:async()=>{
     if(await this.port.quiescent(op.workspaceId,op.sourceSessionId)!==preview.ledgerFingerprint||(await cp.preview()).version!==preview.codeVersion)fail('STALE_SOURCE','Source changed after backup and before rewind');
     await this.port.guardRewind(op.sourceSessionId,op.operationId,true);
     op=await this.journal.save({...op,rewindRequested:true});
    }});
    return await this.finishRewind(op,preview,current,child,owned);
   }
   const record=this.childRecord(op,preview,owned,child.turns);
   await publishForkWorktree(this.dataDir,owned,op.operationId);await this.port.publish(record,owned);op=await this.journal.save({...op,state:'ready'});return this.result(op);
  }catch(error){const uncertain=['nativeRequested','nativeKnown','metadataReady'].includes(op.state);op=await this.journal.save({...op,state:uncertain?'uncertain':'failed',error:error instanceof Error?error.message:String(error)});return this.result(op);
  }finally{try{await control?.close();}finally{release?.();}}
 }
}

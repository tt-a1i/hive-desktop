import { randomUUID } from 'node:crypto';
import { join } from 'node:path';
import { realpath } from 'node:fs/promises';
import { CodeCheckpoints, type ExportedCodeCheckpoint } from './code-checkpoints.js';
import { HistoryOperationJournal, HistoryOperationError, historyHash, type HistoryOperation } from './history-operation-journal.js';
import { openCodexHistoryControl } from './agents/codex-history-control.js';
import type { CodexTurn } from './agents/codex-history.js';
import { createForkCodexMetadata, readCodexMetadata } from './agents/codex-metadata.js';
import { createForkWorktree, publishForkWorktree, readWorktreeIntent, type ManagedWorktree, type WorktreeCreation } from './git-worktrees.js';
import { materializeCheckpoint, verifyMaterializedCheckpoint } from './checkpoint-materialize.js';
import type { ChatRecord } from './chat-index.js';

export interface HistoryDraft {text:string;attachmentIds:string[];missingAttachments:string[]}
export interface HistoryPort {
 source(workspaceId:string,sessionId:string):Promise<{record:ChatRecord;executionPath:string;projectRoot:string;command:string}>;
 lease(workspaceId:string):()=>void;
 quiescent(workspaceId:string):Promise<string>;
 setup(workspaceId:string):Promise<WorktreeCreation['setup']>;
 draft(workspaceId:string,sessionId:string,itemId:string,turnId:string,turns:CodexTurn[]):Promise<HistoryDraft>;
 publish(record:ChatRecord,owned:ManagedWorktree):Promise<unknown>;
 session(recordId:string):Promise<unknown>;
}
type ForkInput={workspaceId:string;sessionId:string;boundary:{nativeUserItemId:string};code:{kind:'current'}|{kind:'checkpoint';checkpointId:string};edit?:boolean};
type StoredPreview={input:ForkInput;source:Awaited<ReturnType<HistoryPort['source']>>;turns:CodexTurn[];prefix:CodexTurn[];boundaryTurnId:string;sourceFingerprint:string;metadataFingerprint:string;ledgerFingerprint:string;codeVersion:string;checkpointId?:string;draft?:HistoryDraft;expiresAt:number;public:Record<string,unknown>};
function fail(code:string,message:string):never{throw new HistoryOperationError(code,message);}
function itemBoundary(turns:CodexTurn[],itemId:string,edit:boolean){
 const index=turns.findIndex(t=>t.items.some(i=>i.id===itemId));if(index<0)fail('BOUNDARY_NOT_FOUND','The exact native user message is unavailable');
 const selected=turns[index]!;const users=selected.items.filter(i=>i.type==='userMessage');
 if(users.length!==1||users[0]!.id!==itemId)fail('AMBIGUOUS_BOUNDARY','This native turn has multiple user inputs; it cannot be edited or forked at an exact user boundary');
 if(!turns.every(t=>['completed','interrupted','failed'].includes(t.status)))fail('SOURCE_BUSY','Native history contains an unsettled turn');
 return {turn:selected,prefix:turns.slice(0,index+(edit?0:1))};
}
/** Forks conversation context and private code only. It never starts an Agent or writes Hive facts. */
export class SessionHistoryOperations {
 readonly journal:HistoryOperationJournal;private readonly previews=new Map<string,StoredPreview>();private readonly running=new Map<string,Promise<unknown>>();
 constructor(private readonly dataDir:string,private readonly port:HistoryPort){this.journal=new HistoryOperationJournal(dataDir);}
 private checkpoints(source:Awaited<ReturnType<HistoryPort['source']>>){return new CodeCheckpoints({dataDir:this.dataDir,workspaceId:source.record.workspaceId,sessionId:source.record.id,executionPath:source.executionPath});}
 async capabilities(workspaceId:string,sessionId:string){try{const source=await this.port.source(workspaceId,sessionId);return {fork:source.record.backend==='codex',edit:source.record.backend==='codex',checkpoint:true,rewind:false,...(source.record.backend!=='codex'&&{reason:'This backend does not expose a supported native history fork'})};}catch(error){return {fork:false,edit:false,checkpoint:false,rewind:false,reason:error instanceof Error?error.message:String(error)};}}
 async checkpointPreview(workspaceId:string,sessionId:string){const release=this.port.lease(workspaceId);try{await this.port.quiescent(workspaceId);return await this.checkpoints(await this.port.source(workspaceId,sessionId)).preview();}finally{release();}}
 async checkpointCapture(workspaceId:string,sessionId:string,expectedVersion:string,requestId:string){const release=this.port.lease(workspaceId);try{await this.port.quiescent(workspaceId);return await this.checkpoints(await this.port.source(workspaceId,sessionId)).capture({expectedVersion,requestId});}finally{release();}}
 async checkpointList(workspaceId:string,sessionId:string){return {checkpoints:await this.checkpoints(await this.port.source(workspaceId,sessionId)).list()};}
 async preview(input:ForkInput){
  if(!input||typeof input.boundary?.nativeUserItemId!=='string'||!input.boundary.nativeUserItemId||!input.code||!['current','checkpoint'].includes(input.code.kind)||input.edit!==undefined&&typeof input.edit!=='boolean')fail('INVALID_ARGUMENT','Supply an exact native user boundary and explicit code source');
  const release=this.port.lease(input.workspaceId);let control:Awaited<ReturnType<typeof openCodexHistoryControl>>|undefined;
  try{
   const source=await this.port.source(input.workspaceId,input.sessionId);if(source.record.backend!=='codex'||!source.record.codexThreadId)fail('UNSUPPORTED_BACKEND','A persisted Codex native thread is required');
   const ledgerFingerprint=await this.port.quiescent(input.workspaceId);const cp=this.checkpoints(source);const scan=await cp.preview();if(!scan.canCapture)fail('CODE_UNSUPPORTED',scan.unsupported.map(x=>x.message).join('; '));
   if(scan.workspace.projectPrefix)fail('CODE_SCOPE_UNSUPPORTED','History forks currently require the repository root as the project directory');
   const chosen=input.code.kind==='checkpoint'?await cp.export(input.code.checkpointId):undefined;
   if(chosen&&chosen.manifest.preview.workspace.commonDir!==scan.workspace.commonDir)fail('CHECKPOINT_IDENTITY','Checkpoint belongs to another repository');
   control=await openCodexHistoryControl({cwd:source.executionPath,command:source.command});const read=await control.read(source.record.codexThreadId);if(await realpath(read.thread.cwd)!==await realpath(source.executionPath))fail('NATIVE_SOURCE_MISMATCH','Native source thread belongs to another execution directory');const metadataFingerprint=historyHash(readCodexMetadata(join(this.dataDir,'chats','codex'),source.record.codexThreadId));const boundary=itemBoundary(read.turns,input.boundary.nativeUserItemId,!!input.edit);
   const draft=input.edit?await this.port.draft(input.workspaceId,input.sessionId,input.boundary.nativeUserItemId,boundary.turn.id,read.turns):undefined;
   if(historyHash(read.turns)!==historyHash((await control.read(source.record.codexThreadId)).turns)||ledgerFingerprint!==await this.port.quiescent(input.workspaceId))fail('STALE_SOURCE','History or collaboration state changed during preview');
   const previewToken=randomUUID(),codeRelation={kind:input.code.kind,...(chosen&&{checkpointId:chosen.manifest.checkpointId}),label:chosen?'Historical conversation + selected code checkpoint':'Historical conversation + current code'};
   const publicValue={previewToken,sessionId:input.sessionId,nativeUserItemId:input.boundary.nativeUserItemId,edit:!!input.edit,codeRelation,exclusions:chosen?.manifest.preview.exclusions??scan.exclusions,...(draft&&{draft}),warnings:['Ignored files remain in the original directory and are excluded from the new worktree.','Hive tasks, dispatches and reports keep their current state.']};
   const stored:StoredPreview={input:structuredClone(input),source,turns:read.turns,prefix:boundary.prefix,boundaryTurnId:boundary.turn.id,sourceFingerprint:historyHash(read.turns),metadataFingerprint,ledgerFingerprint,codeVersion:scan.version,...(chosen&&{checkpointId:chosen.manifest.checkpointId}),...(draft&&{draft}),expiresAt:Date.now()+600000,public:publicValue};
   if(this.previews.size>=100)this.previews.delete(this.previews.keys().next().value!);this.previews.set(previewToken,stored);return publicValue;
  }finally{await control?.close();release();}
 }
 private async result(op:HistoryOperation){let session:unknown;let sessionUnavailable:string|undefined;if(op.state==='ready'){try{session=await this.port.session(op.childSessionId);}catch(error){sessionUnavailable=error instanceof Error?error.message:String(error);}}return {operationId:op.operationId,state:op.state,recoverable:!!op.childThreadId&&!!op.checkpointId&&['nativeKnown','metadataReady','uncertain'].includes(op.state),...(op.error&&{error:op.error}),...(sessionUnavailable&&{sessionUnavailable}),...(session!==undefined?{session,...((op.preview as StoredPreview).draft?{draft:(op.preview as StoredPreview).draft}:{})}:{})};}
 async get(workspaceId:string,operationId:string){return this.result(await this.journal.byId(workspaceId,operationId));}
 async list(workspaceId:string){return {operations:await Promise.all((await this.journal.list(workspaceId)).map(op=>this.result(op)))};}
 async apply(workspaceId:string,sessionId:string,previewToken:string,requestId:string):Promise<unknown>{
  const requestHash=historyHash({workspaceId,sessionId,previewToken});const prior=await this.journal.get(requestId);
  if(prior){if(prior.requestHash!==requestHash||prior.workspaceId!==workspaceId||prior.sourceSessionId!==sessionId)fail('REQUEST_CONFLICT','This request identity belongs to a different history operation');return this.running.get(requestId)??this.result(prior);}
  const preview=this.previews.get(previewToken);if(!preview||preview.expiresAt<Date.now()||preview.input.workspaceId!==workspaceId||preview.input.sessionId!==sessionId)fail('PREVIEW_EXPIRED','Preview expired; inspect the source again');
  const claim=await this.journal.claim({workspaceId,sourceSessionId:sessionId,requestId,requestHash,preview});if(!claim.fresh)return this.result(claim.operation);
  const task=this.execute(claim.operation,preview).finally(()=>this.running.delete(requestId));this.running.set(requestId,task);return task;
 }
 private childRecord(op:HistoryOperation,preview:StoredPreview,owned:ManagedWorktree,turns:CodexTurn[]):ChatRecord {
  return {...(preview.draft&&{forkDraft:preview.draft}),id:op.childSessionId,workspaceId:op.workspaceId,title:preview.source.record.title+' · fork',backend:'codex',sessionFile:null,codexThreadId:op.childThreadId!,originAgentId:preview.source.record.originAgentId!,identityState:'known',updatedAt:Date.now(),...(preview.source.record.model&&{model:preview.source.record.model}),...(preview.source.record.reasoning&&{reasoning:preview.source.record.reasoning}),...(preview.source.record.nativeMode&&{nativeMode:preview.source.record.nativeMode}),execution:{kind:'worktree',path:owned.executionPath,branch:owned.branch},lineage:{operationId:op.operationId,parentSessionId:preview.source.record.id,nativeUserItemId:preview.input.boundary.nativeUserItemId,edit:!!preview.input.edit,codeRelation:{kind:preview.input.code.kind,checkpointId:op.checkpointId!},inheritedItemIds:turns.flatMap(t=>t.items.map(i=>i.id))}};
 }
 async recover(workspaceId:string,operationId:string):Promise<unknown>{
  let op=await this.journal.byId(workspaceId,operationId);
  if(this.running.has(op.requestId))return this.running.get(op.requestId);
  if(op.state==='ready')return this.result(op);
  if(!op.childThreadId||!op.checkpointId||!['nativeKnown','metadataReady','uncertain'].includes(op.state))return this.result(op);
  const preview=op.preview as StoredPreview;let release:(()=>void)|undefined;let control:Awaited<ReturnType<typeof openCodexHistoryControl>>|undefined;
  try{
   release=this.port.lease(workspaceId);
   const source=await this.port.source(workspaceId,op.sourceSessionId);
   if(historyHash(source)!==historyHash(preview.source)||await this.port.quiescent(workspaceId)!==preview.ledgerFingerprint)fail('STALE_SOURCE','The original source or collaboration state changed; the child remains pending inspection');
   const cp=this.checkpoints(source);if((await cp.preview()).version!==preview.codeVersion)fail('STALE_SOURCE','Source code changed; the child remains pending inspection');
   const checkpoint=await cp.export(op.checkpointId);const {record:owned,creation}=await readWorktreeIntent(this.dataDir,op.childSessionId);
   if(creation?.forkOperationId!==op.operationId)fail('FORK_IDENTITY','Worktree belongs to another operation');
   await verifyMaterializedCheckpoint({dataRoot:this.dataDir,record:owned,operationId,checkpoint});
   control=await openCodexHistoryControl({cwd:source.executionPath,command:source.command});
   const child=await control.read(op.childThreadId);
   if(child.thread.forkedFromId!==source.record.codexThreadId||await realpath(child.thread.cwd)!==await realpath(owned.executionPath)||historyHash(child.turns)!==historyHash(preview.prefix))fail('NATIVE_PREFIX_MISMATCH','The known native child is not the exact selected prefix; no fork or revert was retried');
   const nativeSource=await control.read(source.record.codexThreadId!);
   if(child.thread.path===nativeSource.thread.path||historyHash(nativeSource.turns)!==preview.sourceFingerprint||historyHash(readCodexMetadata(join(this.dataDir,'chats','codex'),source.record.codexThreadId!))!==preview.metadataFingerprint)fail('STALE_SOURCE','Source history or display provenance changed');
   createForkCodexMetadata(join(this.dataDir,'chats','codex'),source.record.codexThreadId!,op.childThreadId,source.record.id,child.turns);
   op=await this.journal.save({...op,state:'metadataReady'});
   await verifyMaterializedCheckpoint({dataRoot:this.dataDir,record:owned,operationId,checkpoint});
   if(await this.port.quiescent(workspaceId)!==preview.ledgerFingerprint||(await cp.preview()).version!==preview.codeVersion)fail('STALE_SOURCE','Source changed before recovered publication');
   await publishForkWorktree(this.dataDir,owned,operationId);await this.port.publish(this.childRecord(op,preview,owned,child.turns),owned);
   op=await this.journal.save({...op,state:'ready'});return this.result(op);
  }catch(error){op=await this.journal.save({...op,state:'uncertain',error:error instanceof Error?error.message:String(error)});return this.result(op);}
  finally{await control?.close();release?.();}
 }
 private async execute(initial:HistoryOperation,preview:StoredPreview){let op=initial;let release:(()=>void)|undefined;let control:Awaited<ReturnType<typeof openCodexHistoryControl>>|undefined;
  try{
   release=this.port.lease(op.workspaceId);
   const source=await this.port.source(op.workspaceId,op.sourceSessionId);if(historyHash(source)!==historyHash(preview.source)||await this.port.quiescent(op.workspaceId)!==preview.ledgerFingerprint)fail('STALE_SOURCE','Source identity or collaboration state changed since preview');
   const cp=this.checkpoints(source);if((await cp.preview()).version!==preview.codeVersion)fail('STALE_SOURCE','Current code changed since preview');
   control=await openCodexHistoryControl({cwd:source.executionPath,command:source.command});if(historyHash((await control.read(source.record.codexThreadId!)).turns)!==preview.sourceFingerprint)fail('STALE_SOURCE','Native history changed since preview');
   let checkpoint:ExportedCodeCheckpoint;if(preview.checkpointId)checkpoint=await cp.export(preview.checkpointId);else{const captured=await cp.capture({expectedVersion:preview.codeVersion,requestId:op.operationId});checkpoint=await cp.export(captured.checkpointId);}
   op=await this.journal.save({...op,checkpointId:checkpoint.manifest.checkpointId});
   const setup=await this.port.setup(op.workspaceId);const creation:WorktreeCreation={version:setup?2:1,...(setup&&{setup}),forkOperationId:op.operationId,workspaceId:op.workspaceId,backend:'codex',identityState:'known',originAgentId:source.record.originAgentId!,...(source.record.model&&{model:source.record.model})};
   const owned=await createForkWorktree({projectRoot:source.projectRoot,dataRoot:this.dataDir,chatId:op.childSessionId,creation,operationId:op.operationId,baseOid:checkpoint.manifest.preview.workspace.headOid!});
   await materializeCheckpoint({dataRoot:this.dataDir,record:owned,operationId:op.operationId,checkpoint});op=await this.journal.save({...op,state:'worktreeReady'});
   if(await this.port.quiescent(op.workspaceId)!==preview.ledgerFingerprint||(await cp.preview()).version!==preview.codeVersion||historyHash((await control.read(source.record.codexThreadId!)).turns)!==preview.sourceFingerprint)fail('STALE_SOURCE','Source changed before native fork; the unpublished worktree is retained');
   const last=preview.prefix.at(-1);const child=await control.fork({threadId:source.record.codexThreadId!,cwd:owned.executionPath,...(last?{lastTurnId:last.id}:{beforeFirstTurnId:preview.boundaryTurnId}),onNativeRequested:async()=>{op=await this.journal.save({...op,state:'nativeRequested'});},onNativeKnown:async childThreadId=>{op=await this.journal.save({...op,state:'nativeKnown',childThreadId});}});
   if(historyHash(child.turns)!==historyHash(preview.prefix))fail('NATIVE_PREFIX_MISMATCH','Native child does not contain the exact selected history prefix');
   if(historyHash(readCodexMetadata(join(this.dataDir,'chats','codex'),source.record.codexThreadId!))!==preview.metadataFingerprint)fail('STALE_SOURCE','Native display provenance changed since preview');
   createForkCodexMetadata(join(this.dataDir,'chats','codex'),source.record.codexThreadId!,op.childThreadId!,source.record.id,child.turns);
   op=await this.journal.save({...op,state:'metadataReady'});
   if(await this.port.quiescent(op.workspaceId)!==preview.ledgerFingerprint||(await cp.preview()).version!==preview.codeVersion)fail('STALE_SOURCE','Source changed before child publication; native child retained for inspection');
   const record=this.childRecord(op,preview,owned,child.turns);
   await verifyMaterializedCheckpoint({dataRoot:this.dataDir,record:owned,operationId:op.operationId,checkpoint});
   await publishForkWorktree(this.dataDir,owned,op.operationId);await this.port.publish(record,owned);op=await this.journal.save({...op,state:'ready'});return this.result(op);
  }catch(error){const uncertain=['nativeRequested','nativeKnown','metadataReady'].includes(op.state);op=await this.journal.save({...op,state:uncertain?'uncertain':'failed',error:error instanceof Error?error.message:String(error)});return this.result(op);
  }finally{await control?.close();release?.();}
 }
}

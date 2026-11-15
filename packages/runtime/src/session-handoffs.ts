import {randomUUID} from 'node:crypto';
import type {AgentDriver,AgentProfileRef} from './agent-profiles.js';
import type {CodeCheckpoints,ExportedCodeCheckpoint} from './code-checkpoints.js';
import type {HandoffHistorySnapshot} from './handoff-snapshot.js';
import {HandoffError,HandoffOperationJournal,handoffHash,handoffUuid,handoffDigest,type HandoffOperation,type HandoffCodeReady} from './handoff-operation-journal.js';
export {HandoffError,HandoffOperationJournal,handoffHash} from './handoff-operation-journal.js';
export type {HandoffOperation,HandoffCodeReady} from './handoff-operation-journal.js';
export interface HandoffInput {
 workspaceId:string;sourceSessionId:string;
 target:{driver:AgentDriver;profile:AgentProfileRef;model?:{provider:string;id:string}};
 selection:{throughItemId:string;fromItemId?:string;excludeItemIds?:string[]};
 code:{kind:'current'}|{kind:'checkpoint';checkpointId:string};imagePolicy:'references-only';
}
export interface HandoffCode {
 version:string;repositoryCommonDir:string;sourceExecutionPath:string;headOid:string;checkpointId?:string;coverage:unknown;exclusions:string[];
}
export interface HandoffSnapshot {
 version:1;snapshotId:string;sha256:string;createdAt:number;workspaceId:string;sourceSessionId:string;sourceOriginAgentId:string;
 target:HandoffInput['target'];history:HandoffHistorySnapshot;code:HandoffCode & {choice:HandoffInput['code']};
}
export interface HandoffBlocker {code:string;message:string}
export interface HandoffEligibility {eligible:boolean;blockers:HandoffBlocker[];fingerprint:string;originAgentId?:string}
export interface HandoffPreview {
 version:1;previewId:string;createdAt:number;expiresAt:number;input:HandoffInput;snapshotHash:string;snapshot:HandoffSnapshot;
 sourceFingerprint:string;eligible:boolean;blockers:HandoffBlocker[];omissionsHash:string;
}
export interface HandoffPort {
 /** Hold existing execution mutation admission for the whole operation. No native/model launch. */
 lease(workspaceId:string):Promise<()=>void>|(()=>void);
 inspect(workspaceId:string,sourceSessionId:string,target?:HandoffInput['target']):Promise<HandoffEligibility>;
 readHistory(input:HandoffInput):Promise<HandoffHistorySnapshot>;
 verifyHistory(history:HandoffHistorySnapshot,input:HandoffInput):Promise<void>;
 codePreview(input:HandoffInput):Promise<HandoffCode>;
 checkpoints(workspaceId:string,sourceSessionId:string):Promise<CodeCheckpoints>|CodeCheckpoints;
 /** On recovery only accept a verified completed materialization; never overwrite a partial/edited tree. */
 ensureCode(operation:HandoffOperation,snapshot:HandoffSnapshot,checkpoint:ExportedCodeCheckpoint,options:{recover:boolean}):Promise<HandoffCodeReady>;
 /** Must recheck target profile enabled and exact driver/revision, origin existence, and fixed child ownership.
  * Publish an unbound known-origin passive record with no source native IDs or completion receipts. */
 publishTarget(operation:HandoffOperation,snapshot:HandoffSnapshot,code:HandoffCodeReady):Promise<void>;
 verifyTarget(operation:HandoffOperation,snapshot:HandoffSnapshot,code:HandoffCodeReady):Promise<void>;
}
export interface HandoffApplyInput {workspaceId:string;previewId:string;snapshotHash:string;requestId:string;acknowledgedOmissionsHash:string}
const identity=(value:unknown):value is string=>typeof value==='string'&&value.length>0&&value.length<=256&&!/[\x00-\x1f\x7f]/.test(value);
function fail(code:string,message:string):never{throw new HandoffError(code,message);}
function strict(value:unknown,keys:string[]):void{if(!value||typeof value!=='object'||Array.isArray(value)||Object.keys(value).some(key=>!keys.includes(key)))fail('INVALID_ARGUMENT','Handoff input has unsupported fields');}
function inputValid(input:HandoffInput):void {
 strict(input,['workspaceId','sourceSessionId','target','selection','code','imagePolicy']);strict(input.target,['driver','profile','model']);strict(input.target.profile,['id','revision']);strict(input.selection,['throughItemId','fromItemId','excludeItemIds']);strict(input.code,['kind','checkpointId']);
 if(!identity(input.workspaceId)||!identity(input.sourceSessionId)||!['pi','codex','pi-rpc','claude','acp'].includes(input.target.driver)||!identity(input.target.profile.id)||!Number.isSafeInteger(input.target.profile.revision)||input.target.profile.revision<1||!identity(input.selection.throughItemId)||input.selection.fromItemId!==undefined&&!identity(input.selection.fromItemId)||!['current','checkpoint'].includes(input.code.kind)||input.code.kind==='checkpoint'&&!handoffUuid(input.code.checkpointId)||input.code.kind==='current'&&'checkpointId' in input.code||input.imagePolicy!=='references-only')fail('INVALID_ARGUMENT','Supply explicit source, target profile, history selection and code choice');
 if(input.selection.excludeItemIds!==undefined&&(!Array.isArray(input.selection.excludeItemIds)||input.selection.excludeItemIds.length>256||input.selection.excludeItemIds.some(value=>!identity(value))||new Set(input.selection.excludeItemIds).size!==input.selection.excludeItemIds.length))fail('INVALID_ARGUMENT','Excluded historical items must be explicit unique item identities');
 if(input.target.model){strict(input.target.model,['provider','id']);if(!identity(input.target.model.provider)||!identity(input.target.model.id))fail('INVALID_ARGUMENT','Target model must be explicit');}
}
export const handoffSnapshotHash=(snapshot:HandoffSnapshot):string=>handoffHash({...snapshot,sha256:undefined});
/** Durable offline handoff orchestration. The port owns Git and existing chat/identity contracts. */
export class SessionHandoffs {
 readonly journal:HandoffOperationJournal;
 private closing=false;private readonly actions=new Set<Promise<unknown>>();private readonly active=new Map<string,Promise<HandoffOperation>>();
 constructor(dataDir:string,private readonly port:HandoffPort,private readonly options:{now?:()=>number;previewTtlMs?:number}={}){this.journal=new HandoffOperationJournal(dataDir);}
 private now(){return this.options.now?.()??Date.now();}
 private action<T>(run:()=>Promise<T>):Promise<T>{if(this.closing)return Promise.reject(new HandoffError('CLOSED','Handoff service is closed'));const promise=run();this.actions.add(promise);void promise.finally(()=>this.actions.delete(promise)).catch(()=>{});return promise;}
 private async leased<T>(workspaceId:string,run:()=>Promise<T>):Promise<T>{const release=await this.port.lease(workspaceId);try{return await run();}finally{release();}}
 async close():Promise<void>{this.closing=true;await Promise.allSettled([...this.actions]);}
 capabilities(workspaceId:string,sourceSessionId:string):Promise<HandoffEligibility>{return this.action(()=>this.leased(workspaceId,()=>this.port.inspect(workspaceId,sourceSessionId)));}
 preview(raw:HandoffInput):Promise<HandoffPreview>{return this.action(async()=>{inputValid(raw);const input=structuredClone(raw);return this.leased(input.workspaceId,async()=>{
  const before=await this.port.inspect(input.workspaceId,input.sourceSessionId,input.target);
  const history=await this.port.readHistory(input),code=await this.port.codePreview(input);
  await this.port.verifyHistory(history,input);
  const after=await this.port.inspect(input.workspaceId,input.sourceSessionId,input.target),codeAfter=await this.port.codePreview(input);
  if(before.fingerprint!==after.fingerprint||handoffHash(code)!==handoffHash(codeAfter))fail('STALE_SOURCE','Source identity, responsibility or code changed during preview');
  const createdAt=this.now();const snapshot:HandoffSnapshot={version:1,snapshotId:randomUUID(),sha256:'',createdAt,workspaceId:input.workspaceId,sourceSessionId:input.sourceSessionId,sourceOriginAgentId:after.originAgentId??'',target:input.target,history,code:{...code,choice:input.code}};
  snapshot.sha256=handoffSnapshotHash(snapshot);if(Buffer.byteLength(JSON.stringify(snapshot))>1024*1024)fail('SNAPSHOT_LIMIT','Handoff snapshot exceeds 1 MiB; select fewer historical items');
  const blockers=[...after.blockers];if(history.requiresExplicitExclusionItemIds.length)blockers.push({code:'EXPLICIT_EXCLUSION_REQUIRED',message:'Some user or assistant items exceed the text limit; select a smaller range or explicitly exclude these items in a new preview'});if(!after.originAgentId&&!blockers.some(value=>value.code==='UNKNOWN_ORIGIN'))blockers.push({code:'UNKNOWN_ORIGIN',message:'Source origin cannot be verified; frozen history remains read only'});
  const preview:HandoffPreview={version:1,previewId:randomUUID(),createdAt,expiresAt:createdAt+(this.options.previewTtlMs??600000),input,snapshotHash:snapshot.sha256,snapshot,sourceFingerprint:after.fingerprint,eligible:after.eligible&&blockers.length===0,blockers,omissionsHash:history.omissionsHash};
  await this.journal.savePreview(preview);return preview;
 });});}
 private async loadPreview(workspaceId:string,id:string):Promise<HandoffPreview>{const preview=await this.journal.preview(id);if(!preview||preview.version!==1||preview.input.workspaceId!==workspaceId||preview.previewId!==id)fail('PREVIEW_NOT_FOUND','Handoff preview does not exist in this workspace');inputValid(preview.input);const snapshot=await this.getSnapshot(workspaceId,preview.snapshot.snapshotId);if(snapshot.sha256!==preview.snapshotHash||handoffHash(snapshot)!==handoffHash(preview.snapshot)||snapshot.sourceSessionId!==preview.input.sourceSessionId||handoffHash(snapshot.target)!==handoffHash(preview.input.target)||preview.omissionsHash!==snapshot.history.omissionsHash)fail('SNAPSHOT_CORRUPT','Handoff preview does not match its frozen snapshot');return preview;}
 async getSnapshot(workspaceId:string,snapshotId:string):Promise<HandoffSnapshot>{const snapshot=await this.journal.snapshot(snapshotId);if(!snapshot||snapshot.version!==1||snapshot.workspaceId!==workspaceId||snapshot.snapshotId!==snapshotId||!handoffDigest(snapshot.sha256)||handoffSnapshotHash(snapshot)!==snapshot.sha256)fail('SNAPSHOT_CORRUPT','Frozen handoff snapshot is missing or changed');return snapshot;}
 apply(raw:HandoffApplyInput):Promise<HandoffOperation>{return this.action(async()=>{strict(raw,['workspaceId','previewId','snapshotHash','requestId','acknowledgedOmissionsHash']);if(!identity(raw.workspaceId)||!handoffUuid(raw.previewId)||!handoffUuid(raw.requestId)||!handoffDigest(raw.snapshotHash)||!handoffDigest(raw.acknowledgedOmissionsHash))fail('INVALID_ARGUMENT','Apply requires the reviewed snapshot, omissions hash and UUIDv4 request');
  const requestHash=handoffHash(raw);const active=this.active.get(raw.requestId);if(active){const result=await active;if(result.requestHash!==requestHash)fail('REQUEST_CONFLICT','Request identity belongs to another handoff');return result;}
  const promise=this.leased(raw.workspaceId,async()=>{
   const existing=await this.journal.get(raw.requestId);if(existing){if(existing.workspaceId!==raw.workspaceId||existing.requestHash!==requestHash)fail('REQUEST_CONFLICT','Request identity belongs to another handoff');return existing;}
   const preview=await this.loadPreview(raw.workspaceId,raw.previewId);
   if(preview.snapshotHash!==raw.snapshotHash||preview.omissionsHash!==raw.acknowledgedOmissionsHash)fail('PREVIEW_MISMATCH','Acknowledge the exact snapshot and omitted historical items shown in preview');
   if(this.now()>preview.expiresAt)fail('PREVIEW_EXPIRED','Create a new preview before applying');if(!preview.eligible)fail('HANDOFF_BLOCKED',preview.blockers.map(value=>value.message).join('; '));
   await this.verifySource(preview,true);
   const {operation,fresh}=await this.journal.claim({requestId:raw.requestId,requestHash,workspaceId:raw.workspaceId,sourceSessionId:preview.input.sourceSessionId,previewId:preview.previewId,snapshotId:preview.snapshot.snapshotId,snapshotHash:preview.snapshotHash,originAgentId:preview.snapshot.sourceOriginAgentId,target:preview.input.target});
   if(!fresh)return operation;return this.advance(operation,preview,false);
  });this.active.set(raw.requestId,promise);try{return await promise;}finally{if(this.active.get(raw.requestId)===promise)this.active.delete(raw.requestId);}
 });}
 private async verifySource(preview:HandoffPreview,code:boolean){const state=await this.port.inspect(preview.input.workspaceId,preview.input.sourceSessionId,preview.input.target);if(!state.eligible)fail(state.blockers[0]?.code??'HANDOFF_BLOCKED',state.blockers.map(value=>value.message).join('; '));if(state.fingerprint!==preview.sourceFingerprint||state.originAgentId!==preview.snapshot.sourceOriginAgentId)fail('STALE_SOURCE','Source identity or responsibility changed since preview');await this.port.verifyHistory(preview.snapshot.history,preview.input);if(code&&handoffHash(await this.port.codePreview(preview.input))!==handoffHash({...preview.snapshot.code,choice:undefined}))fail('STALE_SOURCE','Source code changed since preview');}
 private async advance(initial:HandoffOperation,preview:HandoffPreview,recover:boolean):Promise<HandoffOperation>{let op=initial;try{
  if(op.state==='ready'){await this.port.verifyTarget(op,preview.snapshot,op.code!);return op;}
  if(op.state==='blocked')op={...op,state:op.blockedFrom??'prepared'};
  const checkpoints=await this.port.checkpoints(op.workspaceId,op.sourceSessionId);
  if(!op.checkpointId){let checkpointId:string;if(preview.input.code.kind==='checkpoint')checkpointId=preview.input.code.checkpointId;else{const previous=await checkpoints.get(op.operationId);if(recover&&!previous)fail('RECOVERY_REQUIRED','Code capture did not complete; this handoff will not create another tree');const capture=previous??await checkpoints.capture({requestId:op.operationId,expectedVersion:preview.snapshot.code.version});if(capture.state!=='ready')fail('CHECKPOINT_INCOMPLETE','Code capture is incomplete and was not recaptured');checkpointId=capture.checkpointId;}op=await this.journal.save({...op,checkpointId});}
  const checkpoint=await checkpoints.export(op.checkpointId!);
  if(checkpoint.manifest.workspaceId!==op.workspaceId||checkpoint.manifest.sessionId!==op.sourceSessionId||checkpoint.manifest.preview.version!==preview.snapshot.code.version)fail('CHECKPOINT_MISMATCH','Checkpoint does not belong to this frozen source code');
  const code=await this.port.ensureCode(op,preview.snapshot,checkpoint,{recover:recover||op.state!=='prepared'});
  if(op.code&&handoffHash(op.code)!==handoffHash(code))fail('WORKTREE_MISMATCH','Existing handoff worktree ownership changed');
  if(op.state==='prepared')op=await this.journal.save({...op,code,state:'codeReady'});
  await this.verifySource(preview,false);
  if(op.state==='codeReady'){await this.port.publishTarget(op,preview.snapshot,code);op=await this.journal.save({...op,state:'targetPublished'});}
  await this.port.verifyTarget(op,preview.snapshot,code);
  const {error:_error,blockedFrom:_blocked,...rest}=op;return await this.journal.save({...rest,state:'ready'});
 }catch(error){const known=error instanceof HandoffError?error:new HandoffError(error&&typeof error==='object'&&'code' in error&&typeof error.code==='string'?error.code:'HANDOFF_FAILED',error instanceof Error?error.message:'Handoff could not complete',{cause:error});try{await this.journal.save({...op,state:'blocked',blockedFrom:op.state==='blocked'?(op.blockedFrom??'prepared'):op.state,error:{code:known.code,message:known.message}});}catch{throw new HandoffError('JOURNAL_WRITE_FAILED','Handoff state could not be saved; existing snapshot, child and code evidence were preserved',{cause:known});}throw known;}}
 get(workspaceId:string,operationId:string):Promise<HandoffOperation>{return this.action(()=>this.journal.byId(workspaceId,operationId));}
 list(workspaceId:string):Promise<{operations:HandoffOperation[]}>{return this.action(async()=>({operations:await this.journal.list(workspaceId)}));}
 recover(workspaceId:string,operationId:string):Promise<HandoffOperation>{return this.action(()=>this.leased(workspaceId,async()=>{const op=await this.journal.byId(workspaceId,operationId);const preview=await this.loadPreview(workspaceId,op.previewId);if(op.snapshotHash!==preview.snapshotHash||op.sourceSessionId!==preview.input.sourceSessionId||op.originAgentId!==preview.snapshot.sourceOriginAgentId||handoffHash(op.target)!==handoffHash(preview.input.target))fail('OPERATION_MISMATCH','Handoff operation has conflicting fixed ownership');await this.verifySource(preview,op.state==='prepared'||op.blockedFrom==='prepared');return this.advance(op,preview,true);}));}
}

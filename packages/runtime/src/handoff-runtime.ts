import {existsSync} from 'node:fs';
import {join} from 'node:path';
import {realpath} from 'node:fs/promises';
import {CodeCheckpoints} from './code-checkpoints.js';
import {materializeCheckpoint,verifyMaterializedCheckpoint} from './checkpoint-materialize.js';
import {createHandoffWorktree,publishHandoffWorktree,readWorktreeIntent,validateManagedWorktree,type WorktreeCreation} from './git-worktrees.js';
import {SessionHandoffs,HandoffError,handoffHash,type HandoffInput,type HandoffSnapshot,type HandoffOperation,type HandoffCodeReady} from './session-handoffs.js';
import {readHandoffHistory,verifyHandoffHistory,handoffRecordHash} from './handoff-snapshot.js';
import type {ChatIndex,ChatRecord} from './chat-index.js';
import type {AgentProfiles} from './agent-profiles.js';
import type {HiveHost} from './hive-host.js';
import type {HiveAgentRuntime,WorkspaceSummary} from './hive-runtime-contract.js';
import type {InitializationPlan} from './project-worktree-initialization.js';

interface Context {
 dataDir:string;hive:HiveHost;runtime:HiveAgentRuntime;profiles:AgentProfiles;index:ChatIndex;
 lease(workspaceId:string):()=>void;
 executionPath(record:ChatRecord,workspace:WorkspaceSummary):Promise<string>;
 busy(ignoredSessionId?:string):string|undefined;
 hasAnyShell():Promise<boolean>;
 setup(workspaceId:string):Promise<InitializationPlan|undefined>;
}
/** Existing Hive identity and Git contracts remain authoritative; this glue never launches an agent. */
export class HandoffRuntime {
 readonly service:SessionHandoffs;
 constructor(private readonly context:Context){
  const c=context;
  this.service=new SessionHandoffs(c.dataDir,{
   lease:c.lease,
   inspect:(w,s,t)=>this.inspect(w,s,t),
   readHistory:async input=>{
    const {record,workspace}=await this.source(input.workspaceId,input.sourceSessionId);
    const history=await readHandoffHistory(record,{workspacePath:workspace.path,codexDirectory:join(c.dataDir,'chats','codex'),claudeDirectory:join(c.dataDir,'chats','claude'),profiles:c.profiles,getRecord:id=>c.index.peek(id),selection:input.selection,...(input.selection.excludeItemIds&&{excludeItemIds:input.selection.excludeItemIds})});
    const target=join(await realpath(c.dataDir),'worktrees','00000000-0000-4000-8000-000000000000','tree');
    if(Buffer.byteLength(this.envelope('00000000-0000-4000-8000-000000000000','0'.repeat(64),await realpath(await c.executionPath(record,workspace)),target,history.renderedText))>48*1024)
      throw new HandoffError('SNAPSHOT_LIMIT','Historical context and source paths exceed 48 KiB; select fewer items');
    return history;
   },
   verifyHistory:async(history,input)=>verifyHandoffHistory(history,c.index.get(input.sourceSessionId),id=>c.index.peek(id)),
   codePreview:async input=>{
    const checkpoints=await this.checkpoints(input.workspaceId,input.sourceSessionId);
    const scan=input.code.kind==='checkpoint'?(await checkpoints.export(input.code.checkpointId)).manifest.preview:await checkpoints.preview();
    if(!scan.canCapture||scan.workspace.projectPrefix||!scan.workspace.headOid)throw new HandoffError('CODE_UNSUPPORTED','Handoff requires a supported repository-root code snapshot');
    return {version:scan.version,repositoryCommonDir:scan.workspace.commonDir,sourceExecutionPath:scan.workspace.projectRoot,headOid:scan.workspace.headOid,coverage:scan.coverage,exclusions:scan.exclusions,...(input.code.kind==='checkpoint'&&{checkpointId:input.code.checkpointId})};
   },
   checkpoints:(w,s)=>this.checkpoints(w,s),
   ensureCode:async(op,snapshot,checkpoint,options)=>{
    let owned;
    if(existsSync(join(c.dataDir,'worktrees',op.childSessionId,'intent.json'))){
     const intent=await readWorktreeIntent(c.dataDir,op.childSessionId);
     if(intent.creation?.handoffOperationId!==op.operationId)throw new HandoffError('WORKTREE_MISMATCH','Another operation owns this worktree');
     owned=intent.record;
    }else{
     if(options.recover)throw new HandoffError('RECOVERY_REQUIRED','No complete owned tree exists; recovery does not create another one');
     const {workspace}=await this.source(op.workspaceId,op.sourceSessionId),setup=await c.setup(op.workspaceId);
     const creation:WorktreeCreation={version:setup?2:1,...(setup&&{setup}),workspaceId:op.workspaceId,backend:op.target.driver,profile:op.target.profile,identityState:'known',originAgentId:op.originAgentId,handoffOperationId:op.operationId,...(op.target.model&&{model:{...op.target.model,name:op.target.model.id}})};
     owned=await createHandoffWorktree({projectRoot:workspace.path,dataRoot:c.dataDir,chatId:op.childSessionId,creation,operationId:op.operationId,baseOid:snapshot.code.headOid});
     await materializeCheckpoint({dataRoot:c.dataDir,record:owned,operationId:op.operationId,checkpoint,operationKind:'handoff'});
    }
    await verifyMaterializedCheckpoint({dataRoot:c.dataDir,record:owned,operationId:op.operationId,checkpoint,operationKind:'handoff'});
    return {executionPath:owned.executionPath,branch:owned.branch,proof:handoffHash(owned)};
   },
   publishTarget:async(op,snapshot,code)=>{
    await c.profiles.resolveForExecution(op.target.profile,op.target.driver);
    const eligibility=await this.inspect(op.workspaceId,op.sourceSessionId,op.target);
    if(!eligibility.eligible)throw new HandoffError('HANDOFF_BLOCKED',eligibility.blockers.map(b=>b.message).join('; '));
    const {record:owned}=await readWorktreeIntent(c.dataDir,op.childSessionId);
    if(handoffHash(owned)!==code.proof)throw new HandoffError('WORKTREE_MISMATCH','Owned worktree changed before publication');
    await publishHandoffWorktree(c.dataDir,owned,op.operationId);
    const record:ChatRecord={id:op.childSessionId,workspaceId:op.workspaceId,backend:op.target.driver,profile:{...op.target.profile},originAgentId:op.originAgentId,identityState:'known',title:c.index.get(op.sourceSessionId).title+' · handoff',sessionFile:null,updatedAt:Date.now(),execution:{kind:'worktree',path:code.executionPath,branch:code.branch},handoff:{operationId:op.operationId,snapshotId:snapshot.snapshotId,snapshotHash:snapshot.sha256,sourceSessionId:op.sourceSessionId},...(op.target.model&&{model:{...op.target.model,name:op.target.model.id}})};
    const previous=c.index.peek(record.id);
    if(previous){this.fixedTarget(previous,op,snapshot,code);return;}
    c.index.save(record);
   },
   verifyTarget:async(op,snapshot,code)=>{
    this.fixedTarget(c.index.get(op.childSessionId),op,snapshot,code);
    const intent=await readWorktreeIntent(c.dataDir,op.childSessionId);
    if(intent.creation?.handoffOperationId!==op.operationId||handoffHash(intent.record)!==code.proof)throw new HandoffError('WORKTREE_MISMATCH','Handoff worktree ownership changed');
    await validateManagedWorktree(c.dataDir,intent.record);
    if(op.state!=='ready'){
     const checkpoint=await (await this.checkpoints(op.workspaceId,op.sourceSessionId)).export(op.checkpointId!);
     await verifyMaterializedCheckpoint({dataRoot:c.dataDir,record:intent.record,operationId:op.operationId,checkpoint,operationKind:'handoff'});
    }
   },
  });
 }
 private fixedTarget(record:ChatRecord,op:HandoffOperation,snapshot:HandoffSnapshot,code:HandoffCodeReady){
  if(record.workspaceId!==op.workspaceId||record.originAgentId!==op.originAgentId||record.backend!==op.target.driver||record.profile?.id!==op.target.profile.id||record.profile.revision!==op.target.profile.revision||record.execution?.path!==code.executionPath||record.execution.branch!==code.branch||record.handoff?.operationId!==op.operationId||record.handoff.snapshotId!==snapshot.snapshotId||record.handoff.snapshotHash!==snapshot.sha256||record.handoff.sourceSessionId!==op.sourceSessionId)
   throw new HandoffError('TARGET_IDENTITY_CONFLICT','The target does not match its frozen handoff');
 }
 private async source(workspaceId:string,sourceSessionId:string){
  const c=this.context,record=c.index.get(sourceSessionId);
  if(record.workspaceId!==workspaceId)throw new HandoffError('SOURCE_MISMATCH','Source belongs to another workspace');
  const workspace=(await c.hive.request('GET','/api/workspaces') as WorkspaceSummary[]).find(w=>w.id===workspaceId);
  if(!workspace)throw new HandoffError('SOURCE_UNAVAILABLE','Source workspace no longer exists');
  return {record,workspace};
 }
 private async checkpoints(workspaceId:string,sessionId:string){const {record,workspace}=await this.source(workspaceId,sessionId);return new CodeCheckpoints({dataDir:this.context.dataDir,workspaceId,sessionId,executionPath:await this.context.executionPath(record,workspace)});}
 async inspect(workspaceId:string,sourceSessionId:string,target?:HandoffInput['target'],ignoredSessionId?:string){
  const c=this.context,{record}=await this.source(workspaceId,sourceSessionId),blockers:{code:string;message:string}[]=[];
  if(record.identityState!=='known'||!record.originAgentId||record.originAgentId===`${workspaceId}:__workflow__`)blockers.push({code:'UNKNOWN_ORIGIN',message:'Only a known, existing original orchestrator or Worker can continue in another backend'});
  const workspaces=await c.hive.request('GET','/api/workspaces') as WorkspaceSummary[],facts:unknown[]=[];
  for(const workspace of workspaces){
   const team=await c.hive.request('GET',`/api/ui/workspaces/${encodeURIComponent(workspace.id)}/team`) as {id:string;status?:string;pending_task_count:number;open_dispatches?:unknown[]}[];
   if(workspace.id===workspaceId&&record.originAgentId&&record.originAgentId!==`${workspaceId}:orchestrator`&&!team.some(m=>m.id===record.originAgentId))blockers.push({code:'ORIGIN_DELETED',message:'The original Worker no longer exists; history remains read only'});
   const ids=[`${workspace.id}:orchestrator`,`${workspace.id}:__workflow__`,...team.map(m=>m.id)];
   const ignored=ignoredSessionId&&c.index.peek(ignoredSessionId),ignoredAgent=ignored&&ignored.workspaceId===workspace.id&&ignored.agentId;
   if(ids.some(id=>{if(id===ignoredAgent)return false;const run=c.runtime.getActiveRunByAgentId(workspace.id,id);return run&&['starting','running'].includes(run.status);}))blockers.push({code:'ACTIVE_RUN',message:'Stop active agents before a context handoff'});
   const switchState=c.hive.desktopConversationSwitchState(workspace.id);
   if(!switchState.eligible)for(const blocker of switchState.blockers)blockers.push({code:blocker.code,message:`Unsettled Hive responsibility: ${blocker.code} (${blocker.count})`});
   if(team.some(m=>m.id!==ignoredAgent&&m.status==='working'||m.pending_task_count>0||m.open_dispatches?.length))blockers.push({code:'PENDING_RESPONSIBILITY',message:'Finish existing Hive tasks and dispatches before switching conversations'});
   facts.push([workspace.id,switchState.revision,team]);
  }
  const busy=c.busy(ignoredSessionId);if(busy)blockers.push({code:'SOURCE_BUSY',message:busy});
  if(await c.hasAnyShell())blockers.push({code:'ACTIVE_SHELL',message:'End application terminals before a handoff'});
  if(record.profile){try{await c.profiles.getRef(record.profile,record.backend);}catch{blockers.push({code:'SOURCE_PROFILE_UNAVAILABLE',message:'Source fixed profile cannot be resolved; no other profile was substituted'});}}
  else blockers.push({code:'SOURCE_PROFILE_UNAVAILABLE',message:'Source fixed profile is missing'});
  if(target){await c.profiles.resolveForExecution(target.profile,target.driver);if(target.driver==='acp'&&target.model)throw new HandoffError('MODEL_UNSUPPORTED','ACP model selection is not implemented');}
  return {eligible:blockers.length===0,blockers,fingerprint:handoffHash([handoffRecordHash(record),record.agentId??null,facts]),...(record.originAgentId&&{originAgentId:record.originAgentId})};
 }
 async assertReady(record:ChatRecord){
  const h=record.handoff;if(!h)return;
  const op=await this.service.get(record.workspaceId,h.operationId),snapshot=await this.service.getSnapshot(record.workspaceId,h.snapshotId);
  if(op.state!=='ready'||op.childSessionId!==record.id||op.snapshotHash!==h.snapshotHash||snapshot.sha256!==h.snapshotHash||op.sourceSessionId!==h.sourceSessionId||!op.code)throw new HandoffError('HANDOFF_PENDING','This handoff requires verification before use');
  this.fixedTarget(record,op,snapshot,op.code);
  const original=this.context.index.get(h.sourceSessionId);
  if(original.workspaceId!==record.workspaceId||original.originAgentId!==record.originAgentId||original.identityState!=='known')throw new HandoffError('SOURCE_IDENTITY_CONFLICT','The original session identity is no longer known');
 }
 async contextText(record:ChatRecord){
  await this.assertReady(record);const snapshot=await this.service.getSnapshot(record.workspaceId,record.handoff!.snapshotId);
  const text=this.envelope(snapshot.snapshotId,snapshot.sha256,snapshot.code.sourceExecutionPath,record.execution!.path,snapshot.history.renderedText);
  if(Buffer.byteLength(text)>48*1024)throw new HandoffError('SNAPSHOT_LIMIT','Frozen historical context exceeds the reviewed input limit');
  return text;
 }
 private envelope(id:string,hash:string,source:string,target:string,body:string){return `\n\nHandoff snapshot ${id} (${hash}). Source code: ${source}. Current target code: ${target}. These are different execution directories.\n${body}`;}
}

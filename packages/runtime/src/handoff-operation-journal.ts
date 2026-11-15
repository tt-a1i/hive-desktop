import {createHash, randomUUID} from 'node:crypto';
import {chmod, link, lstat, mkdir, open, readdir, realpath, rename, unlink} from 'node:fs/promises';
import {dirname, join, resolve} from 'node:path';
import type {HandoffPreview, HandoffSnapshot} from './session-handoffs.js';

export class HandoffError extends Error {
  constructor(readonly code:string,message:string,options?:ErrorOptions){super(message,options);this.name='HandoffError';}
}
function canonical(value:unknown):unknown {
  if(Array.isArray(value))return value.map(canonical);
  if(value&&typeof value==='object')return Object.fromEntries(Object.keys(value).sort().filter(key=>(value as Record<string,unknown>)[key]!==undefined).map(key=>[key,canonical((value as Record<string,unknown>)[key])]));
  return value;
}
export const handoffHash=(value:unknown):string=>createHash('sha256').update(JSON.stringify(canonical(value))).digest('hex');
export const handoffUuid=(value:unknown):value is string=>typeof value==='string'&&/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(value);
export const handoffDigest=(value:unknown):value is string=>typeof value==='string'&&/^[a-f0-9]{64}$/.test(value);
export type HandoffState='prepared'|'codeReady'|'targetPublished'|'ready'|'blocked';
export interface HandoffCodeReady {executionPath:string;branch:string;proof:string}
export interface HandoffOperation {
  version:1;operationId:string;requestId:string;requestHash:string;workspaceId:string;sourceSessionId:string;childSessionId:string;
  previewId:string;snapshotId:string;snapshotHash:string;originAgentId:string;target:HandoffSnapshot['target'];
  state:HandoffState;blockedFrom?:Exclude<HandoffState,'blocked'>;createdAt:number;updatedAt:number;checkpointId?:string;code?:HandoffCodeReady;
  error?:{code:string;message:string};
}
const states=['prepared','codeReady','targetPublished','ready','blocked'];
const keys=['version','operationId','requestId','requestHash','workspaceId','sourceSessionId','childSessionId','previewId','snapshotId','snapshotHash','originAgentId','target','state','blockedFrom','createdAt','updatedAt','checkpointId','code','error'];
function validate(op:HandoffOperation):void {
  if(!op||Object.keys(op).some(key=>!keys.includes(key))||op.version!==1||![op.operationId,op.requestId,op.childSessionId,op.previewId,op.snapshotId].every(handoffUuid)||![op.requestHash,op.snapshotHash].every(handoffDigest)||![op.workspaceId,op.sourceSessionId,op.originAgentId].every(value=>typeof value==='string'&&!!value)||!states.includes(op.state)||op.blockedFrom!==undefined&&!['prepared','codeReady','targetPublished','ready'].includes(op.blockedFrom)||!Number.isSafeInteger(op.createdAt)||op.createdAt<0||!Number.isSafeInteger(op.updatedAt)||op.updatedAt<0||!op.target||!op.target.profile||typeof op.target.profile.id!=='string'||!Number.isSafeInteger(op.target.profile.revision)||!['pi','codex','pi-rpc','claude','acp'].includes(op.target.driver)||op.checkpointId!==undefined&&!handoffUuid(op.checkpointId)||op.code!==undefined&&(!op.code||typeof op.code!=='object'||typeof op.code.executionPath!=='string'||!op.code.executionPath||typeof op.code.branch!=='string'||!op.code.branch||!handoffDigest(op.code.proof))||op.error!==undefined&&(!op.error||typeof op.error!=='object'||typeof op.error.code!=='string'||typeof op.error.message!=='string')||(['codeReady','targetPublished','ready'].includes(op.state)||op.state==='blocked'&&['codeReady','targetPublished','ready'].includes(op.blockedFrom??''))&&(!op.code||!op.checkpointId))throw new HandoffError('JOURNAL_CORRUPT','Handoff metadata is invalid; existing evidence was preserved');
  const extra=(value:object,allowed:string[])=>Object.keys(value).some(key=>!allowed.includes(key));
  if(extra(op.target,['driver','profile','model'])||extra(op.target.profile,['id','revision'])||op.target.model!==undefined&&(!op.target.model||typeof op.target.model!=='object'||extra(op.target.model,['provider','id'])||typeof op.target.model.provider!=='string'||typeof op.target.model.id!=='string')||op.code!==undefined&&extra(op.code,['executionPath','branch','proof'])||op.error!==undefined&&extra(op.error,['code','message']))throw new HandoffError('JOURNAL_CORRUPT','Handoff metadata contains unsupported fields');
}
const missing=(error:unknown)=>(error as NodeJS.ErrnoException).code==='ENOENT';
/** Private, atomic publications. Exclusive link claims never expose a half-written JSON file. */
export class HandoffOperationJournal {
  private root?:string;
  constructor(private readonly dataDir:string){}
  private async directory():Promise<string>{
    if(!this.root){await mkdir(resolve(this.dataDir),{recursive:true,mode:0o700});const base=await realpath(resolve(this.dataDir));this.root=join(base,'session-handoffs');await mkdir(this.root,{recursive:true,mode:0o700});}
    const info=await lstat(this.root);if(!info.isDirectory()||info.isSymbolicLink()||await realpath(this.root)!==this.root)throw new HandoffError('STORAGE_UNSAFE','Handoff storage was replaced or linked');
    await chmod(this.root,0o700);return this.root;
  }
  private async path(kind:'preview'|'snapshot'|'operation',id:string){if(!handoffUuid(id))throw new HandoffError('INVALID_ARGUMENT','A UUIDv4 identity is required');return join(await this.directory(),kind+'-'+id+'.json');}
  private async sync(path:string){let fd;try{fd=await open(path,'r');await fd.sync();}catch(error){if(process.platform!=='win32'||!['EINVAL','EPERM','EISDIR','EACCES'].includes((error as NodeJS.ErrnoException).code??''))throw error;}finally{await fd?.close();}}
  private async write(path:string,value:unknown,exclusive:boolean){
    const root=await this.directory();if(dirname(path)!==root)throw new HandoffError('STORAGE_UNSAFE','Invalid handoff storage scope');const temporary=join(root,'.'+randomUUID()+'.tmp');
    try{const fd=await open(temporary,'wx',0o600);try{await fd.writeFile(JSON.stringify(value));await fd.sync();}finally{await fd.close();}await this.directory();if(exclusive)await link(temporary,path);else await rename(temporary,path);await this.sync(root);}finally{await unlink(temporary).catch(error=>{if(!missing(error))throw error;});}
  }
  private async read<T>(kind:'preview'|'snapshot'|'operation',id:string):Promise<T|undefined>{const path=await this.path(kind,id);try{const info=await lstat(path);if(!info.isFile()||info.isSymbolicLink()||info.size>2*1024*1024)throw new Error('Invalid private file');const fd=await open(path,'r');try{const opened=await fd.stat();if(opened.ino!==info.ino||opened.dev!==info.dev)throw new Error('File changed');const bytes=await fd.readFile('utf8');await this.directory();return JSON.parse(bytes) as T;}finally{await fd.close();}}catch(error){if(missing(error))return;throw new HandoffError('JOURNAL_UNAVAILABLE','Handoff evidence is unreadable; it was not replaced',{cause:error});}}
  async savePreview(preview:HandoffPreview):Promise<void>{await this.write(await this.path('snapshot',preview.snapshot.snapshotId),preview.snapshot,true);await this.write(await this.path('preview',preview.previewId),preview,true);}
  async preview(id:string):Promise<HandoffPreview|undefined>{return this.read('preview',id);}
  async snapshot(id:string):Promise<HandoffSnapshot|undefined>{return this.read('snapshot',id);}
  async get(requestId:string):Promise<HandoffOperation|undefined>{const op=await this.read<HandoffOperation>('operation',requestId);if(op){validate(op);if(op.requestId!==requestId)throw new HandoffError('JOURNAL_CORRUPT','Handoff request identity does not match its file');}return op;}
  async list(workspaceId:string):Promise<HandoffOperation[]>{const result:HandoffOperation[]=[];for(const name of await readdir(await this.directory())){const match=/^operation-(.+)\.json$/.exec(name);if(!match)continue;const op=await this.get(match[1]!);if(op?.workspaceId===workspaceId)result.push(op);}return result.sort((a,b)=>b.createdAt-a.createdAt);}
  async byId(workspaceId:string,id:string):Promise<HandoffOperation>{const op=(await this.list(workspaceId)).find(value=>value.operationId===id);if(!op)throw new HandoffError('OPERATION_NOT_FOUND','Handoff operation does not exist in this workspace');return op;}
  async claim(input:Omit<HandoffOperation,'version'|'operationId'|'childSessionId'|'state'|'createdAt'|'updatedAt'>):Promise<{operation:HandoffOperation;fresh:boolean}>{
    const existing=await this.get(input.requestId);if(existing){if(existing.requestHash!==input.requestHash||existing.workspaceId!==input.workspaceId)throw new HandoffError('REQUEST_CONFLICT','This request identity already belongs to another handoff');return{operation:existing,fresh:false};}
    const now=Date.now();const op:HandoffOperation={...input,version:1,operationId:randomUUID(),childSessionId:randomUUID(),state:'prepared',createdAt:now,updatedAt:now};validate(op);
    try{await this.write(await this.path('operation',op.requestId),op,true);}catch(error){if((error as NodeJS.ErrnoException).code==='EEXIST')return this.claim(input);throw error;}return{operation:op,fresh:true};
  }
  async save(op:HandoffOperation):Promise<HandoffOperation>{validate(op);const old=await this.get(op.requestId);if(!old||handoffHash({...old,state:undefined,blockedFrom:undefined,updatedAt:undefined,checkpointId:undefined,code:undefined,error:undefined})!==handoffHash({...op,state:undefined,blockedFrom:undefined,updatedAt:undefined,checkpointId:undefined,code:undefined,error:undefined}))throw new HandoffError('REQUEST_CONFLICT','Fixed handoff ownership cannot be changed');if(old.checkpointId!==undefined&&old.checkpointId!==op.checkpointId||old.code!==undefined&&handoffHash(old.code)!==handoffHash(op.code))throw new HandoffError('REQUEST_CONFLICT','Established checkpoint and worktree evidence cannot be replaced');const next={...op,updatedAt:Date.now()};await this.write(await this.path('operation',op.requestId),next,false);return next;}
}

import { randomUUID, createHash } from 'node:crypto';
import { mkdir, open, readFile, readdir, rename, rm } from 'node:fs/promises';
import { join } from 'node:path';

export class HistoryOperationError extends Error { constructor(readonly code:string,message:string,options?:ErrorOptions){super(message,options);this.name='HistoryOperationError';} }
export const historyHash=(value:unknown)=>createHash('sha256').update(JSON.stringify(value)).digest('hex');
export type HistoryState='prepared'|'worktreeReady'|'nativeRequested'|'nativeKnown'|'metadataReady'|'ready'|'failed'|'uncertain';
export interface HistoryOperation {
 version:1; operationId:string; requestId:string; requestHash:string; workspaceId:string; sourceSessionId:string; childSessionId:string;
 state:HistoryState; createdAt:number; updatedAt:number; preview:unknown; checkpointId?:string; childThreadId?:string; error?:string;
}
const uuid=/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;
/** Private claim is durable before any checkout or non-idempotent native request. */
export class HistoryOperationJournal {
 private readonly directory:string;
 constructor(dataDir:string){this.directory=join(dataDir,'history-operations');}
 private path(requestId:string){if(!uuid.test(requestId))throw new HistoryOperationError('INVALID_REQUEST','A UUIDv4 request identity is required');return join(this.directory,requestId+'.json');}
 async get(requestId:string):Promise<HistoryOperation|undefined>{
  const path=this.path(requestId);try{const value=JSON.parse(await readFile(path,'utf8')) as HistoryOperation;
   if(value.version!==1||value.requestId!==requestId||!uuid.test(value.childSessionId)||!uuid.test(value.operationId)||!['prepared','worktreeReady','nativeRequested','nativeKnown','metadataReady','ready','failed','uncertain'].includes(value.state))throw new Error('Invalid journal');return value;
  }catch(error){if((error as NodeJS.ErrnoException).code==='ENOENT')return;throw new HistoryOperationError('JOURNAL_UNAVAILABLE','History operation journal is unreadable; no native operation was retried',{cause:error});}
 }
 async list(workspaceId:string):Promise<HistoryOperation[]>{await mkdir(this.directory,{recursive:true,mode:0o700});const result:HistoryOperation[]=[];for(const name of await readdir(this.directory)){if(!name.endsWith('.json'))continue;const op=await this.get(name.slice(0,-5));if(op?.workspaceId===workspaceId)result.push(op);}return result.sort((a,b)=>b.createdAt-a.createdAt);}
 async byId(workspaceId:string,operationId:string){const op=(await this.list(workspaceId)).find(x=>x.operationId===operationId);if(!op)throw new HistoryOperationError('OPERATION_NOT_FOUND','History operation was not found in this workspace');return op;}
 private async sync(){if(process.platform!=='win32'){const fd=await open(this.directory,'r');try{await fd.sync();}finally{await fd.close();}}}
 async claim(input:Omit<HistoryOperation,'version'|'operationId'|'childSessionId'|'state'|'createdAt'|'updatedAt'>):Promise<{operation:HistoryOperation;fresh:boolean}>{
  const existing=await this.get(input.requestId);if(existing){if(existing.requestHash!==input.requestHash||existing.workspaceId!==input.workspaceId||existing.sourceSessionId!==input.sourceSessionId)throw new HistoryOperationError('REQUEST_CONFLICT','This request identity belongs to a different history operation');return {operation:existing,fresh:false};}
  await mkdir(this.directory,{recursive:true,mode:0o700});const now=Date.now();const op:HistoryOperation={...input,version:1,operationId:randomUUID(),childSessionId:randomUUID(),state:'prepared',createdAt:now,updatedAt:now};
  let fd;try{fd=await open(this.path(input.requestId),'wx',0o600);}catch(error){if((error as NodeJS.ErrnoException).code==='EEXIST')return this.claim(input);throw error;}
  try{await fd.writeFile(JSON.stringify(op));await fd.sync();}finally{await fd.close();}await this.sync();return {operation:op,fresh:true};
 }
 async save(op:HistoryOperation):Promise<HistoryOperation>{const next={...op,updatedAt:Date.now()};const tmp=join(this.directory,'.'+randomUUID()+'.tmp');try{const fd=await open(tmp,'wx',0o600);try{await fd.writeFile(JSON.stringify(next));await fd.sync();}finally{await fd.close();}await rename(tmp,this.path(op.requestId));await this.sync();return next;}finally{await rm(tmp,{force:true});}}
}

import {createHash,randomUUID} from 'node:crypto';
import {mkdirSync,readFileSync,renameSync,rmSync,writeFileSync} from 'node:fs';
import {join} from 'node:path';
import {isDeepStrictEqual} from 'node:util';
import type {AgentProfileRef} from './agent-profiles.js';
import {isDeliveryPresentation,type DeliveryPresentation} from './delivery-presentation.js';

export interface SteerIdentity {
 sessionId:string; workspaceId:string; nativeSessionId:string; originAgentId:string; profile:AgentProfileRef;
}
export interface SteerDisposition extends SteerIdentity {
 version:1; deliveryId:string; disposition:'discarded-local-wait'; nativeOutcome:'unknown';
 presentation:DeliveryPresentation; queueVersion:string; decidedAt:string;
}

/** Local user decisions only; never edits native receipts or Hive task state. */
export class SteerDispositions {
 private readonly directory:string;
 constructor(dataDir:string){this.directory=join(dataDir,'chats','steer-dispositions');mkdirSync(this.directory,{recursive:true,mode:0o700});}
 private path(sessionId:string){return join(this.directory,createHash('sha256').update(sessionId).digest('hex')+'.json');}
 list(sessionId:string):SteerDisposition[]{
  let rows:unknown;
  try{rows=JSON.parse(readFileSync(this.path(sessionId),'utf8'));}catch(error){if((error as NodeJS.ErrnoException).code==='ENOENT')return [];throw error;}
  if(!Array.isArray(rows)||!rows.every(row=>row&&row.version===1&&row.sessionId===sessionId
    && typeof row.workspaceId==='string'&&typeof row.nativeSessionId==='string'&&typeof row.originAgentId==='string'
    &&row.profile&&typeof row.profile.id==='string'&&Number.isSafeInteger(row.profile.revision)&&row.profile.revision>0
    &&typeof row.deliveryId==='string'&&row.disposition==='discarded-local-wait'&&row.nativeOutcome==='unknown'
    &&typeof row.queueVersion==='string'&&/^[a-f0-9]{64}$/.test(row.queueVersion)
    &&typeof row.decidedAt==='string'&&isDeliveryPresentation(row.presentation)&&row.presentation.origin==='user')
    || new Set(rows.map(row=>row.deliveryId)).size!==rows.length)throw Error('Invalid local appended-input disposition receipts');
  return rows;
 }
 matches(receipt:SteerDisposition,identity:SteerIdentity):boolean{
  const {sessionId,workspaceId,nativeSessionId,originAgentId,profile}=receipt;
  return isDeepStrictEqual({sessionId,workspaceId,nativeSessionId,originAgentId,profile},identity);
 }
 record(identity:SteerIdentity,deliveryId:string,presentation:DeliveryPresentation,queueVersion:string):SteerDisposition{
  const rows=this.list(identity.sessionId),existing=rows.find(row=>row.deliveryId===deliveryId);
  if(existing){if(!this.matches(existing,identity)||existing.queueVersion!==queueVersion||!isDeepStrictEqual(existing.presentation,presentation))throw Error('The original appended-input disposition changed');return existing;}
  const receipt:SteerDisposition={...identity,version:1,deliveryId,presentation:structuredClone(presentation),queueVersion,
   disposition:'discarded-local-wait',nativeOutcome:'unknown',decidedAt:new Date().toISOString()};
  const target=this.path(identity.sessionId),temporary=target+'.'+randomUUID()+'.tmp';
  try{writeFileSync(temporary,JSON.stringify([...rows,receipt]),{mode:0o600,encoding:'utf8',flush:true});renameSync(temporary,target);}
  finally{rmSync(temporary,{force:true});}
  return receipt;
 }
}

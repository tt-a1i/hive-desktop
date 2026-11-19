import { createHash } from 'node:crypto';
import { readFile, stat, readdir } from 'node:fs/promises';
import { join } from 'node:path';
import { homedir } from 'node:os';
import type { SessionEntry } from '@earendil-works/pi-coding-agent';
import type { ChatRecord } from './chat-index.js';
import type { AgentProfiles, AgentProfileRef, AgentDriver } from './agent-profiles.js';
import { externalPiBranch, externalPiMessage } from './agents/external-pi-history.js';
import { piUserInputSources } from './agents/pi-user-input.js';
import { isDeliveryPresentation } from './delivery-presentation.js';
import { codexMetadataFile, readCodexMetadata } from './agents/codex-metadata.js';
import { normalizeCodexItem, codexToolInput, type CodexItem } from './agents/codex-history.js';
import { readSessionMessages } from './agents/claude.js';
import { ClaudeProjection } from './agents/claude-projection.js';
import { claudeMetadataPath, readClaudeMetadata } from './agents/claude-metadata.js';
import { AcpHistory } from './agents/acp-history.js';
import { HANDOFF_RENDER_LIMIT, handoffOmissionsHash, renderHistoricalContext } from './handoff-render.js';

export interface HistoricalSourceRef { sourceSessionId:string; sourceDriver:AgentDriver; sourceProfile:AgentProfileRef; nativeSessionId?:string; nativeItemId:string; nativeTurnId?:string; executionPath:string; contentSha256:string }
export interface HandoffOmission { itemId?:string; reason:string; bytes?:number; sha256?:string }
export interface HistoricalFact {
  id:string; kind:'user-text'|'assistant-text'|'tool-input'|'tool-output'|'historical-report'; source:HistoricalSourceRef; text:string;
  tool?:{name:string; nativeCallId?:string; status:'recorded-success'|'recorded-error'|'interrupted'|'unknown'};
  excerpt?:{originalBytes:number; includedBytes:number; omittedBytes:number};
  attachments?:{id:string; name:string; availability:'not-checked-offline'; policy:'references-only'}[];
}
export interface HandoffHistorySnapshot {
  version:1; facts:HistoricalFact[];
  coverage:{scope:'persisted-active-branch';nativeFreshness:'not-checked-offline';completeness:'complete-local-branch'|'last-synced-projection'|'partial';omitted:HandoffOmission[];selectedItemIds:string[];renderedBytes:number};
  warnings:string[];sourceFiles:{path:string;sha256:string;identity:string}[];sourceRecords:{sessionId:string;sha256:string}[];
  sourceRecordHash:string; requiresExplicitExclusionItemIds:string[]; omissionsHash:string; renderedText:string; sha256:string;
}
export interface HandoffHistoryOptions {
  workspacePath:string;codexDirectory:string;claudeDirectory:string;profiles:Pick<AgentProfiles,'getRef'>;
  getRecord(id:string):ChatRecord|undefined;selection:{throughItemId:string;fromItemId?:string};excludeItemIds?:string[];
}
export class HandoffHistoryError extends Error { constructor(readonly code:string,message:string){super(message);this.name='HandoffHistoryError';} }
const hash=(value:string|Buffer)=>createHash('sha256').update(value).digest('hex');
const object=(v:unknown):v is Record<string,unknown>=>v!==null&&typeof v==='object'&&!Array.isArray(v);
export function handoffRecordHash(record:ChatRecord):string {
  return hash(JSON.stringify({id:record.id,workspaceId:record.workspaceId,backend:record.backend,profile:record.profile,originAgentId:record.originAgentId,identityState:record.identityState,execution:record.execution,sessionFile:record.sessionFile,codexThreadId:record.codexThreadId,piSessionId:record.piSessionId,externalPiSessionId:record.externalPiSessionId,nativeclaudeSessionId:record.nativeclaudeSessionId,acpSessionId:record.acpSessionId,lineage:record.lineage,...('handoff' in record?{handoff:record.handoff}:{})}));
}
export function handoffHistoryHash(snapshot:HandoffHistorySnapshot):string { return hash(JSON.stringify({...snapshot,sha256:''})); }
async function frozenFile(path:string){
  try {const a=await stat(path,{bigint:true});if(!a.isFile()||a.size>64n*1024n*1024n)throw new HandoffHistoryError('SOURCE_FILE_LIMIT','Persisted history file exceeds the 64 MiB offline read limit');const bytes=await readFile(path);const b=await stat(path,{bigint:true});
    const identity=(s:typeof a)=>JSON.stringify([String(s.dev),String(s.ino),String(s.size),String(s.mtimeNs),String(s.ctimeNs)]);
    if(identity(a)!==identity(b))throw new HandoffHistoryError('STALE_SOURCE','History file changed while reading');
    return {path,sha256:hash(bytes),identity:identity(b),text:bytes.toString('utf8')};
  }catch(e){if((e as NodeJS.ErrnoException).code==='ENOENT')return {path,sha256:hash('missing'),identity:'missing',text:undefined};throw e;}
}
export async function verifyHandoffHistory(snapshot:HandoffHistorySnapshot,record:ChatRecord,getRecord?:(id:string)=>ChatRecord|undefined):Promise<void>{
  if(snapshot.sha256&&snapshot.sha256!==handoffHistoryHash(snapshot))throw new HandoffHistoryError('SNAPSHOT_CORRUPT','Frozen history manifest hash mismatch');
  if(handoffRecordHash(getRecord?.(record.id)??record)!==snapshot.sourceRecordHash)throw new HandoffHistoryError('STALE_SOURCE','Source session identity changed');
  for(const source of snapshot.sourceRecords){const r=source.sessionId===record.id?record:getRecord?.(source.sessionId);if(!r||handoffRecordHash(r)!==source.sha256)throw new HandoffHistoryError('STALE_SOURCE','Historical source identity changed');}
  for(const saved of snapshot.sourceFiles){const now=await frozenFile(saved.path);if(now.sha256!==saved.sha256||now.identity!==saved.identity)throw new HandoffHistoryError('STALE_SOURCE','Historical source file changed');}
}
type Row={id:string;message:Record<string,unknown>;turnId?:string;sourceId?:string};
const textBlocks=(value:unknown):string=>typeof value==='string'?value:Array.isArray(value)?value.filter(object).filter(b=>['text','input_text','output_text'].includes(String(b.type))&&typeof b.text==='string').map(b=>b.text).join('\n'):'';
function excerpt(text:string,limit:number):string {let out='';let size=0;for(const c of text){size+=Buffer.byteLength(c);if(size>limit)break;out+=c;}return out;}
// Only displayed argument fields are traversed. Skip credential/environment fields even inside arguments.
function displayedInput(value:unknown):string {
  let nodes=0;
  const visit=(v:unknown,depth:number):unknown=>{
    if(++nodes>1000||depth>8)throw new HandoffHistoryError('INPUT_TOO_COMPLEX','Displayed tool input exceeds safe traversal bounds');
    if(v===null||['string','number','boolean'].includes(typeof v))return v;
    if(Array.isArray(v))return v.map(x=>visit(x,depth+1));
    if(object(v))return Object.fromEntries(Object.entries(v).filter(([k])=>!/(?:authorization|credential|password|secret|token|api.?key|^env$)/i.test(k)).map(([k,x])=>[k,visit(x,depth+1)]));
    return null;
  };
  return value===undefined?'':JSON.stringify(visit(value,0));
}
/** Pure offline reader: profile lookup is getRef, never execution resolution. */
export async function readHandoffHistory(record:ChatRecord,options:HandoffHistoryOptions):Promise<HandoffHistorySnapshot>{
  if(!record.profile)throw new HandoffHistoryError('PROFILE_UNKNOWN','Source has no fixed profile');
  const profile=await options.profiles.getRef(record.profile,record.backend);
  const sourceRecords=new Map<string,ChatRecord>([[record.id,structuredClone(record)]]);
  const files=new Map<string,Awaited<ReturnType<typeof frozenFile>>>();
  const freeze=async(path:string)=>{const frozen=await frozenFile(path);files.set(path,frozen);return frozen.text;};
  const omitted:HandoffOmission[]=[];const warnings:string[]=[];
  let completeness:HandoffHistorySnapshot['coverage']['completeness']='partial';let rows:Row[]=[];
  if(record.backend==='pi'||record.backend==='pi-rpc'){
    if(!record.sessionFile)throw new HandoffHistoryError('HISTORY_MISSING','Pi transcript path unavailable');
    const raw=await freeze(record.sessionFile);if(raw===undefined)throw new HandoffHistoryError('HISTORY_MISSING','Pi transcript unavailable');
    const parsed:unknown[]=raw.split(/\r?\n/).filter(x=>x.trim()).map(x=>JSON.parse(x));const header=parsed.shift();
    const expected=record.backend==='pi'?record.piSessionId:record.externalPiSessionId;
    if(!object(header)||header.type!=='session'||typeof header.id!=='string'||(expected&&header.id!==expected))throw new HandoffHistoryError('HISTORY_IDENTITY','Pi transcript identity mismatch');
    const seen=new Set<string>();for(const e of parsed){if(!object(e)||typeof e.id!=='string'||!e.id||seen.has(e.id)||typeof e.type!=='string'||!(e.parentId===null||typeof e.parentId==='string'))throw new Error('Invalid Pi entry');seen.add(e.id);}
    const entries=parsed as SessionEntry[];const branch=externalPiBranch(entries,entries.at(-1)?.id??null);const sources=piUserInputSources(branch);
    for(const entry of branch){if(entry.type==='compaction')omitted.push({itemId:entry.id,reason:'Earlier native context was compacted; only persisted visible records are included'});
      const message=externalPiMessage(entry,sources.users.get(entry.id),record.backend);if(message)rows.push({id:entry.id,message});
      if(sources.notes.has(entry.id))omitted.push({itemId:entry.id,reason:'Input marker has no corresponding persisted native user record'});
    }completeness=omitted.length?'partial':'complete-local-branch';
  }else if(record.backend==='codex'){
    if(!record.codexThreadId)throw new HandoffHistoryError('HISTORY_MISSING','Codex native identity unavailable');
    const path=codexMetadataFile(options.codexDirectory,record.codexThreadId);await freeze(path);
    const raw=await freeze(`${path}.display.json`);if(!raw)throw new HandoffHistoryError('HISTORY_MISSING','Codex display journal unavailable');
    const data:unknown=JSON.parse(raw);if(!object(data)||data.version!==1||!Array.isArray(data.entries))throw new Error('Invalid Codex display journal');
    const valid=(entries:unknown)=>Array.isArray(entries)&&entries.every(e=>object(e)&&typeof e.turnId==='string'&&!!e.turnId&&object(e.item)&&typeof e.item.id==='string'&&!!e.item.id&&typeof e.item.type==='string'&&e.item.status!=='inProgress'&&e.item.type!=='hiveProjectionNotice')&&new Set(entries.map(e=>(e as {item:{id:string}}).item.id)).size===entries.length;
    const complete=data.completeHistory===true&&data.searchVersion===2&&valid(data.searchEntries);
    const entries=complete?data.searchEntries as unknown[]:data.entries;
    completeness=complete?'last-synced-projection':'partial';
    if(!complete)warnings.push('Codex display journal is partial; no native history was requested');
    const metadata=readCodexMetadata(options.codexDirectory,record.codexThreadId);const inherited=new Map(metadata.inheritedSources?.map(s=>[s.itemId,s.sourceSessionId]));
    if(inherited.size!==(metadata.inheritedSources?.length??0)||record.lineage?.inheritedItemIds.some(id=>!inherited.has(id)))throw new HandoffHistoryError('SOURCE_PROVENANCE_MISSING','Inherited Codex item has no unambiguous source');
    for(const entry of entries){if(!object(entry)||!object(entry.item)||typeof entry.turnId!=='string'||typeof entry.item.id!=='string'||typeof entry.item.type!=='string')throw new Error('Invalid Codex history item');
      const item=entry.item as CodexItem;
      const delivery=[...(metadata.inheritedDeliveries??[]),...metadata.deliveries].find(d=>d.turnId===entry.turnId&&(d.itemId===item.id||(d.itemId===undefined&&item.type==='userMessage')));
      const message=normalizeCodexItem(item,delivery);if(message.role==='toolResult'){
        // Never use normalizeCodexItem's raw-envelope JSON fallback.
        message.content=typeof item.aggregatedOutput==='string'?item.aggregatedOutput:item.type==='dynamicToolCall'&&Array.isArray(item.contentItems)?item.contentItems.filter(object).filter(b=>b.type==='inputText'&&typeof b.text==='string').map(b=>b.text).join('\n'):'';
        message.toolInput=codexToolInput(item);
        if(!message.content)omitted.push({itemId:item.id,reason:'Tool output has no explicit displayed text; raw envelope excluded'});
      }
      rows.push({id:item.id,message,turnId:entry.turnId,...(inherited.has(item.id)?{sourceId:inherited.get(item.id)!}:{})});
    }
  }else if(record.backend==='acp'){
    if(!record.sessionFile||!record.acpSessionId)throw new HandoffHistoryError('HISTORY_MISSING','ACP local transcript unavailable');
    await freeze(record.sessionFile);const transcript=await AcpHistory.load(record.sessionFile);
    if(!transcript||transcript.state.sessionId!==record.acpSessionId)throw new HandoffHistoryError('HISTORY_IDENTITY','ACP transcript identity mismatch');
    rows=transcript.state.history.map((m,i)=>{if(!object(m))throw new Error('Invalid ACP history row');return {id:transcript.state.historyIds[i]!,message:m};});
    for(const state of Object.values(transcript.state.deliveries))if(state==='sent')omitted.push({reason:'ACP has a sent input without reliable completion; remote context is unknown'});
    warnings.push('ACP local transcript only; remote history and context were not loaded');
  }else{
    if(!record.nativeclaudeSessionId)throw new HandoffHistoryError('HISTORY_MISSING','Claude native identity unavailable');
    const id=record.nativeclaudeSessionId;if(!/^[a-zA-Z0-9_-]+$/.test(id))throw new Error('Invalid Claude session ID');
    await freeze(claudeMetadataPath(options.claudeDirectory,id));
    const configDir=profile.launch.configDir??process.env.CLAUDE_CONFIG_DIR??join(homedir(),'.claude');
    const root=join(configDir,'projects');
    // Search only transcript filenames, never credentials or other config files.
    const matches:string[]=[];
    try{for(const dir of await readdir(root,{withFileTypes:true})){if(dir.isDirectory()){const file=join(root,dir.name,`${id}.jsonl`);try{await stat(file);await freeze(file);matches.push(file);}catch(e){if((e as NodeJS.ErrnoException).code!=='ENOENT')throw e;}}}}catch(e){if((e as NodeJS.ErrnoException).code!=='ENOENT')throw e;}
    if(matches.length!==1)throw new HandoffHistoryError('HISTORY_IDENTITY','Claude transcript is missing or ambiguous within the fixed profile');
    const projection=new ClaudeProjection(()=>readClaudeMetadata(options.claudeDirectory,id));
    const env={...process.env,CLAUDE_CONFIG_DIR:configDir};
    if(!profile.launch.configDir)warnings.push('Source profile uses the official default Claude directory; its historical process environment cannot be verified offline');
    for(const m of await readSessionMessages(id,record.execution?.path??options.workspacePath,env))projection.put(m);
    const snapshot=projection.snapshot();rows=snapshot.history.map((m,i)=>({id:snapshot.historyMessageIds[i]!,message:m}));
    warnings.push('Official Claude SDK does not establish transcript completeness or reveal skipped parse errors');
  }
  const start=options.selection.fromItemId?rows.findIndex(r=>r.id===options.selection.fromItemId):0;
  const end=rows.findIndex(r=>r.id===options.selection.throughItemId);
  if(start<0||end<start)throw new HandoffHistoryError('INVALID_SELECTION','Selected native history range is unavailable');
  // Completeness describes the readable source, not the selected range. Tell
  // both the reviewer and the receiving agent exactly which bounds were used.
  // Never include the contents of entries outside the selected range.
  warnings.push(`Selected persisted range: ${JSON.stringify({fromItemId:rows[start]!.id,throughItemId:rows[end]!.id})}`);
  if(start>0)omitted.push({reason:`${start} persisted item(s) before the selected range were not included`});
  const after=rows.length-end-1;
  if(after>0)omitted.push({reason:`${after} persisted item(s) after the selected range were not included`});
  const selected=rows.slice(start,end+1);const facts:HistoricalFact[]=[];const requiresExplicitExclusionItemIds=new Set<string>();const excluded=new Set(options.excludeItemIds??[]);
  if('handoff' in record&&record.handoff){warnings.push('Inherited handoff facts are not flattened: exact prior injection mapping is unavailable');completeness='partial';}
  for(const row of selected){
    if(excluded.has(row.id)){const p=isDeliveryPresentation(row.message.presentation)?row.message.presentation:undefined;const visible=p?p.displayText:textBlocks(row.message.content);omitted.push({itemId:row.id,reason:'Explicitly excluded by selection',bytes:Buffer.byteLength(visible),sha256:hash(visible)});continue;}
    const source=row.sourceId?options.getRecord(row.sourceId):record;
    if(!source||source.workspaceId!==record.workspaceId||!source.profile)throw new HandoffHistoryError('SOURCE_PROVENANCE_MISSING','Historical ancestor identity unavailable; current directory was not substituted');
    if(row.sourceId&&(source.identityState!=='known'||source.originAgentId!==record.originAgentId))throw new HandoffHistoryError('SOURCE_PROVENANCE_MISSING','Historical ancestor belongs to another or unknown member');
    const previouslyFrozen=sourceRecords.get(source.id);
    if(previouslyFrozen&&handoffRecordHash(previouslyFrozen)!==handoffRecordHash(source))throw new HandoffHistoryError('STALE_SOURCE','Historical ancestor identity changed while reading');
    if(!previouslyFrozen){await options.profiles.getRef(source.profile,source.backend);sourceRecords.set(source.id,structuredClone(source));}
    const m=row.message;const presentation=isDeliveryPresentation(m.presentation)?m.presentation:undefined;
    if('handoff' in source&&source.handoff&&m.role==='user'&&!presentation){omitted.push({itemId:row.id,reason:'Prior injected context cannot be separated from native user text without exact presentation mapping'});continue;}
    const attachments= presentation?.attachments?.map(a=>({id:a.id,name:a.name,availability:'not-checked-offline' as const,policy:'references-only' as const}));
    if(attachments?.length)omitted.push({itemId:row.id,reason:'Attachment binary content not transferred; references only'});
    if(Array.isArray(m.content)&&m.content.some(b=>object(b)&&['image','image_url','audio','file'].includes(String(b.type))))omitted.push({itemId:row.id,reason:'Native binary attachment content not transferred'});
    const add=(kind:HistoricalFact['kind'],text:string,tool?:HistoricalFact['tool'])=>{
      if(!text&&!attachments?.length)return;
      const bytes=Buffer.byteLength(text);const toolFact=kind==='tool-input'||kind==='tool-output';
      if(!toolFact&&bytes>32*1024){requiresExplicitExclusionItemIds.add(row.id);omitted.push({itemId:row.id,reason:'Oversized user/assistant item excluded whole; select a smaller range or explicitly exclude',bytes,sha256:hash(text)});return;}
      const included=toolFact?excerpt(text,8*1024):text;const includedBytes=Buffer.byteLength(included);
      if(includedBytes<bytes)omitted.push({itemId:row.id,reason:'Tool text excerpt',bytes:bytes-includedBytes,sha256:hash(text)});
      const nativeSessionId=source.backend==='codex'?source.codexThreadId:source.backend==='pi'?source.piSessionId:source.backend==='pi-rpc'?source.externalPiSessionId:source.backend==='claude'?source.nativeclaudeSessionId:source.acpSessionId;
      facts.push({id:`${source.id}:${row.id}:${kind}:${tool?.nativeCallId??''}`,kind,text:included,source:{sourceSessionId:source.id,sourceDriver:source.backend,sourceProfile:source.profile!,...(nativeSessionId?{nativeSessionId}:{}),nativeItemId:row.id,...(row.turnId?{nativeTurnId:row.turnId}:{}),executionPath:source.execution?.path??options.workspacePath,contentSha256:hash(text)},...(tool?{tool}:{}),...(includedBytes<bytes?{excerpt:{originalBytes:bytes,includedBytes,omittedBytes:bytes-includedBytes}}:{}),...(attachments?.length?{attachments}:{})});
    };
    if(presentation){if(presentation.origin==='user')add('user-text',presentation.displayText);else if(presentation.origin==='report')add('historical-report',presentation.displayText);else omitted.push({itemId:row.id,reason:'Internal delivery/context excluded'});continue;}
    if(m.role==='user')add('user-text',textBlocks(m.content));
    else if(m.role==='assistant'){
      add('assistant-text',textBlocks(m.content));
      if(Array.isArray(m.content))for(const b of m.content)if(object(b)&&b.type==='toolCall'){
        try{add('tool-input',displayedInput(b.arguments),{name:typeof b.name==='string'?b.name:'unknown',...(typeof b.id==='string'?{nativeCallId:b.id}:{}),status:'unknown'});}catch(e){if(!(e instanceof HandoffHistoryError))throw e;omitted.push({itemId:row.id,reason:e.message});}
      }
    }else if(m.role==='toolResult'){
      const tool:NonNullable<HistoricalFact['tool']>={name:typeof m.toolName==='string'?m.toolName:'unknown',...(typeof m.toolCallId==='string'?{nativeCallId:m.toolCallId}:{}),status:m.toolStatus==='interrupted'?'interrupted':m.isError===true?'recorded-error':['completed','success'].includes(String(m.toolStatus))?'recorded-success':'unknown'};
      try{add('tool-input',displayedInput(m.toolInput),tool);}catch(e){if(!(e instanceof HandoffHistoryError))throw e;omitted.push({itemId:row.id,reason:e.message});}add('tool-output',textBlocks(m.content),tool);
    }
  }
  const unique=[...new Map(facts.map(f=>[JSON.stringify([f.source.sourceSessionId,f.source.nativeItemId,f.kind,f.tool?.nativeCallId,f.source.contentSha256]),f])).values()];
  const result:HandoffHistorySnapshot={version:1,facts:unique,coverage:{scope:'persisted-active-branch',nativeFreshness:'not-checked-offline',completeness,omitted,selectedItemIds:selected.map(r=>r.id),renderedBytes:0},warnings,sourceFiles:[...files.values()].map(({path,sha256,identity})=>({path,sha256,identity})),sourceRecords:[...sourceRecords.values()].map(r=>({sessionId:r.id,sha256:handoffRecordHash(r)})),sourceRecordHash:handoffRecordHash(record),requiresExplicitExclusionItemIds:[...requiresExplicitExclusionItemIds],omissionsHash:'',renderedText:'',sha256:''};
  while(true){result.renderedText=renderHistoricalContext(result);if(result.facts.length<=256&&Buffer.byteLength(result.renderedText)<=HANDOFF_RENDER_LIMIT)break;const oldest=result.facts[0];if(!oldest)throw new HandoffHistoryError('SNAPSHOT_TOO_LARGE','Omission metadata exceeds rendered text budget; select fewer items');const removed=result.facts.filter(f=>f.source.sourceSessionId===oldest.source.sourceSessionId&&f.source.nativeItemId===oldest.source.nativeItemId);result.facts=result.facts.filter(f=>!removed.includes(f));for(const fact of removed)omitted.push({itemId:fact.source.nativeItemId,reason:`Oldest whole item excluded to meet bounded context budget (${fact.kind})`,bytes:Buffer.byteLength(fact.text),sha256:fact.source.contentSha256});}
  result.coverage.renderedBytes=Buffer.byteLength(result.renderedText);result.omissionsHash=handoffOmissionsHash(omitted);
  if(Buffer.byteLength(JSON.stringify(result))>1024*1024)throw new HandoffHistoryError('SNAPSHOT_TOO_LARGE','History manifest exceeds 1 MiB');
  await verifyHandoffHistory(result,record,options.getRecord);result.sha256=handoffHistoryHash(result);return result;
}

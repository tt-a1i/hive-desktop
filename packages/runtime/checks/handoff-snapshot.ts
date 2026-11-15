/** Synthetic, temporary transcripts only. No models, vendor CLI, login or user data. */
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, writeFile, readFile, rm, stat, utimes, realpath } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { randomUUID, createHash } from 'node:crypto';
import { readHandoffHistory, verifyHandoffHistory, handoffRecordHash, type HandoffHistoryOptions } from '../src/handoff-snapshot.js';
import { handoffOmissionsHash } from '../src/handoff-render.js';
import { codexMetadataFile } from '../src/agents/codex-metadata.js';
import type { ChatRecord } from '../src/chat-index.js';
import type { AgentDriver } from '../src/agent-profiles.js';
const root=await realpath(await mkdtemp(join(tmpdir(),'hive-handoff-history-')));
try{
  const records=new Map<string,ChatRecord>();const profileDir=join(root,'claude-config');const calls:string[]=[];
  const profiles:HandoffHistoryOptions['profiles']={async getRef(ref,driver){calls.push(ref.id);return {ref,driver:driver!,label:'Synthetic disabled profile',enabled:false,launch:{args:[],configDir:profileDir}};}};
  const options=(id:string):HandoffHistoryOptions=>({workspacePath:root,codexDirectory:root,claudeDirectory:root,profiles,getRecord:id=>records.get(id),selection:{throughItemId:id}});
  const record=(backend:AgentDriver,id=backend):ChatRecord=>{const value:ChatRecord={id,workspaceId:'w',title:'Fixture',backend,profile:{id:'profile-'+backend,revision:1},originAgentId:'worker',identityState:'known',sessionFile:null,updatedAt:1};records.set(id,value);return value;};
  for(const driver of ['pi','pi-rpc'] as const){
    const r=record(driver);r.sessionFile=join(root,driver+'.jsonl');if(driver==='pi')r.piSessionId='native-pi';else r.externalPiSessionId='native-pi';
    const entry=(id:string,parentId:string|null,role:string,content:unknown)=>({id,parentId,type:'message',timestamp:new Date().toISOString(),message:{role,content}});
    await writeFile(r.sessionFile,[{type:'session',id:'native-pi'},entry('u',null,'user','Original user'),entry('abandoned','u','assistant','ABANDONED'),entry('a','u','assistant',[{type:'text',text:'Recorded success claim'},{type:'thinking',thinking:'HIDDEN THINKING'},{type:'toolCall',id:'call1',name:'read',arguments:{path:'old/file',env:{SECRET:'AUTH'},token:'AUTH'}}]),{id:'t',parentId:'a',type:'message',timestamp:new Date().toISOString(),message:{role:'toolResult',toolCallId:'call1',content:[{type:'text',text:'工具'.repeat(6000)}]}}].map(x=>JSON.stringify(x)).join('\n'));
    const result=await readHandoffHistory(r,options('t'));assert.equal(result.coverage.completeness,'complete-local-branch');
    assert(!result.renderedText.includes('ABANDONED'));assert(!result.renderedText.includes('HIDDEN THINKING'));assert(!result.renderedText.includes('AUTH'));
    const tool=result.facts.find(f=>f.kind==='tool-output')!;assert(tool.excerpt);assert(Buffer.byteLength(tool.text)<=8192);assert.equal(tool.excerpt.originalBytes,Buffer.byteLength('工具'.repeat(6000)));
    assert.equal(tool.source.contentSha256,createHash('sha256').update('工具'.repeat(6000)).digest('hex'));
    assert.equal(tool.tool?.status,'unknown');assert.equal(result.omissionsHash,handoffOmissionsHash(result.coverage.omitted));await verifyHandoffHistory(result,r,id=>records.get(id));
    const before=await stat(r.sessionFile);const raw=await readFile(r.sessionFile,'utf8');await writeFile(r.sessionFile,raw.replace('Original user','Modified user'));await utimes(r.sessionFile,before.atime,before.mtime);
    await assert.rejects(()=>verifyHandoffHistory(result,r,id=>records.get(id)),/changed/);
  }
  const ancestor=record('codex','ancestor');ancestor.execution={kind:'worktree',path:join(root,'ancestor-tree'),branch:'old'};ancestor.codexThreadId='ancestor-native';
  const c=record('codex');c.codexThreadId='native-codex';c.execution={kind:'worktree',path:join(root,'child-tree'),branch:'child'};
  const metadataPath=codexMetadataFile(root,c.codexThreadId);
  await writeFile(metadataPath,JSON.stringify({version:1,context:[],deliveries:[],completed:['CREDENTIAL-RECEIPT'],inheritedSources:[{itemId:'u',sourceSessionId:ancestor.id}]}));
  const entries=[{turnId:'turn',item:{id:'u',type:'userMessage',content:[{type:'text',text:'Ancestor original'}]}},{turnId:'turn',item:{id:'a',type:'agentMessage',text:'Full history only'}}];
  await writeFile(metadataPath+'.display.json',JSON.stringify({version:1,entries:[entries[1]],completeHistory:true,searchVersion:2,searchEntries:entries}));
  let result=await readHandoffHistory(c,options('a'));assert.equal(result.coverage.completeness,'last-synced-projection');assert.equal(result.facts.length,2);
  assert.equal(result.facts[0]?.source.executionPath,ancestor.execution.path);assert.equal(result.facts[0]?.source.sourceSessionId,ancestor.id);assert(!JSON.stringify(result).includes('CREDENTIAL-RECEIPT'));
  records.delete(ancestor.id);await assert.rejects(()=>readHandoffHistory(c,options('a')),/ancestor/);records.set(ancestor.id,ancestor);
  // Invalid claimed full history must use the ordinary partial projection.
  await writeFile(metadataPath+'.display.json',JSON.stringify({version:1,entries:[entries[1]],completeHistory:true,searchVersion:2,searchEntries:[...entries,entries[0]]}));
  result=await readHandoffHistory(c,options('a'));assert.equal(result.coverage.completeness,'partial');assert.equal(result.facts.length,1);
  const acp=record('acp');acp.acpSessionId='native-acp';acp.sessionFile=join(root,'acp.json');
  const acpState={version:1,sessionId:acp.acpSessionId,history:[{role:'user',content:'native raw SECRET CONTEXT',presentation:{version:1,origin:'user',displayText:'Real user',attachments:[{id:'image',workspaceId:'w',name:'old.png',mimeType:'image/png',size:1}]}},{role:'assistant',content:'Recorded answer'}],historyIds:['u','a'],contexts:[{text:'HIDDEN CONTEXT',customType:'hive-context'}],deliveries:{'DO-NOT-COPY':'sent'}};
  await writeFile(acp.sessionFile,JSON.stringify(acpState));result=await readHandoffHistory(acp,options('a'));assert.equal(result.coverage.completeness,'partial');assert.equal(result.facts[0]?.text,'Real user');assert.equal(result.facts[0]?.attachments?.[0]?.policy,'references-only');assert(!JSON.stringify(result).includes('DO-NOT-COPY'));assert(!result.renderedText.includes('SECRET CONTEXT'));
  await writeFile(acp.sessionFile,JSON.stringify({...acpState,sessionId:'different'}));await assert.rejects(()=>readHandoffHistory(acp,options('a')),/identity mismatch/);
  // Official SDK reader scoped to a synthetic profile; no CLI/model is executed.
  const claude=record('claude');claude.nativeclaudeSessionId=randomUUID();
  const project=join(profileDir,'projects',root.replace(/[^a-zA-Z0-9]/g,'-'));await mkdir(project,{recursive:true});
  const userUuid=randomUUID();const assistantUuid=randomUUID();
  await writeFile(join(project,claude.nativeclaudeSessionId+'.jsonl'),[{type:'user',uuid:userUuid,parentUuid:null,message:{role:'user',content:'Synthetic Claude input'}},{type:'assistant',uuid:assistantUuid,parentUuid:userUuid,message:{id:'api',model:'fixture',role:'assistant',content:[{type:'text',text:'Synthetic Claude output'}]}}].map(m=>JSON.stringify({...m,sessionId:claude.nativeclaudeSessionId,isSidechain:false,cwd:root,timestamp:new Date().toISOString()})).join('\n')+'\n');
  result=await readHandoffHistory(claude,options(assistantUuid));assert(result.facts.some(f=>f.text==='Synthetic Claude input'));assert(result.facts.some(f=>f.text==='Synthetic Claude output'));assert.equal(result.coverage.completeness,'partial');
  assert(result.sourceFiles.some(f=>f.path.endsWith('.jsonl')));
  // Whole oversized dialogue omission; bounded tools, whole-fact budget eviction and exact hashes.
  await writeFile(acp.sessionFile,JSON.stringify({...acpState,history:[{role:'user',content:'x'.repeat(33000)},...Array.from({length:100},(_,i)=>({role:'assistant',content:`${i}:`+'文'.repeat(1000)}))],historyIds:['large',...Array.from({length:100},(_,i)=>'a'+i)]}));
  result=await readHandoffHistory(acp,options('a99'));assert(result.coverage.omitted.some(o=>o.itemId==='large'&&o.reason.includes('Oversized')));assert(result.coverage.omitted.some(o=>o.reason.includes('Oldest whole item')));assert.deepEqual(result.requiresExplicitExclusionItemIds,['large']);assert(result.coverage.renderedBytes<=49152);assert(result.facts.length<=256);assert(Buffer.byteLength(JSON.stringify(result))<=1048576);
  const explicit=await readHandoffHistory(acp,{...options('a99'),excludeItemIds:['large']});assert.deepEqual(explicit.requiresExplicitExclusionItemIds,[]);assert.notEqual(explicit.omissionsHash,result.omissionsHash);
  const oldHash=handoffRecordHash(acp);assert.equal(handoffRecordHash({...acp,title:'Renamed',updatedAt:999}),oldHash);assert.notEqual(handoffRecordHash({...acp,acpSessionId:'new-native'}),oldHash);
  assert(calls.includes('profile-claude'));console.log('Handoff offline five-driver history, source provenance, exact omissions, UTF-8 budgets and content-stale checks passed');
}finally{await rm(root,{recursive:true,force:true});}

/** Official SDK history reader against synthetic local transcripts only; no vendor CLI or account. */
import assert from 'node:assert/strict';
import {mkdir,mkdtemp,realpath,writeFile} from 'node:fs/promises';
import {randomUUID} from 'node:crypto';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {createDesktopRuntime} from '../src/desktop-runtime.js';
import {readSessionMessages} from '../src/agents/claude.js';
import type {ChatIndex} from '../src/chat-index.js';
const directory=await realpath(await mkdtemp(join(tmpdir(),'hive-profile-claude-history-'))),project=join(directory,'project'),profileDir=join(directory,'profile'),hostDir=join(directory,'host');
const old={PI_CODING_AGENT_DIR:process.env.PI_CODING_AGENT_DIR,PI_OFFLINE:process.env.PI_OFFLINE,CLAUDE_CONFIG_DIR:process.env.CLAUDE_CONFIG_DIR};
await mkdir(project);await mkdir(join(directory,'pi'));process.env.PI_CODING_AGENT_DIR=join(directory,'pi');process.env.PI_OFFLINE='1';process.env.CLAUDE_CONFIG_DIR=hostDir;
let runtime:Awaited<ReturnType<typeof createDesktopRuntime>>|undefined;
const nativeId=randomUUID();
async function transcript(root:string,text:string){const path=join(root,'projects',project.replace(/[^a-zA-Z0-9]/g,'-'));await mkdir(path,{recursive:true});await writeFile(join(path,nativeId+'.jsonl'),JSON.stringify({type:'user',uuid:randomUUID(),sessionId:nativeId,parentUuid:null,isSidechain:false,cwd:project,timestamp:new Date().toISOString(),message:{role:'user',content:text}})+'\n');}
try{
 await transcript(profileDir,'PROFILE_ONLY_SYNTHETIC_HISTORY');await transcript(hostDir,'HOST_HISTORY_MUST_NOT_LEAK');
 assert.equal((await readSessionMessages(nativeId,project,{...process.env,CLAUDE_CONFIG_DIR:profileDir})).length,1);
 const open=()=>createDesktopRuntime(join(directory,'state'),()=>{},{executableEnvironment:{}});runtime=await open();
 const workspace=await runtime.hive.request('POST','/api/workspaces',{path:project,name:'Synthetic offline profile history',autostart_orchestrator:false}) as {id:string};
 const profile=(await runtime.profiles.create({driver:'claude',label:'Synthetic profile history',launch:{command:'/fixture/never-start-claude',args:[],configDir:profileDir}},(await runtime.profiles.list()).storeVersion)).profile;
 const chat=await runtime.chats.createRecord(workspace.id,'claude',{profile:profile.ref});
 // Seed only the synthetic native-history fixture reference, never a fabricated Hive business result.
 let index=(runtime.chats as unknown as {index:ChatIndex}).index;index.save({...index.get(chat.id),nativeclaudeSessionId:nativeId});
 await runtime.profiles.enableDisable(profile.ref.id,false,(await runtime.profiles.list()).storeVersion);
 const first=await runtime.chats.open(chat.id);assert.equal(first.readOnly,true);assert.equal(first.history.length,1);assert.ok(JSON.stringify(first.history).includes('PROFILE_ONLY_SYNTHETIC_HISTORY'));assert.ok(!JSON.stringify(first.history).includes('HOST_HISTORY_MUST_NOT_LEAK'));
 await runtime.close();runtime=await open();const cold=await runtime.chats.open(chat.id);assert.equal(cold.history.length,1);assert.deepEqual(cold.profile,profile.ref);assert.ok(!JSON.stringify(cold.history).includes('HOST_HISTORY_MUST_NOT_LEAK'));
 index=(runtime.chats as unknown as {index:ChatIndex}).index;
 const unknownId=randomUUID();index.save({...index.get(chat.id),id:unknownId,profile:{id:'unknown-profile-fixture',revision:1}});
 const unknown=await runtime.chats.open(unknownId);assert.equal(unknown.readOnly,true);assert.equal(unknown.history.length,0);assert.match(unknown.historyWarning!,/PROFILE_NOT_FOUND/);assert.equal(process.env.CLAUDE_CONFIG_DIR,hostDir);
 const profileSearch=await runtime.chats.search('PROFILE_ONLY_SYNTHETIC_HISTORY',false,0,workspace.id);
 assert.deepEqual(profileSearch.matches.map(hit=>hit.session.id),[chat.id],'search reads the pinned profile, including disabled profiles');
 assert.match(profileSearch.incompleteHistory.find(hit=>hit.sessionId===unknownId)?.message??'',/PROFILE_NOT_FOUND/);
 assert.equal((await runtime.chats.search('HOST_HISTORY_MUST_NOT_LEAK',false,0,workspace.id)).total,0);
 assert.equal(process.env.CLAUDE_CONFIG_DIR,hostDir,'search does not mutate the host environment');
 console.log(JSON.stringify({pass:true,directory,officialReader:true,disabledProfileHistory:true,unknownRefNoHostFallback:true,coldIdentity:true,searchProfileIsolation:true,noVendorProcess:true,noInference:true}));
}finally{await runtime?.close();for(const [key,value]of Object.entries(old)){if(value===undefined)delete process.env[key];else process.env[key]=value;}}

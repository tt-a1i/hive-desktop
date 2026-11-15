/** Actual Git + Hive SQLite + production JSONL + controlled ACP/MCP. No vendor/model/auth. */
import assert from 'node:assert/strict';
import {spawn,execFileSync} from 'node:child_process';
import {once} from 'node:events';
import {mkdtemp,mkdir,writeFile,readFile,realpath,stat} from 'node:fs/promises';
import {join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {tmpdir} from 'node:os';
import {randomUUID} from 'node:crypto';
import {createInterface} from 'node:readline';
import {createServer} from 'node:http';
import {createDesktopRuntime} from '../src/desktop-runtime.js';
import {codexMetadataFile,writeCodexMetadata} from '../src/agents/codex-metadata.js';
import {readWorktreeIntent} from '../src/git-worktrees.js';
import type {ChatIndex,ChatRecord} from '../src/chat-index.js';
import type {HandoffInput} from '../src/session-handoffs.js';
const root=await realpath(await mkdtemp(join(tmpdir(),'hive-handoff-runtime-'))),repo=join(root,'repo'),data=join(root,'state'),acp=join(root,'native-acp');
await mkdir(repo);await mkdir(join(root,'pi'));await writeFile(join(root,'gitconfig'),'');
const envKeys=['PI_CODING_AGENT_DIR','PI_OFFLINE','GIT_CONFIG_GLOBAL','GIT_CONFIG_NOSYSTEM'] as const;
const prior=Object.fromEntries(envKeys.map(key=>[key,process.env[key]]));
Object.assign(process.env,{PI_CODING_AGENT_DIR:join(root,'pi'),PI_OFFLINE:'1',GIT_CONFIG_GLOBAL:join(root,'gitconfig'),GIT_CONFIG_NOSYSTEM:'1'});
const git=(cwd:string,...args:string[])=>execFileSync('git',['-c','user.name=Fixture','-c','user.email=fixture@example.invalid','-c','commit.gpgSign=false',...args],{cwd,encoding:'utf8'}).trim();
const guarded=join(root,'never-agent.cjs');await writeFile(guarded,`#!/usr/bin/env node\nrequire('node:fs').appendFileSync(${JSON.stringify(join(root,'unexpected-cli'))},'unexpected agent launch\\n');process.exit(2);`,{mode:0o700});
let runtime:Awaited<ReturnType<typeof createDesktopRuntime>>|undefined;
let wire:ReturnType<typeof startWire>|undefined;
let modelRequests=0;
const rejectModel=createServer((_req,res)=>{modelRequests++;res.writeHead(500);res.end('This fixture forbids model requests');});
await new Promise<void>(resolve=>rejectModel.listen(0,'127.0.0.1',resolve));
function startWire(){
 const safeEnv=Object.fromEntries(['HOME','PATH','USER','LOGNAME','SHELL','TMPDIR','LANG','LC_ALL','__CF_USER_TEXT_ENCODING','GIT_CONFIG_GLOBAL','GIT_CONFIG_NOSYSTEM','PI_CODING_AGENT_DIR','PI_OFFLINE'].flatMap(key=>process.env[key]===undefined?[]:[[key,process.env[key]]]));
 const child=spawn(process.execPath,[process.env.HIVE_DESKTOP_TEST_RUNTIME??fileURLToPath(new URL('../dist/main.js',import.meta.url))],{env:{...safeEnv,HIVE_DESKTOP_DATA_DIR:data},stdio:['pipe','pipe','pipe']});
 const exited=once(child,'exit');let next=0,stderr='';child.stderr.on('data',chunk=>{stderr+=String(chunk);});
 const waiting=new Map<number,{resolve(value:any):void;reject(e:Error):void}>();
 const lines=createInterface({input:child.stdout});lines.on('line',line=>{const value=JSON.parse(line),pending=waiting.get(value.id);if(pending){waiting.delete(value.id);pending.resolve(value);}});
 child.on('exit',()=>{for(const value of waiting.values())value.reject(Error('Runtime exited: '+stderr));waiting.clear();});
 const rpc=(method:string,params:Record<string,unknown>={})=>new Promise<any>((resolve,reject)=>{const id=++next,timer=setTimeout(()=>{waiting.delete(id);reject(Error('RPC timeout '+method+': '+stderr));},20000);waiting.set(id,{resolve:frame=>{clearTimeout(timer);frame.error?reject(Object.assign(Error(frame.error.message??JSON.stringify(frame.error)),{code:frame.error.code})):resolve(frame.result);},reject:error=>{clearTimeout(timer);reject(error);}});child.stdin.write(JSON.stringify({id,method,params})+'\n');});
 return {rpc,close:async()=>{if(child.exitCode===null){await rpc('shutdown');await exited;}lines.close();assert.equal(child.exitCode,0,stderr);}};
}
try{
 git(repo,'init','--template=');await writeFile(join(repo,'result.txt'),'A');git(repo,'add','.');git(repo,'commit','-qm','A');
 runtime=await createDesktopRuntime(data,()=>{},{executableEnvironment:{}});
 const ws=await runtime.hive.request('POST','/api/workspaces',{path:repo,name:'Real handoff fixture',autostart_orchestrator:false}) as {id:string};
 const createProfile=async(driver:any,label:string,launch:any)=>(await runtime!.profiles.create({driver,label,launch},(await runtime!.profiles.list()).storeVersion)).profile;
 const sourceProfile=await createProfile('codex','Source disabled after seed',{command:guarded,args:[]});
 const worker=runtime.createMember(ws.id,{name:'Handoff Worker',role:'coder',backend:'codex',profile:sourceProfile.ref});
 const source=await runtime.chats.createMemberWorktree(ws.id,worker.id);
 const sourcePath=source.execution.path;await writeFile(join(sourcePath,'result.txt'),'B');git(sourcePath,'add','result.txt');await writeFile(join(sourcePath,'result.txt'),'C');
 const index=(runtime.chats as unknown as {index:ChatIndex}).index;
 index.save({...index.get(source.id),agentId:worker.id,codexThreadId:'source-synthetic-native'});
 const nativeDirectory=join(data,'chats','codex');writeCodexMetadata(nativeDirectory,'source-synthetic-native',{version:1,context:[],deliveries:[],completed:[]});
 const entries=[{turnId:'turn1',item:{id:'u1',type:'userMessage',content:[{type:'text',text:'Inspect result.txt in this source directory.'}]}},{turnId:'turn1',item:{id:'a1',type:'agentMessage',text:'Recorded assistant statement, no inference was run.'}},{turnId:'turn1',item:{id:'tool1',type:'commandExecution',command:'cat result.txt',status:'completed',aggregatedOutput:'C',exitCode:0}}];
 await writeFile(codexMetadataFile(nativeDirectory,'source-synthetic-native')+'.display.json',JSON.stringify({version:1,entries,completedTurns:['turn1'],completeHistory:true,searchVersion:2,searchEntries:entries}));
 await runtime.profiles.enableDisable(sourceProfile.ref.id,false,(await runtime.profiles.list()).storeVersion);
 const sourceBefore=index.get(source.id),launchBefore=runtime.agents.peekAgentLaunchConfig(ws.id,worker.id),ledgerBefore=runtime.hive.executionLedgerRevision(),sourceIndex=git(sourcePath,'show',':result.txt'),sourceHead=git(sourcePath,'show','HEAD:result.txt');
 const targets=[];
 for(const driver of ['pi','codex','pi-rpc','claude','acp'] as const){
  const launch=driver==='pi'?{args:[],configDir:join(root,'target-pi')}:driver==='acp'?{command:process.execPath,args:[fileURLToPath(new URL('./fixtures/acp-runtime-peer.cjs',import.meta.url)),acp]}:{command:guarded,args:[],configDir:join(root,'target-'+driver)};
  const profile=await createProfile(driver,'Target '+driver,launch);
  const input:HandoffInput={workspaceId:ws.id,sourceSessionId:source.id,target:{driver,profile:profile.ref},selection:{throughItemId:'tool1'},code:{kind:'current'},imagePolicy:'references-only'};
  const reviewed=await runtime.chats.handoffs!.preview(input);assert.equal(reviewed.eligible,true,JSON.stringify(reviewed.blockers));
  const apply={workspaceId:ws.id,previewId:reviewed.previewId,snapshotHash:reviewed.snapshotHash,requestId:randomUUID(),acknowledgedOmissionsHash:reviewed.omissionsHash};
  const [created,repeated]=await Promise.all([runtime.chats.handoffs!.apply(apply),runtime.chats.handoffs!.apply({...apply})]);assert.equal(created.childSessionId,repeated.childSessionId);assert.equal(created.state,'ready');
  const child=index.get(created.childSessionId);assert.equal(child.originAgentId,worker.id);assert.equal(child.agentId,undefined);assert.equal(child.sessionFile,null);assert.equal(child.codexThreadId,undefined);assert.equal(child.acpSessionId,undefined);assert.deepEqual(child.profile,profile.ref);
  assert.equal(git(child.execution!.path,'show','HEAD:result.txt'),'A');assert.equal(git(child.execution!.path,'show',':result.txt'),'B');assert.equal(await readFile(join(child.execution!.path,'result.txt'),'utf8'),'C');
  assert.equal((await readWorktreeIntent(data,child.id)).creation!.handoffOperationId,created.operationId);
  const opened=await runtime.chats.open(child.id);assert.equal(opened.readOnly,false);assert.ok(opened.handoffSnapshot);assert.equal((runtime.chats as any).live.size,0);
  await assert.rejects(runtime.chats.prompt(child.id,'/help'),/ordinary nonempty text/);await assert.rejects(runtime.chats.resume(child.id),/ordinary text/);
  targets.push({driver,profile:profile.ref,id:child.id,apply});
 }
 assert.deepEqual(index.get(source.id),sourceBefore);assert.deepEqual(runtime.agents.peekAgentLaunchConfig(ws.id,worker.id),launchBefore);assert.equal(runtime.hive.executionLedgerRevision(),ledgerBefore);assert.equal(git(sourcePath,'show',':result.txt'),sourceIndex);assert.equal(git(sourcePath,'show','HEAD:result.txt'),sourceHead);assert.equal(await readFile(join(sourcePath,'result.txt'),'utf8'),'C');
 await assert.rejects(stat(join(root,'unexpected-cli')));await assert.rejects(stat(join(acp,'protocol.jsonl')));
 await runtime.close();runtime=undefined;
 wire=startWire();await wire.rpc('initialize');
 for(const target of targets){const opened=await wire.rpc('chat.open',{sessionId:target.id});assert.ok(opened.handoffSnapshot);assert.equal(opened.handoffDelivery.status,'pending');}
 await assert.rejects(stat(join(root,'unexpected-cli')));await assert.rejects(stat(join(acp,'protocol.jsonl')));
 const pi=targets.find(item=>item.driver==='pi')!;
 await assert.rejects(wire.rpc('chat.prompt',{sessionId:pi.id,text:'Keep this explicit draft until a model is selected'}),(error:any)=>error.code==='HANDOFF_MODEL_REQUIRED');
 const missingModel=await wire.rpc('chat.open',{sessionId:pi.id});assert.equal(missingModel.passive,false);assert.equal(missingModel.pending,0);assert.equal(missingModel.handoffDelivery.status,'pending');assert.equal(missingModel.model,undefined);assert.ok(missingModel.piSessionId);
 const address=rejectModel.address();assert.ok(address&&typeof address==='object');
 await mkdir(join(root,'target-pi'),{recursive:true});
 const ownedModels=JSON.stringify({providers:{'owned-handoff-fixture':{api:'openai-responses',baseUrl:`http://127.0.0.1:${address.port}/v1`,apiKey:'owned-local-placeholder-not-real',models:[{id:'gpt-6.1-terra',name:'Controlled Terra catalog only',reasoning:true,input:['text'],contextWindow:32768,maxTokens:4096}]}}});
 await writeFile(join(root,'target-pi','models.json'),ownedModels);
 await wire.rpc('chat.refreshPiModels',{sessionId:pi.id});
 await wire.rpc('chat.setModel',{sessionId:pi.id,provider:'owned-handoff-fixture',modelId:'gpt-6.1-terra'});
 const selected=await wire.rpc('chat.open',{sessionId:pi.id});assert.equal(selected.model.provider,'owned-handoff-fixture');assert.equal(selected.handoffDelivery.status,'pending');assert.equal(selected.pending,0);assert.equal(modelRequests,0);assert.equal(await readFile(join(root,'target-pi','models.json'),'utf8'),ownedModels);
 const target=targets.find(item=>item.driver==='acp')!;
 const retry=await wire.rpc('chat.handoff.apply',target.apply);assert.equal(retry.childSessionId,target.id);
 await wire.rpc('chat.prompt',{sessionId:target.id,text:'Please inspect this new worktree.\nACP_TEST_COMMAND:["list"]'});
 let opened:any;
 for(let count=0;count<200;count++){opened=await wire.rpc('chat.open',{sessionId:target.id});if(!opened.busy&&opened.pending===0)break;await new Promise(resolve=>setTimeout(resolve,25));}
 assert.equal(opened.pending,0,JSON.stringify(opened.deliveryFailure));assert.ok(opened.acpSessionId);assert.equal(opened.originAgentId,worker.id);assert.deepEqual(opened.profile,target.profile);
 const native=JSON.parse(await readFile(join(acp,opened.acpSessionId+'.json'),'utf8'));const user=native.history.find((row:any)=>row.kind==='user_message_chunk');
 assert.ok(user.text.includes('Please inspect this new worktree.'));assert.equal(user.text.split('[Historical context — recorded material only]').length-1,1);assert.ok(user.text.includes(sourcePath));assert.ok(user.text.includes('Recorded assistant statement'));
 const members=await wire.rpc('team.members',{workspaceId:ws.id});const actual=members.find((item:any)=>item.id===worker.id);assert.equal(actual.configured_command,'hive:acp');assert.deepEqual(actual.desktop_profile,target.profile);
 const old=await wire.rpc('chat.list',{workspaceId:ws.id});assert.equal(old.find((item:any)=>item.id===source.id).originAgentId,worker.id);assert.equal(old.find((item:any)=>item.id===source.id).codexThreadId,'source-synthetic-native');
 await wire.close();wire=undefined;wire=startWire();await wire.rpc('initialize');const cold=await wire.rpc('chat.open',{sessionId:target.id});assert.equal(cold.handoffDelivery.status,'consumed');assert.equal(cold.acpSessionId,opened.acpSessionId);
 const protocol=(await readFile(join(acp,'protocol.jsonl'),'utf8')).trim().split('\n').map(line=>JSON.parse(line));assert.equal(protocol.filter((row:any)=>row.method==='session/prompt').length,1);assert.equal(protocol.filter((row:any)=>row.method==='session/new').length,1);
 const coldPi=await wire.rpc('chat.open',{sessionId:pi.id});assert.equal(coldPi.passive,true);assert.equal(coldPi.handoffDelivery.status,'pending');
 await wire.rpc('chat.setModel',{sessionId:pi.id,provider:'owned-handoff-fixture',modelId:'gpt-6.1-terra'});assert.equal((await wire.rpc('chat.open',{sessionId:pi.id})).model.provider,'owned-handoff-fixture');assert.equal(modelRequests,0);
 await writeFile(join(root,'receipt.json'),JSON.stringify({pass:true,root,realGit:true,realHive:true,productionRpc:true,allFivePassiveNoCli:process.platform!=='win32',passiveCliMarkerCoverage:process.platform==='win32'?'POSIX-only executable marker; Windows zero-CLI not established by this fixture':'POSIX executable marker plus ACP protocol log',firstPlainInputTargetAcp:true,sameWorkerActualBackendSwitch:true,frozenIndexDiskCode:true,piMissingModelRetainsPending:true,explicitPiModelSelectionNoRequests:true,noInference:true},null,2));
 console.log('SESSION_HANDOFF_RUNTIME_OK '+root);
}catch(error){await writeFile(join(root,'failure.json'),JSON.stringify({message:String(error),stack:error instanceof Error?error.stack:undefined},null,2));throw error;}
finally{await wire?.close();await runtime?.close();await new Promise<void>(resolve=>rejectModel.close(()=>resolve()));for(const [key,value]of Object.entries(prior)){if(value===undefined)delete process.env[key];else process.env[key]=value;}console.log('Evidence retained '+root);}

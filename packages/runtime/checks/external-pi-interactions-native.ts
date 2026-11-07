/** Original installed Pi extensions + real Hive HTTP/SQLite; no model inference. */
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, writeFile, readFile, readdir } from 'node:fs/promises';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { createDesktopRuntime } from '../src/desktop-runtime.js';
import { execFileSync } from 'node:child_process';
import { createExternalPiSession, type ExternalPiSessionHandle } from '../src/agents/external-pi.js';
import { ExternalPiTransport } from '../src/agents/external-pi-transport.js';
import type { ExternalPiInteraction } from '../src/agents/types.js';
const directory = await mkdtemp(join(tmpdir(), 'hive-pi-interactions-'));
const cwd = join(directory, 'project'); await mkdir(join(cwd, '.pi', 'extensions'), { recursive: true });
await writeFile(join(cwd, '.pi', 'extensions', 'dialogs.ts'), `
import {appendFileSync} from 'node:fs';
import {createHash} from 'node:crypto';
export default function(pi){pi.registerCommand('hive-dialog-check',{description:'No inference callback check',async handler(args,ctx){
 const method=args.split(' ')[0];const timed=args.includes('timeout');
 const opts=timed?{timeout:150}:undefined;
 let value;
 if(method==='select')value=await ctx.ui.select('Select check',['alpha','beta'],opts);
 if(method==='confirm')value=await ctx.ui.confirm('Confirm check','Continue?',opts);
 if(method==='input')value=await ctx.ui.input('Input check','placeholder',opts);
 if(method==='editor')value=await ctx.ui.editor('Editor check','native prefill');
 appendFileSync(${JSON.stringify(join(cwd, 'answers.jsonl'))},JSON.stringify({method,undefined:value===undefined,hash:createHash('sha256').update(JSON.stringify(value)??'undefined').digest('hex')})+'\\n');
 ctx.ui.notify('native callback completed','info');
}});
 appendFileSync(${JSON.stringify(join(cwd, 'extension-loaded'))},JSON.stringify({pid:process.pid,command:'hive-dialog-check',path:${JSON.stringify(join(cwd,'.pi','extensions','dialogs.ts'))}})+'\\n');
 pi.on('before_agent_start',()=>{process.stderr.write('Inference forbidden in native interaction check\\n');process.exit(91);});
}
`);
const quote = (value: string) => "'" + value.replaceAll("'", "'\\''") + "'";
const wrapper=join(directory,'pi-wrapper.sh');
await writeFile(wrapper,`#!/bin/sh\nprintf '%s\\n' \"$$\" > ${quote(join(directory,'pi.pid'))}\nexec ${quote(process.env.HIVE_PI_COMMAND ?? '/Users/tsk/Library/pnpm/pi')} --provider openai --model gpt-6.1-terra --extension ${quote(join(cwd,'.pi','extensions','dialogs.ts'))} \"$@\"\n`,{mode:0o700});
const events: unknown[] = [];
const runtime = await createDesktopRuntime(join(directory, 'state'), event => events.push(event),{executableEnvironment:{}});
const live = () => (runtime.chats as unknown as { live: Map<string, { handle: ExternalPiSessionHandle; queue: { pending: unknown[]; pause(): Promise<void>; resume(): void } }> }).live;
const wait = async <T>(fn: () => T | Promise<T>, ready: (value: T) => boolean): Promise<T> => {
  const until = Date.now() + 8000;
  for (;;) { const value = await fn(); if (ready(value)) return value; if (Date.now() > until) throw new Error('Native dialog check timed out'); await new Promise(resolve => setTimeout(resolve, 10)); }
};
const hash = async (value: unknown) => (await import('node:crypto')).createHash('sha256').update(JSON.stringify(value) ?? 'undefined').digest('hex');
try {
 await runtime.settings.update({piCommand:wrapper,codexCommand:null,claudeCommand:null});
 const native=await ExternalPiTransport.start({cwd,sessionDir:join(directory,'preflight-session'),command:wrapper,extensionPath:new URL('../dist/agents/external-pi-extension.js',import.meta.url).pathname});
 let nativeCommands: {commands:{name:string;source:string;sourceInfo?:{path?:string}}[]};
 try { nativeCommands=await native.request('get_commands'); } finally { await native.close(); }
 const ws = await runtime.hive.request('POST','/api/workspaces',{path:cwd,name:'Pi native interactions',autostart_orchestrator:false}) as {id:string};
 const chat = await runtime.chats.create(ws.id,'pi-rpc');
 const registered=async()=>{
   const pid=Number((await readFile(join(directory,'pi.pid'),'utf8')).trim());
   const markers=(await readFile(join(cwd,'extension-loaded'),'utf8')).trim().split('\n').map(line=>JSON.parse(line));
   assert.ok(markers.some(marker=>marker.pid===pid&&marker.command==='hive-dialog-check'&&marker.path===join(cwd,'.pi','extensions','dialogs.ts')),'Actual Hive Pi PID must register the explicit command before any prompt');
   return pid;
 };
 const registeredPid=await registered();
 await runtime.chats.setModel(chat.id,'openai','gpt-6.1-terra');
 const current = live().get(chat.id)!;
 assert.equal(current.queue.pending.length,0);
 const handle = current.handle;
 assert.equal(execFileSync(process.env.HIVE_PI_COMMAND ?? '/Users/tsk/Library/pnpm/pi',['--version'],{encoding:'utf8'}).trim(),'1.4.0');
 const dialogCommand=nativeCommands.commands.filter(c=>c.name==='hive-dialog-check');
 assert.equal(dialogCommand.length,1);assert.equal(dialogCommand[0]!.source,'extension');
 assert.equal(dialogCommand[0]!.sourceInfo?.path,join(cwd,'.pi','extensions','dialogs.ts'));
 assert.equal(handle.currentModel()?.provider,'openai');assert.equal(handle.currentModel()?.id,'gpt-6.1-terra');
 const preflight={version:'1.4.0',command:dialogCommand[0],model:handle.currentModel(),queuePending:current.queue.pending.length,freshWorkspaceNoQueuedDeliveries:true,registeredPid,beforeAgentStartExitsBeforeInference:true};
 await writeFile(join(directory,'preflight.json'),JSON.stringify(preflight,null,2));
 console.log('PREFLIGHT',JSON.stringify(preflight));
 await runtime.chats.resume(chat.id);
 assert.equal(current.queue.pending.length,0);
 const start = async (method: string) => {
   assert.equal(await registered(),registeredPid);
   const done=handle.prompt('/hive-dialog-check '+method); void done.catch(()=>{});
   const snap=await wait(()=>runtime.chats.open(chat.id),s=>s.interactions.length===1);
   const card=snap.interactions[0] as ExternalPiInteraction;
   assert.equal(card.backend,'pi-rpc');assert.equal('turnId' in card,false);
   return {done,card};
 };
 const results: unknown[]=[];
 const sentinel='PI-ANSWER-ONLY-NATIVE-'+(await import('node:crypto')).randomUUID();
 for(const method of ['select','confirm','input','editor']) for(const cancel of [false,true]) {
   const {done,card}=await start(method);
   if(method==='select') await assert.rejects(runtime.chats.respondInteraction(chat.id,card.id,{value:'invented'}),e=>(e as {code?:string}).code==='INTERACTION_INVALID_RESPONSE');
   await assert.rejects(runtime.chats.respondInteraction('other-chat',card.id,{cancelled:true}),e=>(e as {code?:string}).code==='INTERACTION_STALE');
   const response=cancel?{cancelled:true}:method==='confirm'?{confirmed:true}:{value:method==='select'?'beta':''};
   await runtime.chats.respondInteraction(chat.id,card.id,response);
   await done;
   assert.equal((await runtime.chats.open(chat.id)).interactions.length,0);
   await assert.rejects(runtime.chats.respondInteraction(chat.id,card.id,response),e=>(e as {code?:string}).code==='INTERACTION_STALE');
   const answer=JSON.parse((await readFile(join(cwd,'answers.jsonl'),'utf8')).trim().split('\n').at(-1)!);
   const expected=cancel?(method==='confirm'?false:undefined):method==='confirm'?true:method==='select'?'beta':'';
   assert.equal(answer.hash,await hash(expected));results.push({method,cancel});
 }
 const timed=await start('input timeout');assert.ok(timed.card.expiresAt);await timed.done;
 const timeoutAnswer=JSON.parse((await readFile(join(cwd,'answers.jsonl'),'utf8')).trim().split('\n').at(-1)!);assert.equal(timeoutAnswer.hash,await hash(undefined));assert.equal(timeoutAnswer.undefined,true);
 await wait(()=>runtime.chats.open(chat.id),s=>s.interactions.length===0);
 await assert.rejects(runtime.chats.respondInteraction(chat.id,timed.card.id,{value:'late'}),e=>(e as {code?:string}).code==='INTERACTION_STALE');
 const stopped=await start('editor');await handle.abort();await stopped.done;
 assert.equal(handle.interactions!().length,0);
 await assert.rejects(handle.respondInteraction!(stopped.card.id,{value:'late'}),e=>(e as {code?:string}).code==='INTERACTION_STALE');
 // Dialog answers do not appear in transcript snapshots, or the durable chat index.
 assert.equal(handle.history().some(m=>(m as {role?:string}).role==='assistant'),false);
 for (const method of ['input','editor']) {
   const run=await start(method);await runtime.chats.respondInteraction(chat.id,run.card.id,{value:sentinel});await run.done;
   const answer=JSON.parse((await readFile(join(cwd,'answers.jsonl'),'utf8')).trim().split('\n').at(-1)!);assert.equal(answer.hash,await hash(sentinel));
 }
 assert.equal(JSON.stringify(events).includes(sentinel),false);
 assert.equal(JSON.stringify(handle.snapshot()).includes(sentinel),false);
 async function inspectSaved(path:string):Promise<void>{for(const entry of await readdir(path,{withFileTypes:true})){const child=join(path,entry.name);if(entry.isDirectory())await inspectSaved(child);else if(entry.isFile())assert.equal((await readFile(child)).includes(Buffer.from(sentinel)),false,child);}}
 await inspectSaved(join(directory,'state'));
 const dead=await start('editor');process.kill(Number((await readFile(join(directory,'pi.pid'),'utf8')).trim()),'SIGKILL');await assert.rejects(dead.done);assert.equal(handle.interactions!().length,0);
 await writeFile(join(directory,'events.json'),JSON.stringify(events,null,2));
 await writeFile(join(directory,'receipt.json'),JSON.stringify({pass:true,inference:false,directory,results,timeout:true,abort:true,processDeath:true,originalPi:true,uniqueSentinelNativeHashMatched:true,answersAbsentFromHiveStateAndEvents:true,preflight},null,2));
 console.log(JSON.stringify({pass:true,inference:false,directory,results,timeout:true,abort:true,originalPi:true}));
}catch(error){await writeFile(join(directory,"events.json"),JSON.stringify(events,null,2));console.error("EVIDENCE",directory);throw error;}finally{await runtime.close();}
// Upstream Pi awaits session_start before stdin is read. Fail with diagnosis, promptly.
const startup=join(directory,'startup');await mkdir(join(startup,'.pi','extensions'),{recursive:true});
await writeFile(join(startup,'.pi','extensions','startup.ts'),`export default function(pi){pi.on('session_start',async(_event,ctx)=>{await ctx.ui.confirm('Startup','Awaited before stdin reader');})}`);
const startupWrapper=join(startup,'pi-start.sh');
await writeFile(startupWrapper,`#!/bin/sh\nexec ${quote(process.env.HIVE_PI_COMMAND ?? '/Users/tsk/Library/pnpm/pi')} --extension ${quote(join(startup,'.pi','extensions','startup.ts'))} \"$@\"\n`,{mode:0o700});
const started=Date.now();
await assert.rejects(createExternalPiSession({command:startupWrapper,cwd:startup,sessionDir:join(startup,'sessions'),onEvent(){}}),/startup dialog cannot be answered/);
assert.ok(Date.now()-started<10000);
await assert.rejects(ExternalPiTransport.start({command:startupWrapper,cwd:startup,sessionDir:join(startup,'catalog'),extensionPath:new URL('../dist/agents/external-pi-extension.js',import.meta.url).pathname}),/startup dialog cannot be answered/);
console.log(JSON.stringify({startupFailureBounded:true,elapsedMs:Date.now()-started}));

const catalogRuntime=await createDesktopRuntime(join(directory,'catalog-state'),()=>{},{executableEnvironment:{}});
try {
 await catalogRuntime.settings.update({piCommand:startupWrapper,codexCommand:null,claudeCommand:null});
 const ws=await catalogRuntime.hive.request('POST','/api/workspaces',{path:startup,name:'Startup catalog diagnosis',autostart_orchestrator:false}) as {id:string};
 const began=Date.now();await assert.rejects(catalogRuntime.agentModels.list(ws.id,'pi-rpc'),/startup dialog cannot be answered/);
 assert.ok(Date.now()-began<10000);
 await writeFile(join(directory,'startup-receipt.json'),JSON.stringify({sessionFailureBounded:true,catalogFailureBounded:true,diagnosis:'Pi 1.4 awaits session_start before stdin reader',elapsedMs:Date.now()-began},null,2));
}finally{await catalogRuntime.close();}

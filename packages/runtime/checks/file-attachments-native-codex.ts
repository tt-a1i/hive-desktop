/** One real native Codex Terra inference on an owned uploaded text file. No fallback model. */
import assert from 'node:assert/strict';
import {mkdtemp,mkdir,writeFile,readFile,unlink} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
import {randomUUID} from 'node:crypto';
import {setTimeout as delay} from 'node:timers/promises';
import {createDesktopRuntime} from '../src/desktop-runtime.js';
import {ChatIndex} from '../src/chat-index.js';
const dir=await mkdtemp(join(tmpdir(),'hive-file-native-codex-'));const data=join(dir,'desktop');const project=join(dir,'owned project');await mkdir(project);
const command=process.env.HIVE_CODEX_COMMAND;assert.ok(command,'Explicit original Codex CLI required');
const receipt:Record<string,unknown>={pass:false,dir,requestedModel:'openai/gpt-6.1-terra',realNative:true,inferenceAttempted:false};
let runtime:Awaited<ReturnType<typeof createDesktopRuntime>>|undefined;let id:string|undefined;
try{
 runtime=await createDesktopRuntime(data,()=>{},{executableEnvironment:{HIVE_CODEX_COMMAND:command}});
 const ws=await runtime.hive.request('POST','/api/workspaces',{path:project,name:'文件原生读取',autostart_orchestrator:false})as{id:string};const record=await runtime.chats.createRecord(ws.id,'codex');id=record.id;
 const source=join(dir,'需求 中文.txt');const marker='uploaded-'+randomUUID();await writeFile(source,'Read-only attachment fixture.\nmarker='+marker+'\n');const file=await runtime.chats.attachments.add(ws.id,source);assert.ok(file.filePath);await unlink(source);receipt.originalDeleted=true;receipt.uploadedPath=file.filePath;
 await runtime.close();runtime=undefined;const index=new ChatIndex(join(data,'chats'));index.save({...index.get(id),model:{provider:'openai',id:'gpt-6.1-terra',name:'Terra native file proof'}});
 runtime=await createDesktopRuntime(data,()=>{},{executableEnvironment:{HIVE_CODEX_COMMAND:command}});const opened=await runtime.chats.open(id);assert.equal(opened.model?.provider,'openai');assert.equal(opened.model?.id,'gpt-6.1-terra');
 receipt.inferenceAttempted=true;await runtime.chats.prompt(id,'Use a read-only local tool to read the attached file copy. Reply with only the exact value after marker= inside that file. Do not write or execute any file, and do not create a team. The original source was intentionally deleted; use the attached saved copy.',[file.attachment.id]);
 const deadline=Date.now()+120000;let final:any;
 while(Date.now()<deadline){const state=await runtime.chats.open(id);if(state.interactions?.some((i:any)=>i.state==='pending'))throw new Error('Native file read requires a user interaction; no automatic permission was granted');if(!state.busy&&state.history.some((m:any)=>m.role==='assistant')){final=state;break;}await delay(100);}
 assert.ok(final,'Native file-read turn did not finish within 120 seconds');const answers=final.history.filter((m:any)=>m.role==='assistant').flatMap((m:any)=>m.content??[]).filter((c:any)=>c.type==='text').map((c:any)=>c.text).join('\n');assert.ok(answers.includes(marker),'Native model did not return the marker known only from uploaded bytes');assert.ok(final.history.some((m:any)=>m.role==='toolResult'&&JSON.stringify(m.toolInput??m.rawBackendItem).includes(file.filePath!)),'Actual tool input must address the saved upload copy');assert.ok((await readFile(file.filePath!,'utf8')).includes(marker));
 receipt.pass=true;receipt.model=opened.model;receipt.toolMessages=final.history.filter((m:any)=>m.role==='toolResult').length;receipt.markerReturned=true;receipt.nativeSessionPreserved=runtime.chats.list(ws.id).find(r=>r.id===id)?.codexThreadId;
}catch(error){receipt.error=error instanceof Error?error.message:String(error);process.exitCode=1;}
finally{if(runtime){if(id)await runtime.chats.abort(id).catch(error=>{receipt.abortError=String(error);});await runtime.close();}await mkdir(resolve('../../.artifacts/file-attachments'),{recursive:true});await writeFile(resolve('../../.artifacts/file-attachments/native-codex-receipt.json'),JSON.stringify(receipt,null,2));console.log(JSON.stringify(receipt));}

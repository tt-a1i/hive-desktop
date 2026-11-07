/** Real subprocess JSONL; synthetic peer isolates deadline/duplicate/lifecycle behavior. */
import assert from 'node:assert/strict';
import { mkdtemp, writeFile, readFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { ExternalPiTransport } from '../src/agents/external-pi-transport.js';
import { ExternalPiInteractions } from '../src/agents/external-pi-interactions.js';
import type { PiEvent } from '../src/agents/types.js';
const directory=await mkdtemp(join(tmpdir(),'hive-pi-dialog-jsonl-'));
const executable=join(directory,'peer');
await writeFile(executable,`#!${process.execPath}
const {createInterface}=require('node:readline');
const {appendFileSync}=require('node:fs');
const send=x=>process.stdout.write(JSON.stringify(x)+'\\n');
let held;
createInterface({input:process.stdin}).on('line',line=>{
 const q=JSON.parse(line);const answer=data=>send({type:'response',id:q.id,command:q.type,success:true,data});
 if(q.type==='extension_ui_response'){
  appendFileSync(${JSON.stringify(join(directory,'responses.jsonl'))},JSON.stringify({id:q.id,cancelled:q.cancelled===true})+'\\n');
  if(held){send({type:'response',id:held.id,command:'prompt',success:true,data:{disposition:'handled'}});held=null;}return;
 }
 if(q.type==='get_state')answer({sessionId:'fixture'});
 else if(q.type==='get_commands')answer({commands:[{name:process.env.HIVE_PI_BRIDGE_COMMAND,source:'extension',sourceInfo:{path:process.argv[process.argv.indexOf('--extension')+1]}}]});
 else if(q.type==='prompt'&&q.message.startsWith('/'+process.env.HIVE_PI_BRIDGE_COMMAND+' '))answer({disposition:'handled'});
 else if(q.type==='prompt'){
   held=q;const dialog={type:'extension_ui_request',id:'native-request',method:'input',title:'Human input'};
   send(dialog);if(q.message==='duplicate')send(dialog);
 }else if(q.type==='missing'){}else answer({});
});
`,{mode:0o700});
const events:PiEvent[]=[];
let transport:ExternalPiTransport;
const manager=new ExternalPiInteractions(()=> 'fixture',event=>events.push(event),id=>transport.expireUiRequest(id));
let closed!: (error:Error)=>void;
const died=new Promise<Error>(resolve=>{closed=resolve;});
transport=await ExternalPiTransport.start({cwd:directory,sessionDir:directory,extensionPath:join(directory,'bridge.js'),command:executable,requestTimeoutMs:1500,
 onDialogRequest:(request,write)=>manager.request(request,write),onClose:error=>{manager.clear('disconnected');closed(error);}});
const wait=async()=>{for(let n=0;n<100;n++){if(manager.snapshot().length)return;await new Promise(resolve=>setTimeout(resolve,5));}throw new Error('No native fixture dialog');};
try{
 let settled=false;
 const prompt=transport.request('prompt',{message:'wait'}).then(()=>{settled=true;});
 await wait();
 const first=manager.snapshot()[0]!;
 await assert.rejects(transport.request('missing'),/timed out: missing/);
 await new Promise(resolve=>setTimeout(resolve,2000));
 assert.equal(settled,false);assert.equal(manager.snapshot().length,1);
 await manager.respond(first.id,{value:''});await prompt;
 assert.equal(manager.snapshot().length,0);
 assert.equal(events.filter(e=>e.type==='interaction_resolved'&&e.reason==='resolved').length,1);
 await assert.rejects(manager.respond(first.id,{value:'late'}),e=>(e as {code?:string}).code==='INTERACTION_STALE');
 const duplicate=transport.request('prompt',{message:'duplicate'});await assert.rejects(duplicate,/duplicate external Pi UI request/);
 assert.match((await died).message,/duplicate external Pi UI request/);assert.equal(manager.snapshot().length,0);
 await assert.rejects(manager.respond(first.id,{value:'late'}));
 const replies=(await readFile(join(directory,'responses.jsonl'),'utf8')).trim().split('\n').map(JSON.parse);
 assert.deepEqual(replies,[{id:'native-request',cancelled:false}]);
}finally{await transport.close();}
const receipt={pass:true,synthetic:true,noModelRequests:true,directory,promptHumanWaitExcluded:true,otherOperationBounded:true,duplicateCloses:true,noLateWrites:true};
await writeFile(join(directory,'receipt.json'),JSON.stringify(receipt,null,2));
console.log(JSON.stringify(receipt));

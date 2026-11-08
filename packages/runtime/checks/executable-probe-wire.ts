/** Compiled JSONL runtime (optionally extracted package), actual process and settings; no model. */
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {once} from 'node:events';
import {mkdtemp,readFile,writeFile,mkdir} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
import {createInterface} from 'node:readline';
import {setTimeout as delay} from 'node:timers/promises';
const dir=await mkdtemp(join(tmpdir(),'hive-program-wire-'));
const node=process.env.HIVE_PROGRAM_NODE??process.execPath;
const runtime=resolve(process.env.HIVE_PROGRAM_RUNTIME??'.');
const script=join(dir,'version ; literal.cjs');await writeFile(script,"#!/usr/bin/env node\nif(process.argv.slice(2).join(' ')!=='--version')process.exit(4);console.log('pi 1.4.0');",{mode:0o700});
let command=script;
if(process.platform==='win32'){command=join(dir,'version ; literal.cmd');await writeFile(command,`@echo off\r\n"${node}" "${script}" %*\r\n`);}
const slow=join(dir,'slow.cjs');const marker=join(dir,'slow.pid');await writeFile(slow,`#!/usr/bin/env node\nrequire('node:fs').writeFileSync(${JSON.stringify(marker)},String(process.pid));setInterval(()=>{},1000);`,{mode:0o700});
let slowCommand=slow;if(process.platform==='win32'){slowCommand=join(dir,'slow.cmd');await writeFile(slowCommand,`@echo off\r\n"${node}" "${slow}" %*\r\n`);}
const child=spawn(node,[join(runtime,'dist/main.js')],{cwd:dir,env:{...process.env,HIVE_DESKTOP_DATA_DIR:join(dir,'desktop'),HIVE_PI_COMMAND:'',HIVE_CODEX_COMMAND:'',HIVE_CLAUDE_COMMAND:''},stdio:['pipe','pipe','pipe']});
const closed=once(child,'close');let stderr='';child.stderr.on('data',chunk=>stderr+=chunk);
let sequence=0;const pending=new Map<number,{resolve:(value:any)=>void;reject:(error:Error)=>void}>();
const lines=createInterface({input:child.stdout});lines.on('line',line=>{const value=JSON.parse(line);pending.get(value.id)?.resolve(value);});
child.on('close',()=>{for(const request of pending.values())request.reject(new Error('Runtime closed: '+stderr));pending.clear();});
async function rpc(method:string,params={}){const id=++sequence;let timer:ReturnType<typeof setTimeout>;try{return await new Promise<any>((yes,no)=>{pending.set(id,{resolve:yes,reject:no});timer=setTimeout(()=>no(new Error('Timed out: '+method)),15000);child.stdin.write(JSON.stringify({id,method,params})+'\n');});}finally{clearTimeout(timer!);pending.delete(id);}}
try{
 assert.ok((await rpc('initialize')).result.hivePort);const before=(await rpc('desktop.settings.get')).result;
 const checked=await rpc('desktop.settings.probe',{backend:'pi',command});assert.equal(checked.error,undefined);assert.equal(checked.result.version,'1.4.0');assert.equal(checked.result.source,'draft');assert.equal(checked.result.command,command);
 assert.deepEqual((await rpc('desktop.settings.get')).result,before);
 for(const params of [{backend:'unknown',command},{backend:'pi',command:4},{backend:'pi'}])assert.ok((await rpc('desktop.settings.probe',params)).error);
 const waiting=rpc('desktop.settings.probe',{backend:'claude',command:slowCommand});
 const deadline=Date.now()+5000;let pid:number|undefined;
 while(Date.now()<deadline){try{pid=Number(await readFile(marker,'utf8'));break;}catch{}await delay(10);}
 assert.ok(pid);const stopping=rpc('shutdown');assert.equal((await waiting).result.status,'cancelled');assert.equal((await stopping).result.ok,true);assert.equal((await closed)[0],0);
 assert.throws(()=>process.kill(pid!,0),(error:NodeJS.ErrnoException)=>error.code==='ESRCH');
 await mkdir(resolve('../../.artifacts/program-check'),{recursive:true});await writeFile(resolve('../../.artifacts/program-check/wire-receipt.json'),JSON.stringify({pass:true,dir,runtime,node,modelInference:false,processExited:true},null,2));
 console.log('PROGRAM_WIRE_OK: compiled probe RPC, strict input, unsaved path, live shutdown cancellation and process exit. No model request.');
}finally{if(child.exitCode===null&&child.signalCode===null){child.kill('SIGKILL');await closed;}lines.close();}

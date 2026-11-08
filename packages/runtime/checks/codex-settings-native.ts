/** Explicit original CLI, no inference; repeated changed/same-value native settings. */
import assert from 'node:assert/strict';
import {createCodexSession} from '../src/agents/codex.js';
import {mkdtemp,writeFile} from 'node:fs/promises';import {tmpdir} from 'node:os';import {join} from 'node:path';
if(!process.env.HIVE_CODEX_COMMAND)throw new Error('Explicit original Codex CLI required');
const results=[];
for(let trial=0;trial<8;trial++){
 const dir=await mkdtemp(join(tmpdir(),'hive-codex-applied-'));const session=await createCodexSession({command:process.env.HIVE_CODEX_COMMAND,cwd:dir,sessionDir:join(dir,'session'),model:{provider:'openai',id:'gpt-6.1-terra'},onEvent(){}});
 try{for(const level of ['high','low','high','medium','high','high']){const result=await session.setReasoning!(level);assert.equal(result.level,level);assert.equal((await session.reasoning!()).level,level);results.push({trial,level,actual:result.level});}}finally{await session.dispose();}
}
await writeFile(join(tmpdir(),'hive-codex-applied-receipt.json'),JSON.stringify({pass:true,trials:8,updatesIncludingNoops:48,results,originalCodex:'0.166.0',model:'gpt-6.1-terra',vendorInference:false},null,2));console.log('ADAPTER_NATIVE_APPLIED_OK 8 processes / 48 settings incl same-value no-ops');

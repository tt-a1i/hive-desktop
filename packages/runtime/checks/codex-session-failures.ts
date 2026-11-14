import assert from 'node:assert/strict';
import { setImmediate as nextImmediate } from 'node:timers/promises';
import { mkdir, mkdtemp, writeFile } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { Type } from 'typebox';
import { createCodexSession } from '../src/agents/codex.js';

// A real JSONL subprocess with synthetic server behavior. No model requests and
// no claim this fixture verifies the installed Codex backend's protocol details.
const artifacts = resolve('.artifacts');
await mkdir(artifacts, { recursive: true });
const directory = await mkdtemp(join(artifacts, 'codex-session-failures-'));
await writeFile(join(directory, 'package.json'), JSON.stringify({ type: 'commonjs' }));
const fixture = `const {createInterface}=require('node:readline');
const {existsSync}=require('node:fs');
const mode = process.cwd().endsWith('exit') ? 'exit' : 'callback';
(async()=>{
let counter=0;
const thread={id:'fixture-thread',path:null,historyMode:'legacy',turns:[]};
const send=value=>process.stdout.write(JSON.stringify(value)+'\\n');
const notify=(method,params)=>send({method,params:{threadId:thread.id,...params}});
for await(const line of createInterface({input:process.stdin})) {
 const q=JSON.parse(line); if(!q.method) continue;
 const answer=result=>send({id:q.id,result});
 if(q.method==='initialize')answer({});
 else if(q.method==='model/list')answer({data:[{id:'terra',model:'gpt-6.1-terra',displayName:'Terra',hidden:false}],nextCursor:null});
 else if(q.method==='thread/start')answer({thread,model:'gpt-6.1-terra',modelProvider:'openai'});
 else if(q.method==='turn/start') {
  if(mode==='callback' && counter>0 && !existsSync('../binding-restored')) {
   send({id:q.id,error:{code:-32000,message:'Binding was not persisted before second turn'}});continue;
  }
  const turn={id:'turn-'+(++counter),status:'inProgress',items:[]};
  thread.turns.push(turn); answer({turn}); notify('turn/started',{turn});
  if(mode==='exit') {
   send({id:'tool-call',method:'item/tool/call',params:{threadId:thread.id,turnId:turn.id,callId:'owned-tool',tool:'owned',arguments:{}}});
   setTimeout(()=>process.exit(7),40);
  } else if(counter>1) {
   turn.status='completed'; notify('turn/completed',{turn});
  }
 } else if(q.method==='turn/interrupt') {
  const turn=thread.turns.at(-1); turn.status='interrupted'; answer({}); notify('turn/completed',{turn});
 } else if(q.method==='thread/read')answer({thread});
}
})().catch(error=>{console.error(error);process.exit(1)});
`;
const exitDirectory = join(directory, 'exit');
const callbackDirectory = join(directory, 'callback');
await mkdir(exitDirectory);
await mkdir(callbackDirectory);
await writeFile(join(exitDirectory, 'app-server'), fixture);
await writeFile(join(callbackDirectory, 'app-server'), fixture);
const unexpectedExits: Error[] = [];
const options = { onUnexpectedExit: (error: Error) => { unexpectedExits.push(error); }, cwd: directory, sessionDir: directory, onEvent() {}, model: { provider: 'openai', id: 'gpt-6.1-terra' } };
let finishCleanup!: () => void;
let observedAbort!: () => void;
const aborted = new Promise<void>(resolve => { observedAbort = resolve; });
const cleanup = new Promise<void>(resolve => { finishCleanup = resolve; });
const exiting = await createCodexSession({ ...options, command: process.execPath, cwd: exitDirectory, customTools: [{
  name: 'owned', description: 'Synthetic owned task', parameters: Type.Object({}),
  async execute(_id, _args, signal) {
    signal!.addEventListener('abort', observedAbort, { once: true });
    await cleanup;
    assert.equal(signal!.aborted, true);
    throw new Error('Owned task cleanup finished');
  },
}] });
let settled = false;
const dyingPrompt = exiting.prompt('fixture exit').finally(() => { settled = true; });
void dyingPrompt.catch(() => {});
try {
  await Promise.race([aborted, new Promise((_, reject) => setTimeout(() => reject(new Error('Owned tool was not aborted')), 3000))]);
  await nextImmediate();
  assert.equal(unexpectedExits.length, 1);
  assert.equal(settled, false, 'Backend exit must wait for owned tool cleanup');
  assert.throws(() => exiting.dispose(), /Abort and await/);
  await assert.rejects(exiting.prompt('must stay busy'), /already processing/);
} finally { finishCleanup(); }
await assert.rejects(dyingPrompt, /app-server/);
await exiting.dispose();

let attempts = 0;
const callback = await createCodexSession({ ...options, command: process.execPath, cwd: callbackDirectory,
  async onMaterialized() {
    if (++attempts === 1) throw new Error('Identity persistence failed');
    await writeFile(join(directory, 'binding-restored'), 'fixture-thread');
  },
});
try {
  await assert.rejects(callback.prompt('first'), /Identity persistence failed/);
  assert.equal(callback.materialized, true, 'Backend identity remains materialized despite persistence failure');
  await callback.prompt('second');
  assert.equal(attempts, 2, 'Next prompt must retry failed identity persistence');
} finally { await callback.dispose(); }
assert.equal(unexpectedExits.length, 1, 'Intentional disposal must not report an unexpected exit');
await assert.rejects(createCodexSession({ ...options, command: process.execPath, cwd: callbackDirectory,
  model: { provider: 'openai', id: 'missing-model' },
}), /did not select/);
assert.equal(unexpectedExits.length, 1, 'Failed construction must not report an initialized session exit');
await writeFile(join(directory, 'receipt.json'), JSON.stringify({ synthetic: true, noModelRequests: true,
  backendDeathAbortsAndReapsTool: true, materializationCallbackRetried: true, unexpectedExitOnlyForInitializedUnintentionalClose: true }, null, 2));
console.log(`Synthetic Codex session failure checks passed: ${directory}`);

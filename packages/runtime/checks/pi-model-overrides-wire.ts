/** Real production JSONL + Hive HTTP/SQLite + isolated Pi override configuration. No model calls. */
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createInterface } from 'node:readline';

const directory = await mkdtemp(join(tmpdir(), 'hive-model-overrides-wire-'));
const agentDir = join(directory, 'pi');
await mkdir(agentDir);
const settings = JSON.stringify({ defaultProvider: 'openai', defaultModel: 'gpt-6.1-terra' });
await writeFile(join(agentDir, 'settings.json'), settings);
const child = spawn(process.execPath, ['dist/main.js'], { cwd: process.cwd(),
  env: { ...process.env, PI_CODING_AGENT_DIR: agentDir, PI_OFFLINE: '1', HIVE_DESKTOP_DATA_DIR: join(directory, 'desktop') },
  stdio: ['pipe', 'pipe', 'pipe'],
});
const exited = once(child, 'exit');
let stderr = '';
child.stderr.on('data', chunk => { stderr += String(chunk); });
let sequence = 0;
const pending = new Map<number, { resolve(value: any): void; reject(error: Error): void }>();
const lines = createInterface({ input: child.stdout });
lines.on('line', line => {
  const response = JSON.parse(line);
  const request = pending.get(response.id);
  if (request) { pending.delete(response.id); request.resolve(response); }
});
child.on('exit', () => {
  for (const request of pending.values()) request.reject(new Error('Runtime exited before reply: ' + stderr));
  pending.clear();
});
function rpc(method: string, params = {}) {
  return new Promise<any>((resolve, reject) => {
    const id = ++sequence;
    const timeout = setTimeout(() => { pending.delete(id); reject(new Error('Runtime auth request timed out: ' + method)); }, 10_000);
    pending.set(id, { resolve: value => { clearTimeout(timeout); resolve(value); }, reject: error => { clearTimeout(timeout); reject(error); } });
    child.stdin.write(JSON.stringify({ id, method, params }) + '\n');
  });
}
try {
  const configPath = join(agentDir, 'models.json');
  await writeFile(configPath, JSON.stringify({providers:{openai:{modelOverrides:{'gpt-6.1-terra':{name:'Wire override',headers:{'X-Keep':'value'}}}}}}));
  const initialized = await rpc('initialize');
  assert.ok(initialized.result.hivePort);
  const workspaces = await rpc('hive.request',{method:'GET',path:'/api/workspaces'});
  assert.equal(workspaces.error,undefined);assert.deepEqual(workspaces.result,[]);
  const first = await rpc('pi.models.get');
  assert.equal(first.error,undefined);
  const model = first.result.providers[0].models[0];
  assert.equal(model.id,'gpt-6.1-terra');
  assert.equal(model.overrideOnly,true);
  assert.equal(model.reasoning,null);
  assert.equal(model.vision,null);
  const change = {action:'model',id:'openai',modelId:model.id,name:'Wire edit',contextWindow:262144,maxTokens:null,reasoning:false,vision:null};
  const changed = await rpc('pi.models.change',{revision:first.result.revision,change});
  assert.equal(changed.error,undefined);
  assert.equal(changed.result.conflict,false);
  assert.equal(changed.result.snapshot.providers[0].models[0].reasoning,false);
  const saved = JSON.parse(await readFile(configPath,'utf8'));
  assert.equal(saved.providers.openai.models,undefined);
  assert.equal(saved.providers.openai.modelOverrides[model.id].reasoning,false);
  assert.equal(saved.providers.openai.modelOverrides[model.id].input,undefined);
  assert.deepEqual(saved.providers.openai.modelOverrides[model.id].headers,{'X-Keep':'value'});
  const stale = await rpc('pi.models.change',{revision:first.result.revision,change:{...change,name:'Must not save'}});
  assert.equal(stale.result.conflict,true);
  const reset = await rpc('pi.models.change',{revision:changed.result.snapshot.revision,change:{...change,reasoning:null,contextWindow:null}});
  assert.equal(reset.error,undefined);
  assert.equal(reset.result.snapshot.providers[0].models[0].reasoning,null);
  const removed = await rpc('pi.models.change',{revision:reset.result.snapshot.revision,change:{action:'removeModel',id:'openai',modelId:model.id}});
  assert.equal(removed.error,undefined);
  assert.deepEqual(removed.result.snapshot.providers,[]);
  assert.deepEqual(JSON.parse(await readFile(configPath,'utf8')).providers,{});
  assert.equal(await readFile(join(agentDir,'settings.json'),'utf8'),settings);
  assert.ok((await rpc('shutdown')).result);
  child.stdin.end();
  const [code,signal] = await exited;
  assert.equal(code,0);assert.equal(signal,null);
  console.log('PI_MODEL_OVERRIDES_WIRE_OK: production JSONL, real Hive HTTP/SQLite, nullable inheritance, disk edit/reset/remove, stale conflict, original defaults and clean shutdown. No inference.');
} finally {
  lines.close();
  if(child.exitCode===null&&child.signalCode===null){child.kill('SIGTERM');await exited;}
  await rm(directory,{recursive:true,force:true});
}

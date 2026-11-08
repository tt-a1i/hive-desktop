/** Production desktop JSONL, real Hive and isolated Pi configuration; no vendor requests. */
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createInterface } from 'node:readline';

const directory = await mkdtemp(join(tmpdir(), 'hive-usage-wire-'));
const agentDir = join(directory, 'pi');
await mkdir(agentDir);
const settings = JSON.stringify({ defaultProvider: 'reasoning-wire', defaultModel: 'gpt-6.1-terra', defaultThinkingLevel: 'low' });
await writeFile(join(agentDir, 'settings.json'), settings);
await writeFile(join(agentDir, 'models.json'), JSON.stringify({providers:{'reasoning-wire':{api:'openai-responses',apiKey:'fixture-only',baseUrl:'http://127.0.0.1:1/v1',models:[{id:'gpt-6.1-terra',name:'Terra wire fixture',reasoning:true,input:['text']}]}}}));
const child = spawn(process.execPath, ['dist/main.js'], {
  cwd: process.cwd(),
  env: {
    ...process.env,
    PI_CODING_AGENT_DIR: agentDir,
    PI_OFFLINE: '1',
    HIVE_DESKTOP_DATA_DIR: join(directory, 'desktop'),
  },
  stdio: ['pipe', 'pipe', 'pipe'],
});
const exited = once(child, 'exit');
let stderr = '';
child.stderr.on('data', (chunk) => {
  stderr += String(chunk);
});
const notifications: any[] = [];
let sequence = 0;
const pending = new Map<number, { resolve(value: any): void; reject(error: Error): void }>();
const lines = createInterface({ input: child.stdout });
lines.on('line', (line) => {
  const response = JSON.parse(line);
  if (response.event === 'agent') notifications.push(response.data);
  const request = pending.get(response.id);
  if (request) {
    pending.delete(response.id);
    request.resolve(response);
  }
});
child.on('exit', () => {
  for (const request of pending.values()) request.reject(new Error('Runtime exited before reply: ' + stderr));
  pending.clear();
});
function rpc(method: string, params = {}) {
  return new Promise<any>((resolve, reject) => {
    const id = ++sequence;
    const timeout = setTimeout(() => {
      pending.delete(id);
      reject(new Error('Runtime usage request timed out: ' + method));
    }, 10_000);
    pending.set(id, {
      resolve: (value) => {
        clearTimeout(timeout);
        resolve(value);
      },
      reject: (error) => {
        clearTimeout(timeout);
        reject(error);
      },
    });
    child.stdin.write(JSON.stringify({ id, method, params }) + '\n');
  });
}
try {
  assert.ok((await rpc('initialize')).result.hivePort);
  const workspace=(await rpc('hive.request',{method:'POST',path:'/api/workspaces',body:{path:directory,name:'会话强度协议',autostart_orchestrator:false}})).result;
  const created=await rpc('chat.create',{workspaceId:workspace.id,backend:'pi'});
  assert.equal(created.error,undefined);const id=created.result.id;
  assert.equal(created.result.model.id,'gpt-6.1-terra');assert.equal(created.result.model.provider,'reasoning-wire');
  const first=await rpc('chat.usage',{sessionId:id});
  assert.equal(first.error,undefined);
  assert.equal(first.result.source,'内置 Pi 原生会话');
  assert.equal(first.result.reports[0].counters.find((c:any)=>c.key==='total').tokens,0);
  assert.ok(!first.result.context || first.result.context.window>0);
  assert.ok((await rpc('chat.usage',{sessionId:'missing-session'})).error);
  assert.deepEqual((await rpc('chat.usage',{sessionId:id})).result,first.result);
  assert.equal(notifications.filter(e=>['text_delta','tool_start','settled'].includes(e.type)).length,0);
  assert.equal(await readFile(join(agentDir,'settings.json'),'utf8'),settings);
  assert.equal((await rpc('shutdown')).result.ok,true);await exited;assert.equal(child.exitCode,0);
  console.log('AGENT_USAGE_WIRE_OK: production JSONL reads actual Pi stats, missing session errors, repeated reads preserve native counters/global settings, no prompt.');
} finally {
  if(child.exitCode===null&&child.signalCode===null){child.kill('SIGKILL');await exited;}
  lines.close();await rm(directory,{recursive:true,force:true});
}

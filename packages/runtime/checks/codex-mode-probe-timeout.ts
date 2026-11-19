/** Controlled unresponsive app-server; exercises production request deadlines without inference. */
import assert from 'node:assert/strict';
import {mkdtemp, readFile, rm, writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {CodexTransport} from '../src/agents/codex-transport.js';
import {CodexModes} from '../src/agents/codex-modes.js';
const dir=await mkdtemp(join(tmpdir(),'hive-hanging-modes-'));
let transport:CodexTransport|undefined, otherTransport:CodexTransport|undefined;
try {
 await writeFile(join(dir,'modes-control.json'),JSON.stringify({hangRequirements:true,hangMode:true}));
 transport=await CodexTransport.start({cwd:dir,requestTimeoutMs:120,command:process.execPath,args:[fileURLToPath(new URL('./codex-modes-peer.cjs',import.meta.url))]});
 const pid=transport.pid!;const modes=new CodexModes(transport);
 const started=Date.now();await Promise.all([modes.load(),modes.load()]);const elapsed=Date.now()-started;
 assert.ok(elapsed>=220&&elapsed<4000,`unexpected timeout: ${elapsed}`);
 assert.equal(modes.mode().options.length,0);assert.equal(modes.permissionMode().options.length,0);
 assert.match(modes.mode().description??'',/无法确定/);
 const wire=(await readFile(join(dir,'modes-wire.jsonl'),'utf8')).trim().split('\n').map(line=>JSON.parse(line));
 assert.equal(wire.filter(row=>row.method==='configRequirements/read').length,1);
 assert.equal(wire.filter(row=>row.method==='collaborationMode/list').length,1);
 await modes.load();
 const retry=(await readFile(join(dir,'modes-wire.jsonl'),'utf8')).trim().split('\n').map(line=>JSON.parse(line));
 assert.equal(retry.filter(row=>row.method==='configRequirements/read').length,2);
 assert.equal(retry.filter(row=>row.method==='collaborationMode/list').length,2);
 // A later successful attempt remains cached for this session.
 await writeFile(join(dir,'modes-control.json'),'{}');
 await Promise.all([modes.load(),modes.load()]);
 assert.ok(modes.mode().options.length);assert.ok(modes.permissionMode().options.length);
 const count=async()=>{const rows=(await readFile(join(dir,'modes-wire.jsonl'),'utf8')).trim().split('\n').map(line=>JSON.parse(line));return [rows.filter(row=>row.method==='configRequirements/read').length,rows.filter(row=>row.method==='collaborationMode/list').length];};
 assert.deepEqual(await count(),[3,3]);await modes.load();assert.deepEqual(await count(),[3,3]);
 // Unsupported is cached per axis, while another session has independent state.
 await writeFile(join(dir,'modes-control.json'),JSON.stringify({error:-32601}));
 const unsupported=new CodexModes(transport);
 await Promise.all([unsupported.load(),unsupported.load()]);
 assert.equal(unsupported.mode().options.length,0);assert.ok(unsupported.permissionMode().options.length);
 assert.deepEqual(await count(),[4,4]);await unsupported.load();assert.deepEqual(await count(),[4,4]);
 await writeFile(join(dir,'modes-control.json'),JSON.stringify({requirements:{allowedSandboxModes:['read-only']}}));
 otherTransport=await CodexTransport.start({cwd:dir,requestTimeoutMs:120,command:process.execPath,args:[fileURLToPath(new URL('./codex-modes-peer.cjs',import.meta.url))]});
 const otherPid=otherTransport.pid!;assert.notEqual(otherPid,pid);
 const other=new CodexModes(otherTransport);await other.load();
 assert.deepEqual(other.permissionMode().options.map(item=>item.id),['read-only']);
 assert.equal(unsupported.mode().options.length,0);
 await otherTransport.close();otherTransport=undefined;
 if(process.platform!=='win32')assert.throws(()=>process.kill(otherPid,0),{code:'ESRCH'});
 assert.ok(!wire.some(row=>row.method==='turn/start'||row.method==='turn/steer'));
 await transport.close();transport=undefined;
 if(process.platform!=='win32')assert.throws(()=>process.kill(pid,0),{code:'ESRCH'});
 console.log(`CODEX_NORMAL_MODE_HANG_MS=${elapsed}; concurrent probes shared; failed probe retried; no turn; process reaped on close`);
} finally {await otherTransport?.close();await transport?.close();await rm(dir,{recursive:true,force:true});}

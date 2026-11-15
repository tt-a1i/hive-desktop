/** Actual Hive SQLite app_state via two independent connections. No scripts are run. */
import assert from 'node:assert/strict';
import { DatabaseSync } from 'node:sqlite';
import { mkdtemp, mkdir, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createDesktopRuntime } from '../src/desktop-runtime.js';
import { ProjectScriptSettings, type ProjectScriptStatePort } from '../src/project-script-settings.js';
const dir = await mkdtemp(join(tmpdir(), 'hive-script-settings-'));
let runtime: Awaited<ReturnType<typeof createDesktopRuntime>> | undefined;
let db1: DatabaseSync | undefined, db2: DatabaseSync | undefined;
try {
  const project1 = join(dir, 'one'), project2 = join(dir, 'two'), data = join(dir, 'data');
  await mkdir(project1); await mkdir(project2);
  runtime = await createDesktopRuntime(data, () => {});
  const ws1 = await runtime.hive.request('POST', '/api/workspaces', {path: project1, name: 'One', autostart_orchestrator: false}) as {id:string};
  const ws2 = await runtime.hive.request('POST', '/api/workspaces', {path: project2, name: 'Two', autostart_orchestrator: false}) as {id:string};
  db1 = new DatabaseSync(join(data, 'hive', 'runtime.sqlite'));
  db2 = new DatabaseSync(join(data, 'hive', 'runtime.sqlite'));
  const port = (db: DatabaseSync): ProjectScriptStatePort => ({
    read(key) { const row = db.prepare('SELECT value FROM app_state WHERE key = ?').get(key) as {value: string | null} | undefined; return row?.value ?? null; },
    compareAndSwap(key, expected, replacement) {
      if (expected === null && replacement === null) throw new Error('unused');
      if (expected === null) return Number(db.prepare('INSERT OR IGNORE INTO app_state (key,value,updated_at) VALUES (?,?,?)').run(key,replacement,Date.now()).changes) === 1;
      if (replacement === null) return Number(db.prepare('DELETE FROM app_state WHERE key=? AND value=?').run(key,expected).changes) === 1;
      return Number(db.prepare('UPDATE app_state SET value=?,updated_at=? WHERE key=? AND value=?').run(replacement,Date.now(),key,expected).changes) === 1;
    },
  });
  const known = new Set([ws1.id, ws2.id]);
  const validate = (id: string) => { if (!known.has(id)) throw new Error('Workspace not found'); };
  const one = new ProjectScriptSettings(port(db1), validate), two = new ProjectScriptSettings(port(db2), validate);
  const initial = await one.list(ws1.id); assert.deepEqual(initial, {version:2,revision:'0',scripts:[]});
  const marker = join(dir, 'must-not-exist');
  const secretCommand = `echo SENSITIVE_TEST_COMMAND > '${marker}'\nprintf done\n`;
  const saved = await one.save(ws1.id, initial.revision, {name:'Local check',command:secretCommand});
  const script = saved.scripts[0]!; assert.equal(script.command, secretCommand);
  assert.deepEqual(await two.list(ws1.id), saved);
  assert.deepEqual(await one.list(ws2.id), initial);
  const raced = await Promise.allSettled([
    one.save(ws2.id, '0', {name:'A', command:'echo A'}),
    two.save(ws2.id, '0', {name:'B', command:'echo B'}),
  ]);
  assert.equal(raced.filter(item => item.status === 'fulfilled').length, 1);
  const rejected = raced.find(item => item.status === 'rejected');
  assert.equal(rejected?.status === 'rejected' && rejected.reason.code, 'CONFLICT');
  const secondProject = await one.list(ws2.id);
  const firstRole = await one.save(ws2.id, secondProject.revision, {id: secondProject.scripts[0]!.id, name:'First setup', command:'true', runOnWorktreeCreate:true});
  const roleVersion = firstRole.scripts[0]!.version;
  const switchedRole = await two.save(ws2.id, firstRole.revision, {name:'Second setup', command:'true', runOnWorktreeCreate:true});
  assert.equal(switchedRole.scripts.filter(item => item.runOnWorktreeCreate).length, 1);
  assert.equal(switchedRole.scripts[0]!.runOnWorktreeCreate, false);
  assert.notEqual(switchedRole.scripts[0]!.version, roleVersion, 'revoking setup also updates the old script CAS version');
  await assert.rejects(one.capture(ws2.id, switchedRole.scripts[0]!.id, roleVersion), (e:any)=>e.code==='CONFLICT');
  await assert.rejects(two.capture(ws2.id, script.id, script.version), (e:any)=>e.code==='NOT_FOUND');
  await assert.rejects(two.save(ws1.id, initial.revision, {name:'Other',command:'false'}), (e:any)=>e.code==='CONFLICT');
  const captured = await one.capture(ws1.id, script.id, script.version);
  const changed = await two.save(ws1.id, saved.revision, {id:script.id,name:'Revised',command:'printf changed'});
  assert.equal(captured.command, secretCommand); assert.notEqual(changed.scripts[0]!.version, script.version);
  await assert.rejects(one.capture(ws1.id, script.id, script.version), (e:any)=>e.code==='CONFLICT');
  for (const input of [{name:'',command:'ok'}, {name:'bad\nname',command:'ok'}, {name:'ok',command:'bad\0command'}, {name:'ok',command:'x'.repeat(16385)}]) {
    await assert.rejects(one.save(ws1.id, changed.revision, input), (e:any)=>e.code==='INVALID' && !e.message.includes('SENSITIVE_TEST_COMMAND'));
  }
  const rows = db1.prepare("SELECT key,value FROM app_state WHERE key LIKE 'desktop_project_scripts_v1_%'").all() as {key:string,value:string}[];
  assert.equal(rows.length, 2);
  const row = rows.find(row => JSON.parse(row.value).workspaceId === ws1.id)!; db2.prepare('UPDATE app_state SET value=? WHERE key=?').run('{broken',row.key);
  await assert.rejects(one.list(ws1.id), (e:any)=>e.code==='CORRUPT' && !e.message.includes('SENSITIVE_TEST_COMMAND'));
  await assert.rejects(one.save(ws1.id, changed.revision, {name:'x',command:'x'}), (e:any)=>e.code==='CORRUPT');
  assert.equal((db1.prepare('SELECT value FROM app_state WHERE key=?').get(row.key) as {value:string}).value, '{broken');
  for (const invalid of [JSON.stringify({...JSON.parse(row.value), version: 3}), JSON.stringify({...JSON.parse(row.value), futureField: true}), JSON.stringify({...JSON.parse(row.value), scripts: [{...JSON.parse(row.value).scripts[0], command: 42}]})]) {
    db2.prepare('UPDATE app_state SET value=? WHERE key=?').run(invalid, row.key);
    await assert.rejects(one.remove(ws1.id, changed.revision, script.id), (e:any)=>e.code==='CORRUPT');
    assert.equal((db1.prepare('SELECT value FROM app_state WHERE key=?').get(row.key) as {value:string}).value, invalid);
  }
  // Legacy rows retain every saved command but never opt in to setup implicitly.
  const legacy = {...JSON.parse(row.value), version: 1};
  db2.prepare('UPDATE app_state SET value=? WHERE key=?').run(JSON.stringify(legacy), row.key);
  assert.equal((await one.list(ws1.id)).version, 2);
  assert.equal((await one.list(ws1.id)).scripts.some(script => script.runOnWorktreeCreate), false);
  db2.prepare('UPDATE app_state SET value=? WHERE key=?').run(row.value,row.key);
  db1.close(); db1=undefined; db2.close(); db2=undefined;
  await runtime.close(); runtime=undefined;
  runtime=await createDesktopRuntime(data,()=>{});
  db1=new DatabaseSync(join(data,'hive','runtime.sqlite'));
  const cold = new ProjectScriptSettings(port(db1), validate);
  assert.deepEqual(await cold.list(ws1.id), changed);
  const empty = await cold.remove(ws1.id, changed.revision, script.id);
  assert.deepEqual(empty.scripts, []); await cold.clearWorkspace(ws1.id, empty.revision);
  assert.deepEqual(await cold.list(ws1.id), initial);
  await assert.rejects(readFile(marker), (e:any)=>e.code==='ENOENT');
  console.log('PROJECT_SCRIPT_SETTINGS_OK: real Hive SQLite; two test connections, cold restore, strict CAS and validation; no scripts executed.');
} finally { db1?.close(); db2?.close(); await runtime?.close(); await rm(dir,{recursive:true,force:true}); }

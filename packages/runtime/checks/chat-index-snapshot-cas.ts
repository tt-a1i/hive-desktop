/** Snapshot identity is independent of object allocation and wall-clock timestamps. */
import assert from 'node:assert/strict';
import {mkdtempSync,mkdirSync,renameSync,rmSync} from 'node:fs';
import {join} from 'node:path';
import {tmpdir} from 'node:os';
import {ChatIndex,type ChatRecord} from '../src/chat-index.js';
const directory=mkdtempSync(join(tmpdir(),'hive-chat-index-cas-'));
try{
 const index=new ChatIndex(directory);
 const record:ChatRecord={id:'a',backend:'codex',workspaceId:'w',title:'A',sessionFile:null,updatedAt:1,identityState:'new',profile:{id:'default-codex',revision:1},execution:{kind:'worktree',path:'/source',branch:'original'}};
 index.save(record);index.save({...record,id:'b'});
 const before=index.snapshotVersion('a');
 for(const snapshot of [index.get('a'),index.list('w').find(item=>item.id==='a')!,index.all().find(item=>item.id==='a')!]){snapshot.execution!.path='/mutated';snapshot.profile!.revision=9;}
 record.execution!.path='/caller-mutated';
 assert.equal(index.get('a').execution!.path,'/source');assert.equal(index.get('a').profile!.revision,1);assert.equal(index.snapshotVersion('a'),before);
 index.save({...index.get('b'),title:'unrelated'});assert.equal(index.snapshotVersion('a'),before);
 const original=index.get('a');index.save({...original,title:'B'});index.save(original);
 assert.deepEqual(index.get('a'),original);assert.notEqual(index.snapshotVersion('a'),before,'ABA must invalidate admitted snapshot even with unchanged updatedAt');
 const equal=index.snapshotVersion('a');index.save(index.get('a'));assert.notEqual(index.snapshotVersion('a'),equal,'explicit equal-value save is still a mutation');
 const stable=index.snapshotVersion('a'),file=join(directory,'index.json');renameSync(file,file+'.saved');mkdirSync(file);
 assert.throws(()=>index.save({...original,title:'failed publication'}));assert.equal(index.snapshotVersion('a'),stable);assert.deepEqual(index.get('a'),original);
 rmSync(file,{recursive:true});renameSync(file+'.saved',file);assert.deepEqual(new ChatIndex(directory).get('a'),original);
 console.log('CHAT_INDEX_SNAPSHOT_CAS_OK: independent nested snapshots, unrelated writes, ABA, equal saves and failed publication');
}finally{rmSync(directory,{recursive:true,force:true});}

import assert from 'node:assert/strict';
import {mkdtempSync, readFileSync, writeFileSync, rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {ChatIndex} from '../src/chat-index.js';
const root = mkdtempSync(join(tmpdir(), 'hive-session-origin-index-'));
try {
  const base = {backend:'pi',workspaceId:'w',title:'same title',sessionFile:null,updatedAt:1};
  writeFileSync(join(root,'index.json'),JSON.stringify([{...base,id:'old-bound',agentId:'worker'}, {...base,id:'old-unknown'}]));
  const index = new ChatIndex(root);
  assert.equal(index.get('old-bound').originAgentId,'worker'); assert.equal(index.get('old-bound').identityState,'known');
  assert.equal(index.get('old-unknown').identityState,'unknown');
  assert.throws(()=>index.bind('old-unknown','w:orchestrator'),/SESSION_IDENTITY_UNKNOWN/);
  index.save({...base,backend:'pi',id:'new',identityState:'new'});
  index.bind('new','worker');
  assert.equal(index.get('old-bound').agentId,undefined); assert.equal(index.get('old-bound').originAgentId,'worker');
  assert.throws(()=>index.bind('old-bound','w:orchestrator'),/SESSION_IDENTITY_CONFLICT/);
  assert.throws(()=>index.save({...index.get('old-bound'),originAgentId:'replacement'}),/SESSION_IDENTITY_CONFLICT/);
  index.establishOrigin('old-unknown','exact-dispatch-worker');
  assert.throws(()=>index.establishOrigin('old-unknown','other-worker'),/SESSION_IDENTITY_CONFLICT/);
  const cold = new ChatIndex(root);
  assert.equal(cold.get('old-bound').originAgentId,'worker'); assert.equal(cold.get('old-unknown').originAgentId,'exact-dispatch-worker');
  const stored=JSON.parse(readFileSync(join(root,'index.json'),'utf8'));
  assert.equal(stored.find((record:any)=>record.id==='old-unknown').identityState,'known');
  console.log('SESSION_ORIGIN_INDEX_OK: reliable migration, explicit new vs legacy unknown, immutable first identity, exact-source recovery and cold persistence.');
} finally {rmSync(root,{recursive:true,force:true});}

#!/usr/bin/env node
const fs=require('node:fs'), readline=require('node:readline');
const send=x=>process.stdout.write(JSON.stringify(x)+'\n');
const thread={id:'modes-thread',path:null,historyMode:'legacy',turns:[],model:'synthetic',reasoningEffort:'medium'};
const sandboxes={
 'read-only':{type:'readOnly',networkAccess:false},
 'workspace-write':{type:'workspaceWrite',writableRoots:[],networkAccess:false,excludeTmpdirEnvVar:false,excludeSlashTmp:false},
 'danger-full-access':{type:'dangerFullAccess'},
};
const control=()=>fs.existsSync('modes-control.json')?JSON.parse(fs.readFileSync('modes-control.json','utf8')):{};
readline.createInterface({input:process.stdin}).on('line',line=>{
 const q=JSON.parse(line); if(!q.method)return; const answer=result=>send({id:q.id,result});
 if(q.id!==undefined)fs.appendFileSync('modes-wire.jsonl',JSON.stringify(q)+'\n');
 if(q.method==='initialize')answer({});
 else if(q.method==='model/list')answer({data:[{id:'synthetic',model:'synthetic',displayName:'Synthetic',hidden:false}],nextCursor:null});
 else if(q.method==='configRequirements/read')answer({requirements:control().requirements??null});
 else if(q.method==='collaborationMode/list'){const c=control(); if(c.error)send({id:q.id,error:{code:c.error,message:'fixture'}}); else answer({data:[{mode:'plan'},{mode:'default'}]});}
 else if(q.method==='thread/start'||q.method==='thread/resume'){
  const c=control();
  if(c.legacyPath)thread.path='legacy-rollout.jsonl';
  answer({thread,model:'synthetic',modelProvider:'openai',
   approvalPolicy:q.params.approvalPolicy??'on-request',
   sandbox:sandboxes[q.params.sandbox??'read-only'],
   ...(q.method==='thread/resume'&&c.resumeMode?{collaborationMode:{mode:c.resumeMode}}:{}),
   ...c.threadReceipt});
 }
 else if(q.method==='thread/read')answer({thread});
 else if(q.method==='turn/start'){
  const c=control();
  if(c.rejectTurn)return send({id:q.id,error:{code:-32000,message:'fixture rejected turn'}});
  if(c.omitTurn)return answer({});
  const turn={id:'turn-'+thread.turns.length,status:'inProgress',items:[]};thread.turns.push(turn);answer({turn});
  setTimeout(()=>{turn.status='completed';send({method:'turn/completed',params:{threadId:thread.id,turn}})},30);
 }
});

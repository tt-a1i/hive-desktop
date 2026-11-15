#!/usr/bin/env node
const fs=require('node:fs'), readline=require('node:readline');
const send=x=>process.stdout.write(JSON.stringify(x)+'\n');
const thread={id:'modes-thread',path:null,historyMode:'legacy',turns:[],model:'synthetic',reasoningEffort:'medium'};
readline.createInterface({input:process.stdin}).on('line',line=>{
 const q=JSON.parse(line); if(!q.method)return; const answer=result=>send({id:q.id,result});
 if(q.id!==undefined)fs.appendFileSync('modes-wire.jsonl',JSON.stringify(q)+'\n');
 if(q.method==='initialize')answer({});
 else if(q.method==='model/list')answer({data:[{id:'synthetic',model:'synthetic',displayName:'Synthetic',hidden:false}],nextCursor:null});
 else if(q.method==='configRequirements/read'){const c=fs.existsSync('modes-control.json')?JSON.parse(fs.readFileSync('modes-control.json','utf8')):{}; answer({requirements:c.requirements??null});}
 else if(q.method==='collaborationMode/list'){const c=fs.existsSync('modes-control.json')?JSON.parse(fs.readFileSync('modes-control.json','utf8')):{}; if(c.error)send({id:q.id,error:{code:c.error,message:'fixture'}}); else answer({data:[{mode:'plan'},{mode:'default'}]});}
 else if(q.method==='thread/start'||q.method==='thread/resume')answer({thread,model:'synthetic',modelProvider:'openai',approvalPolicy:'on-request',sandbox:{type:'readOnly',networkAccess:false}});
 else if(q.method==='thread/read')answer({thread});
 else if(q.method==='turn/start'){
  const turn={id:'turn-'+thread.turns.length,status:'inProgress',items:[]};thread.turns.push(turn);answer({turn});
  setTimeout(()=>{turn.status='completed';send({method:'turn/completed',params:{threadId:thread.id,turn}})},30);
 }
});

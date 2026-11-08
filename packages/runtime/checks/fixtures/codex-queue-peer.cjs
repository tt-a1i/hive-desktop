// Synthetic app-server for the production Codex adapter. No model or network calls.
const fs=require('node:fs'),crypto=require('node:crypto'),readline=require('node:readline');
const file='queue-peer-thread.json',wire='queue-peer-queries.jsonl';let thread;
try{thread=JSON.parse(fs.readFileSync(file,'utf8'));}catch(e){if(e.code!=='ENOENT')throw e;thread={id:'queue-'+crypto.randomUUID(),path:null,historyMode:'legacy',turns:[]};}
const save=()=>fs.writeFileSync(file,JSON.stringify(thread));const send=x=>process.stdout.write(JSON.stringify(x)+'\n');
const notify=(method,params)=>send({method,params:{threadId:thread.id,...params}});
readline.createInterface({input:process.stdin}).on('line',line=>{
 const q=JSON.parse(line);if(!q.method)return;const answer=result=>send({id:q.id,result});
 if(q.method==='initialize')answer({});
 else if(q.method==='model/list')answer({data:[{id:'terra',model:'gpt-6.1-terra',displayName:'Synthetic Terra metadata',hidden:false}],nextCursor:null});
 else if(q.method==='thread/start'||q.method==='thread/resume'){save();answer({thread,model:'gpt-6.1-terra',modelProvider:'openai'});}
 else if(q.method==='thread/read')answer({thread});
 else if(q.method==='turn/start'){
  const user={id:crypto.randomUUID(),type:'userMessage',content:q.params.input};const turn={id:crypto.randomUUID(),status:'inProgress',items:[user]};thread.turns.push(turn);save();fs.appendFileSync(wire,JSON.stringify({turnId:turn.id,input:q.params.input})+'\n');answer({turn});notify('turn/started',{turn});notify('item/completed',{turnId:turn.id,item:user});
  let control={};try{control=JSON.parse(fs.readFileSync('queue-peer-control.json','utf8'));}catch(e){if(e.code!=='ENOENT')throw e;}
  if(control.complete){const assistant={id:crypto.randomUUID(),type:'agentMessage',text:'Synthetic queued completion'};turn.items.push(assistant);turn.status='completed';save();notify('item/completed',{turnId:turn.id,item:assistant});notify('turn/completed',{turn});}
 }else if(q.method==='turn/interrupt'){const turn=thread.turns.find(t=>t.id===q.params.turnId);if(turn){turn.status='interrupted';save();}answer({});if(turn)notify('turn/completed',{turn});}
});

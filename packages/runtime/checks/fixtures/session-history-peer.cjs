#!/usr/bin/env node
const fs=require('fs'),rl=require('readline');
const stateFile=process.env.HIVE_HISTORY_STATE, log=process.env.HIVE_HISTORY_LOG;
const initial=[{id:'t1',status:'completed',itemsView:'full',items:[{id:'u1',type:'userMessage',content:[{type:'text',text:'first'}]}]},{id:'t2',status:'completed',itemsView:'full',items:[{id:'u2',type:'userMessage',content:[{type:'text',text:'second'}]}]}];
const state=fs.existsSync(stateFile)?JSON.parse(fs.readFileSync(stateFile,'utf8')):{source:{thread:{id:'source',cwd:process.env.HIVE_HISTORY_SOURCE,path:'/native/source',historyMode:'paginated'},turns:initial},next:0};
function save(){fs.writeFileSync(stateFile,JSON.stringify(state));}
rl.createInterface({input:process.stdin}).on('line',line=>{const x=JSON.parse(line);fs.appendFileSync(log,JSON.stringify(x)+'\n');if(!x.id)return;let result;
if(x.method==='initialize')result={userAgent:'codex_cli_rs/0.169.0'};
else if(x.method==='thread/read')result={thread:state[x.params.threadId]?.thread};
else if(x.method==='thread/turns/list'){const turns=state[x.params.threadId].turns,n=x.params.cursor?Number(x.params.cursor):0;result={data:turns.slice(n,n+1),nextCursor:n+1<turns.length?String(n+1):null};}
else if(x.method==='thread/fork'){const id='child'+(++state.next),source=state[x.params.threadId],end=x.params.lastTurnId?source.turns.findIndex(t=>t.id===x.params.lastTurnId)+1:source.turns.length;state[id]={thread:{id,cwd:x.params.cwd,path:'/native/'+id,historyMode:'paginated',forkedFromId:x.params.threadId},turns:source.turns.slice(0,end)};save();if(fs.existsSync(process.env.HIVE_HISTORY_DROP))process.exit(2);result={thread:state[id].thread};}
else if(x.method==='thread/revert'){if(fs.existsSync(process.env.HIVE_HISTORY_DROP_BEFORE_REVERT))process.exit(2);const child=state[x.params.threadId],n=child.turns.findIndex(t=>t.id===x.params.beforeTurnId);child.turns=child.turns.slice(0,n);save();if(fs.existsSync(process.env.HIVE_HISTORY_DROP_REVERT))process.exit(2);result={thread:child.thread};}
else throw Error('unexpected method '+x.method);
console.log(JSON.stringify({id:x.id,result}));});

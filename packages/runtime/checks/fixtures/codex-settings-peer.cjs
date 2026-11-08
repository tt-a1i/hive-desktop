#!/usr/bin/env node
// Synthetic protocol peer: acknowledgement precedes application; identical settings emit no event.
const {createInterface}=require('node:readline');
const {existsSync,readFileSync}=require('node:fs');
const send=value=>process.stdout.write(JSON.stringify(value)+'\n');
let model='native-default',effort='medium';
const thread=()=>({id:'settings-peer',path:null,historyMode:'legacy',turns:[],model,reasoningEffort:effort});
createInterface({input:process.stdin}).on('line',line=>{
 const q=JSON.parse(line);const answer=result=>send({id:q.id,result});
 if(q.method==='initialize')answer({});
 else if(q.method==='model/list')answer({data:[{id:'gpt-6.1-terra',model:'gpt-6.1-terra',displayName:'Synthetic Terra',hidden:false,supportedReasoningEfforts:['low','medium','high'].map(reasoningEffort=>({reasoningEffort,description:reasoningEffort}))}],nextCursor:null});
 else if(q.method==='thread/start')answer({thread:thread(),model,modelProvider:'openai'});
 else if(q.method==='thread/read')answer({thread:thread()});
 else if(q.method==='thread/settings/update'){
  const control=existsSync('settings-control.json')?JSON.parse(readFileSync('settings-control.json','utf8')):{};
  if(control.reject){send({id:q.id,error:{code:-32602,message:'Synthetic rejection'}});return;}
  answer({});
  if(control.exit){setTimeout(()=>process.exit(7),10);return;}
  if((q.params.effort??effort)===effort&&(q.params.model??model)===model)return;
  setTimeout(()=>{model=q.params.model??model;effort=control.clamp??q.params.effort??effort;send({method:'thread/settings/updated',params:{threadId:'unrelated',threadSettings:{model,effort:'low'}}});send({method:'thread/settings/updated',params:{threadId:'settings-peer',threadSettings:{model,effort}}});},40);
 }
});

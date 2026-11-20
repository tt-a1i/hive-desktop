// Offline read-only app-server: any unexpected operation fails closed and is logged.
const fs=require('node:fs'),readline=require('node:readline');
const turns=[
 {id:'t1',status:'completed',itemsView:'full',items:[{id:'u1',type:'userMessage',content:[{type:'text',text:'first'}]}]},
 {id:'t2',status:'completed',itemsView:'full',items:[{id:'u2',type:'userMessage',content:[{type:'text',text:'second'}]}]},
];
const thread={id:'source',cwd:process.env.HIVE_HISTORY_SOURCE,path:'/controlled/source',historyMode:'paginated'};
readline.createInterface({input:process.stdin}).on('line',line=>{
 const x=JSON.parse(line);fs.appendFileSync(process.env.HIVE_HISTORY_LOG,JSON.stringify({...x,cwd:process.cwd()})+'\n');
 if(!x.id)return;
 let result;
 if(x.method==='initialize')result={userAgent:'hive_desktop/0.169.0 (Windows; x86_64) dumb (hive_desktop; 0.1.0)'};
 else if(x.method==='model/list')result={data:[{id:'offline',model:'offline',displayName:'Offline fixture',supportedReasoningEfforts:[],defaultReasoningEffort:'none'}],nextCursor:null};
 else if(x.method==='configRequirements/read')result={requirements:null};
 else if(x.method==='collaborationMode/list')result={data:[{mode:'default'}]};
 else if(['thread/read','thread/resume'].includes(x.method)&&x.params.threadId==='source')result={thread,model:'offline',modelProvider:'fixture'};
 else if(x.method==='thread/turns/list'&&x.params.threadId==='source'){
  const n=x.params.cursor?Number(x.params.cursor):0;result={data:turns.slice(n,n+1),nextCursor:n+1<turns.length?String(n+1):null};
 }else throw Error('Unexpected controlled history operation '+x.method);
 console.log(JSON.stringify({id:x.id,result}));
});

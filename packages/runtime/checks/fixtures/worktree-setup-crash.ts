import {createDesktopRuntime} from '../../src/desktop-runtime.js';
const [data,project]=process.argv.slice(2);if(!data||!project)throw new Error('Fixture paths required');
const rt=await createDesktopRuntime(data,()=>{}, {executableEnvironment:{}});
const ws=await rt.hive.request('POST','/api/workspaces',{path:project,name:'Crash setup',autostart_orchestrator:false}) as {id:string};
await rt.projectScripts.save(ws.id,'0',{name:'Crash',command:'printf "NATIVE_PID=%s\\n" "$$"; printf once >> count; sleep 60; printf replayed >> count',runOnWorktreeCreate:true});
const session=await rt.chats.createRecord(ws.id,'codex',{isolation:'worktree'});
for(;;){const status=await rt.initialization.get(ws.id,session.id);if(status?.runId){process.send?.({workspaceId:ws.id,sessionId:session.id,path:session.execution!.path,runId:status.runId});break;}await new Promise(done=>setTimeout(done,10));}
setInterval(()=>{},1000);

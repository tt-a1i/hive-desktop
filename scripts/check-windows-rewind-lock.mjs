import { runWindowsNativeFixture } from './lib/windows-native-fixture.mjs';
await runWindowsNativeFixture({
  name:'rewind-lock', fixtureFile:'rewind-lock', seed:'windows-rewind-lock-seed.mjs', ui:'windows-rewind-lock-ui.ps1',
  isolatedHome:true, failureMode:'inspect', verifyMode:phase=>phase==='select'?'before':'after',
  uiArguments:fixture=>['-CheckpointId',fixture.checkpointId,'-Draft',fixture.draft],
  forbiddenRpc:['history.fork.apply','history.operations.recover','chat.prompt','chat.resume'],
  summary:fixture=>({workspaceId:fixture.workspaceId,sessionId:fixture.sessionId,checkpointId:fixture.checkpointId}),
});

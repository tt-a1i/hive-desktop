import { runWindowsNativeFixture } from './lib/windows-native-fixture.mjs';
await runWindowsNativeFixture({
  name: 'question', fixtureFile: 'question', seed: 'windows-question-seed.mjs', ui: 'windows-question-ui.ps1',
  isolatedHome: true, uiArguments: f => ['-Source', f.source, '-SessionId', f.sessionId, '-WorkspaceId', f.workspaceId], verifyMode: phase => phase,
  summary: f => ({ workspaceId: f.workspaceId, sessionId: f.sessionId, sha256: f.sha256 }),
});

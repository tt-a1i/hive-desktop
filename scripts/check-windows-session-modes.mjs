import { runWindowsNativeFixture } from './lib/windows-native-fixture.mjs';

await runWindowsNativeFixture({
  name: 'session-modes',
  fixtureFile: 'session-modes',
  seed: 'windows-session-modes-seed.mjs',
  ui: 'windows-session-modes-ui.ps1',
  isolatedHome: true,
  verifyMode: phase => phase,
  uiArguments: () => [],
  forbiddenRpc: ['chat.prompt', 'chat.resume', 'chat.steer'],
  summary: ({ workspaceId, sessions }) => ({ workspaceId, sessions }),
});

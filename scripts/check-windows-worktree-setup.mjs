import { runWindowsNativeFixture } from './lib/windows-native-fixture.mjs';

await runWindowsNativeFixture({
  name: 'worktree-setup',
  fixtureFile: 'worktree-setup',
  seed: 'windows-worktree-setup-seed.mjs',
  ui: 'windows-worktree-setup-ui.ps1',
  phases: ['create', 'retry', 'reopen'],
  uiArguments: fixture => ['-FixtureRoot', fixture.root, '-Draft', fixture.draft],
  verifyMode: phase => phase,
  failureMode: 'inspect',
  forbiddenRpc: ['chat.prompt', 'chat.resume', 'worktree.initialization.skip'],
  summary: fixture => ({ workspaceId: fixture.workspaceId, scriptVersion: fixture.script.version, wire: fixture.wire }),
});

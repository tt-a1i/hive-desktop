import { runWindowsNativeFixture } from './lib/windows-native-fixture.mjs';

await runWindowsNativeFixture({
  name: 'worktree-dirty', fixtureFile: 'worktree-dirty',
  seed: 'windows-worktree-dirty-seed.mjs', ui: 'windows-worktree-dirty-ui.ps1',
  uiArguments: fixture => ['-FirstWorktree', fixture.first.path, '-SecondWorktree', fixture.second.path],
  verifyMode: phase => phase,
  failureMode: 'failure',
  forbiddenRpc: ['chat.prompt', 'chat.activate', 'chat.worktrees.removal.apply'],
  summary: fixture => ({ workspaceId: fixture.workspaceId, first: fixture.first, second: fixture.second }),
});

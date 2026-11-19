import { runWindowsNativeFixture } from './lib/windows-native-fixture.mjs';
await runWindowsNativeFixture({
  name: 'worktree-remove', fixtureFile: 'worktree-remove',
  seed: 'windows-worktree-remove-seed.mjs', ui: 'windows-worktree-remove-ui.ps1',
  uiArguments: f => ['-Worktree', f.clean.path, '-Branch', f.clean.branch, '-Head', f.clean.head],
  verifyMode: phase => phase, failureMode: 'failure',
  forbiddenRpc: ['chat.prompt', 'chat.activate'],
  summary: f => ({ workspaceId: f.workspaceId, clean: f.clean, other: f.other }),
});

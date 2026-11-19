import { runWindowsNativeFixture } from './lib/windows-native-fixture.mjs';
await runWindowsNativeFixture({
  name: 'project-reference', fixtureFile: 'reference',
  seed: 'windows-reference-seed.mjs', ui: 'windows-reference-ui.ps1',
  uiArguments: fixture => ['-Sha256', fixture.sha256, '-Worktree', fixture.worktree],
  verifyMode: phase => phase === 'select' ? 'before' : 'after',
  summary: fixture => ({sessionId: fixture.sessionId, sha256: fixture.sha256, relativePath: fixture.relativePath}),
});

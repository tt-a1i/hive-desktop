import { runWindowsNativeFixture } from './lib/windows-native-fixture.mjs';
await runWindowsNativeFixture({
  name: 'git-preview', fixtureFile: 'git-preview', isolatedHome: true, forbiddenRpc: ['git.commit.apply','git.index.apply'],
  seed: 'windows-git-preview-seed.mjs', ui: 'windows-git-preview-ui.ps1',
  uiArguments: fixture => ['-Draft', fixture.draft],
  verifyMode: phase => phase === 'select' ? 'before' : 'after',
  failureMode: 'failure',
  summary: fixture => ({sessionId: fixture.sessionId, stagedPath: fixture.stagedPath, head: fixture.snapshot.head, indexSha256: fixture.snapshot.indexSha256}),
});

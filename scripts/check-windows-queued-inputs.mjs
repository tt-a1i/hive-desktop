import { runWindowsNativeFixture } from './lib/windows-native-fixture.mjs';
await runWindowsNativeFixture({
  name: 'queued-inputs', fixtureFile: 'queue',
  seed: 'windows-queue-seed.mjs', ui: 'windows-queue-ui.ps1',
  uiArguments: fixture => ['-OriginalText', fixture.originalText, '-EditedText', fixture.editedText,
    '-CanceledText', fixture.canceledText, '-Draft', fixture.draft],
  verifyMode: phase => phase === 'select' ? 'before' : 'after',
  failureMode: 'inspect',
  isolatedHome: true,
  summary: fixture => ({sessionId: fixture.sessionId, deliveryId: fixture.deliveryId,
    canceledId: fixture.canceledId, attachmentId: fixture.attachmentId, attachmentSha256: fixture.attachmentSha256}),
});

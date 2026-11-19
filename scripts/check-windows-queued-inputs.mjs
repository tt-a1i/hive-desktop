import { runWindowsNativeFixture } from './lib/windows-native-fixture.mjs';
const stopRejection = process.argv.includes('--stop-rejection');
await runWindowsNativeFixture({
  name: stopRejection ? 'queued-stop-rejection' : 'queued-inputs', fixtureFile: 'queue',
  seedMode: stopRejection ? 'stop-seed' : 'seed',
  seed: 'windows-queue-seed.mjs', ui: 'windows-queue-ui.ps1',
  uiArguments: fixture => ['-OriginalText', fixture.originalText, '-EditedText', fixture.editedText,
    '-CanceledText', fixture.canceledText, '-Draft', fixture.draft,
    ...(stopRejection ? ['-StopRejection', '-SpoolPath', fixture.spoolPath,
      '-RejectionPath', fixture.rejectionPath, '-SnapshotPath', fixture.snapshotPath] : [])],
  verifyMode: phase => phase === 'select' ? 'before' : 'after',
  failureMode: 'inspect',
  isolatedHome: true,
  summary: fixture => ({sessionId: fixture.sessionId, ...(fixture.stopRejection
    ? {primaryId:fixture.primaryId, primaryAttachmentId:fixture.primaryAttachmentId,
      primaryAttachmentSha256:fixture.primaryAttachmentSha256}
    : {deliveryId:fixture.deliveryId}),
    canceledId: fixture.canceledId, attachmentId: fixture.attachmentId, attachmentSha256: fixture.attachmentSha256}),
});

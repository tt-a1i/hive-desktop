import { runWindowsNativeFixture } from './lib/windows-native-fixture.mjs';
const legacyRetry = process.argv.includes('--legacy-retry');
if (legacyRetry && ['--definite-rejection', '--unknown-recovery', '--stop-rejection'].some(flag => process.argv.includes(flag))) {
  throw Error('The legacy retry replay is a separate queue scenario');
}
const definiteRejection = process.argv.includes('--definite-rejection');
const unknownRecovery = process.argv.includes('--unknown-recovery');
const stopRejection = unknownRecovery || definiteRejection || process.argv.includes('--stop-rejection');
await runWindowsNativeFixture(legacyRetry ? {
  name: 'queued-legacy-retry', fixtureFile: 'queue', phases: ['select','resolve','reopen'],
  seed: 'windows-queue-legacy-seed.mjs', ui: 'windows-queue-ui.ps1',
  uiArguments: fixture => ['-LegacyRetry', '-OriginalText', fixture.originalText, '-Draft', fixture.draft,
    '-SpoolPath', fixture.spoolPath, '-LegacyAttachmentName', fixture.attachmentName,
    '-LegacyDeliveryId', fixture.deliveryId,
    '-LegacyNativePath', fixture.nativePath, '-LegacyNativeHash', fixture.nativeSha256,
    '-LegacyQueriesPath', fixture.queriesPath],
  verifyMode: phase => phase === 'select' ? 'before' : phase === 'resolve' ? 'resolved' : 'after',
  failureMode: 'inspect', isolatedHome: true,
  forbiddenRpc: ['chat.queue.discardUncertainDelivery'],
  summary: fixture => ({sessionId: fixture.sessionId, originalSpoolFormat: fixture.originalSpoolFormat,
    nativeThreadId: fixture.nativeBefore.id, deliveryId: fixture.deliveryId, attachmentId: fixture.attachmentId,
    attachmentSha256: fixture.attachmentSha256}),
} : {
  name: unknownRecovery ? 'queued-stop-unknown' : definiteRejection ? 'queued-stop-declined' : stopRejection ? 'queued-stop-rejection' : 'queued-inputs', fixtureFile: 'queue',
  phases: unknownRecovery ? ['select','resolve','reopen'] : undefined,
  seedMode: unknownRecovery ? 'unknown-seed' : definiteRejection ? 'declined-seed' : stopRejection ? 'stop-seed' : 'seed',
  seed: 'windows-queue-seed.mjs', ui: 'windows-queue-ui.ps1',
  uiArguments: fixture => ['-OriginalText', fixture.originalText, '-EditedText', fixture.editedText,
    '-CanceledText', fixture.canceledText, '-Draft', fixture.draft,
    ...(stopRejection ? ['-StopRejection', '-SpoolPath', fixture.spoolPath,
      '-RejectionPath', fixture.rejectionPath, '-SnapshotPath', fixture.snapshotPath] : []),
    ...(definiteRejection ? ['-DefiniteRejection'] : []),...(unknownRecovery ? ['-UnknownRecovery'] : [])],
  verifyMode: phase => phase === 'select' ? 'before' : phase === 'resolve' ? 'resolved' : 'after',
  failureMode: 'inspect',
  isolatedHome: true,
  summary: fixture => ({sessionId: fixture.sessionId, ...(fixture.stopRejection
    ? {primaryId:fixture.primaryId, primaryAttachmentId:fixture.primaryAttachmentId,
      primaryAttachmentSha256:fixture.primaryAttachmentSha256}
    : {deliveryId:fixture.deliveryId}),
    canceledId: fixture.canceledId, attachmentId: fixture.attachmentId, attachmentSha256: fixture.attachmentSha256}),
});

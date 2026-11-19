import { createHash, randomUUID } from 'node:crypto';
import { mkdirSync, readFileSync, renameSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { isDeliveryPresentation } from '../delivery-presentation.js';
import { SessionIdentityError } from '../chat-index.js';
import type { CodexMetadata, CodexTurn } from './codex-history.js';
import { CodexProjection } from './codex-projection.js';

export function codexMetadataFile(directory: string, threadId: string): string {
  return join(directory, `codex-${createHash('sha256').update(threadId).digest('hex')}.json`);
}
export function readCodexMetadata(directory: string, threadId: string): CodexMetadata {
  let value: CodexMetadata;
  try { value = JSON.parse(readFileSync(codexMetadataFile(directory, threadId), 'utf8')); }
  catch (error) {
    if ((error as NodeJS.ErrnoException).code === 'ENOENT') {
      try {
        statSync(`${codexMetadataFile(directory, threadId)}.required`);
      } catch (markerError) {
        if ((markerError as NodeJS.ErrnoException).code === 'ENOENT')
          return { version: 1, context: [], deliveries: [], completed: [] };
        throw markerError;
      }
      throw new SessionIdentityError('CODEX_DELIVERY_METADATA_MISSING',
        'Saved Hive delivery metadata is missing. Restore the original metadata file to recover message identities and attachments.');
    }
    throw error;
  }
  if (value.version !== 1 || !Array.isArray(value.context) || !Array.isArray(value.deliveries) || !Array.isArray(value.completed)
    || !value.context.every(entry => typeof entry.text === 'string' && typeof entry.customType === 'string')
    || !value.deliveries.every(entry => typeof entry.turnId === 'string' && (entry.itemId === undefined || typeof entry.itemId === 'string') && typeof entry.deliveryId === 'string' && isDeliveryPresentation(entry.presentation))
    || !value.completed.every(id => typeof id === 'string')
    || (value.inheritedSources !== undefined && (!Array.isArray(value.inheritedSources) || !value.inheritedSources.every(entry => typeof entry.itemId === 'string' && !!entry.itemId && typeof entry.sourceSessionId === 'string' && !!entry.sourceSessionId)))
    || (value.inheritedDeliveries !== undefined && (!Array.isArray(value.inheritedDeliveries) || !value.inheritedDeliveries.every(entry => typeof entry.turnId === 'string' && typeof entry.itemId === 'string' && !!entry.itemId && typeof entry.deliveryId === 'string' && isDeliveryPresentation(entry.presentation))))) throw new Error('Invalid Hive Codex metadata');
  return value;
}
export function writeCodexMetadata(directory: string, threadId: string, metadata: CodexMetadata): void {
  mkdirSync(directory, { recursive: true, mode: 0o700 });
  const target = codexMetadataFile(directory, threadId);
  const temporary = `${target}.${randomUUID()}.tmp`;
  try {
    writeFileSync(temporary, JSON.stringify(metadata), { encoding: 'utf8', mode: 0o600, flush: true });
    renameSync(temporary, target);
    // Only mark a successfully published Hive delivery. A failed first write
    // must remain retryable without inventing a missing historical file.
    if (metadata.deliveries.length || metadata.inheritedDeliveries?.length || metadata.inheritedSources?.length) {
      try { writeFileSync(`${target}.required`, '', { mode: 0o600, flag: 'wx', flush: true }); }
      catch (error) {
        // Metadata is already durable. Do not fail an accepted native delivery
        // and risk replay just because the independent guard could not be made.
        // A later metadata write will retry the guard.
        if ((error as NodeJS.ErrnoException).code !== 'EEXIST')
          console.error('[hive-desktop] Could not persist Codex delivery metadata guard:', error);
      }
    }
  } finally {
    rmSync(temporary, { force: true });
  }
}

/** Publish only display history for a native fork. No execution state is inherited. */
export function createForkCodexMetadata(directory: string, sourceThreadId: string, childThreadId: string, sourceSessionId: string, turns: CodexTurn[]): void {
  const source = readCodexMetadata(directory, sourceThreadId);
  const inheritedSources: NonNullable<CodexMetadata['inheritedSources']> = [];
  const inheritedDeliveries: NonNullable<CodexMetadata['inheritedDeliveries']> = [];
  const sources = new Map(source.inheritedSources?.map(entry => [entry.itemId, entry.sourceSessionId]));
  for (const turn of turns) for (const item of turn.items) {
    inheritedSources.push({ itemId: item.id, sourceSessionId: sources.get(item.id) ?? sourceSessionId });
    if (item.type !== 'userMessage') continue;
    const candidates = [...(source.inheritedDeliveries ?? []), ...source.deliveries];
    const delivery = candidates.find(entry => entry.turnId === turn.id && entry.itemId === item.id)
      ?? (turn.items.filter(candidate => candidate.type === 'userMessage').length === 1
        ? candidates.find(entry => entry.turnId === turn.id && entry.itemId === undefined) : undefined);
    if (delivery) inheritedDeliveries.push({ ...delivery, itemId: item.id });
  }
  writeCodexMetadata(directory, childThreadId, { version: 1, context: [], deliveries: [], completed: [], inheritedSources, inheritedDeliveries });
  new CodexProjection(directory, childThreadId).recover(turns, true);
}

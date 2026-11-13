import { createHash, randomUUID } from 'node:crypto';
import { mkdirSync, readFileSync, renameSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { isDeliveryPresentation } from '../delivery-presentation.js';
import type { CodexMetadata } from './codex-history.js';

export function codexMetadataFile(directory: string, threadId: string): string {
  return join(directory, `codex-${createHash('sha256').update(threadId).digest('hex')}.json`);
}
export function readCodexMetadata(directory: string, threadId: string): CodexMetadata {
  let value: CodexMetadata;
  try { value = JSON.parse(readFileSync(codexMetadataFile(directory, threadId), 'utf8')); }
  catch (error) {
    if ((error as NodeJS.ErrnoException).code === 'ENOENT') return { version: 1, context: [], deliveries: [], completed: [] };
    throw error;
  }
  if (value.version !== 1 || !Array.isArray(value.context) || !Array.isArray(value.deliveries) || !Array.isArray(value.completed)
    || !value.context.every(entry => typeof entry.text === 'string' && typeof entry.customType === 'string')
    || !value.deliveries.every(entry => typeof entry.turnId === 'string' && (entry.itemId === undefined || typeof entry.itemId === 'string') && typeof entry.deliveryId === 'string' && isDeliveryPresentation(entry.presentation))
    || !value.completed.every(id => typeof id === 'string')) throw new Error('Invalid Hive Codex metadata');
  return value;
}
export function writeCodexMetadata(directory: string, threadId: string, metadata: CodexMetadata): void {
  mkdirSync(directory, { recursive: true, mode: 0o700 });
  const target = codexMetadataFile(directory, threadId);
  const temporary = `${target}.${randomUUID()}.tmp`;
  try {
    writeFileSync(temporary, JSON.stringify(metadata), { encoding: 'utf8', mode: 0o600, flush: true });
    renameSync(temporary, target);
  } finally {
    rmSync(temporary, { force: true });
  }
}

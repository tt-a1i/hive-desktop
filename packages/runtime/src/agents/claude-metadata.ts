import { createHash, randomUUID } from 'node:crypto';
import { mkdirSync, readFileSync, renameSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { isDeliveryPresentation, type DeliveryPresentation } from '../delivery-presentation.js';
export type ClaudeMetadata = { version: 1; context: { uuid: string; customType: string; text: string }[];
  deliveries: { uuid: string; deliveryId: string; presentation: DeliveryPresentation }[];
  completed: { deliveryId: string; userUuid: string; assistantUuid: string; resultUuid: string }[] };
export function claudeMetadataPath(directory: string, sessionId: string): string {
  return join(directory, `claude-${createHash('sha256').update(sessionId).digest('hex')}.json`);
}
export function readClaudeMetadata(directory: string, sessionId: string): ClaudeMetadata {
  let value: ClaudeMetadata;
  try { value = JSON.parse(readFileSync(claudeMetadataPath(directory, sessionId), 'utf8')); }
  catch (error) { if ((error as NodeJS.ErrnoException).code === 'ENOENT') return { version: 1, context: [], deliveries: [], completed: [] }; throw error; }
  if (value.version !== 1 || !Array.isArray(value.context) || !Array.isArray(value.deliveries) || !Array.isArray(value.completed)
    || !value.context.every(entry => typeof entry.uuid === 'string' && typeof entry.customType === 'string' && typeof entry.text === 'string')
    || !value.deliveries.every(entry => typeof entry.uuid === 'string' && typeof entry.deliveryId === 'string' && isDeliveryPresentation(entry.presentation))
    || !value.completed.every(entry => typeof entry.deliveryId === 'string' && typeof entry.userUuid === 'string' && typeof entry.assistantUuid === 'string' && typeof entry.resultUuid === 'string')) throw new Error('Invalid Hive Claude presentation metadata');
  return value;
}
export function writeClaudeMetadata(directory: string, sessionId: string, metadata: ClaudeMetadata): void {
  mkdirSync(directory, { recursive: true, mode: 0o700 });
  const target = claudeMetadataPath(directory, sessionId);
  const temporary = `${target}.${randomUUID()}.tmp`;
  writeFileSync(temporary, JSON.stringify(metadata), { mode: 0o600 }); renameSync(temporary, target);
}

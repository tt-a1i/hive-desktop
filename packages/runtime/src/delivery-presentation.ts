import type { ChatAttachment } from './image-attachments.js';
export interface DeliveryPresentation {
  version: 1;
  origin: 'user' | 'dispatch' | 'report' | 'cancel' | 'recovery' | 'system' | 'unknown';
  displayText: string;
  dispatchId?: string;
  workerName?: string;
  workerId?: string;
  artifacts?: string[];
  attachments?: ChatAttachment[];
  sourceText?: string;
  /** Exact authoritative Hive messages.sequence; absent for direct worker input. */
  messageSequence?: number;
}

/** Only explicit, versioned metadata is a presentation contract. */
export function isDeliveryPresentation(value: unknown): value is DeliveryPresentation {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return false;
  const record = value as Record<string, unknown>;
  return record.version === 1
    && ['user', 'dispatch', 'report', 'cancel', 'recovery', 'system', 'unknown'].includes(record.origin as string)
    && typeof record.displayText === 'string'
    && (record.dispatchId === undefined || typeof record.dispatchId === 'string')
    && (record.workerName === undefined || typeof record.workerName === 'string')
    && (record.workerId === undefined || typeof record.workerId === 'string')
    && (record.messageSequence === undefined || (Number.isSafeInteger(record.messageSequence) && (record.messageSequence as number) > 0))
    && (record.sourceText === undefined || typeof record.sourceText === 'string')
    && (record.attachments === undefined || (Array.isArray(record.attachments) && record.attachments.every(item=>item && typeof item.id==='string' && typeof item.workspaceId==='string' && typeof item.name==='string' && typeof item.mimeType==='string' && typeof item.size==='number')))
    && (record.artifacts === undefined || (Array.isArray(record.artifacts) && record.artifacts.every(item => typeof item === 'string')));
}

/** Early clients saved Hive reports as system notices. The envelope only supplies
 * a lookup key. The Hive outbox must match the complete payload and recipient. */
export function restoreHistoricalReport(message: unknown, lookup: (dispatchId: string, payload: string) => unknown): unknown {
  if (!message || typeof message !== 'object' || !('role' in message) || message.role !== 'custom'
    || !('customType' in message) || message.customType !== 'hive-delivery' || !('presentation' in message)) return message;
  const meta = message.presentation;
  if (!isDeliveryPresentation(meta) || meta.origin !== 'system' || !('deliveryId' in meta)
    || typeof meta.deliveryId !== 'string' || !meta.deliveryId) return message;
  const dispatchId = /^<hive-message kind="report" from="[^"]*" dispatch="([^"]+)">/.exec(meta.displayText)?.[1];
  if (!dispatchId || !('content' in message)) return message;
  const content = message.content;
  const raw = Array.isArray(content) && content.length === 1 && content[0]?.type === 'text' ? content[0].text : content;
  if (raw !== meta.displayText) return message;
  const report = lookup(dispatchId, meta.displayText);
  if (!isDeliveryPresentation(report) || report.origin !== 'report') return message;
  return { ...message, presentation: { ...report, deliveryId: meta.deliveryId } };
}

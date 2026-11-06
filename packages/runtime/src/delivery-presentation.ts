export interface DeliveryPresentation {
  version: 1;
  origin: 'user' | 'dispatch' | 'report' | 'cancel' | 'recovery' | 'system' | 'unknown';
  displayText: string;
  dispatchId?: string;
  workerName?: string;
}

/** Only explicit, versioned metadata is a presentation contract. */
export function isDeliveryPresentation(value: unknown): value is DeliveryPresentation {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return false;
  const record = value as Record<string, unknown>;
  return record.version === 1
    && ['user', 'dispatch', 'report', 'cancel', 'recovery', 'system', 'unknown'].includes(record.origin as string)
    && typeof record.displayText === 'string'
    && (record.dispatchId === undefined || typeof record.dispatchId === 'string')
    && (record.workerName === undefined || typeof record.workerName === 'string');
}

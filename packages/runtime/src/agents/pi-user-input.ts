import type { SessionEntry } from '@earendil-works/pi-coding-agent';
import { isDeliveryPresentation, type DeliveryPresentation } from '../delivery-presentation.js';

export const PI_USER_INPUT = 'hive-user-input';
export const PI_USER_INPUT_RESULT = 'hive-user-input-result';
export type PiUserInput = DeliveryPresentation & { deliveryId: string; rawText: string };

export function piUserInput(value: unknown): PiUserInput | undefined {
  if (!isDeliveryPresentation(value) || value.origin !== 'user' || !('deliveryId' in value)
    || typeof value.deliveryId !== 'string' || !value.deliveryId || !('rawText' in value) || typeof value.rawText !== 'string') return;
  return value as PiUserInput;
}
/** Markers live in Pi's state-only entries; they never become model context. */
export function piUserInputSources(branch: readonly SessionEntry[]) {
  const users = new Map<string, PiUserInput>();
  const notes = new Map<string, PiUserInput>();
  let pending: { entryId: string; input: PiUserInput } | undefined;
  for (const entry of branch) {
    if (entry.type === 'custom' && entry.customType === PI_USER_INPUT) {
      const input = piUserInput(entry.data);
      pending = input ? { entryId: entry.id, input } : undefined;
      if (input) notes.set(entry.id, input);
    } else if (entry.type === 'custom' && entry.customType === PI_USER_INPUT_RESULT) {
      if (pending && entry.data && typeof entry.data === 'object' && 'deliveryId' in entry.data
        && entry.data.deliveryId === pending.input.deliveryId) pending = undefined;
    } else if (entry.type === 'message' && entry.message.role === 'user' && pending) {
      users.set(entry.id, pending.input);
      notes.delete(pending.entryId);
      pending = undefined;
    } else if (entry.type === 'custom_message' && entry.customType === 'hive-delivery') {
      pending = undefined;
    }
  }
  return { users, notes };
}
export function piUserInputNote(entry: SessionEntry, input: PiUserInput, backend: 'pi' | 'pi-rpc') {
  return { role: 'custom', customType: 'hive-delivery', content: input.rawText, display: false,
    details: input, presentation: input, timestamp: Date.parse(entry.timestamp), applicationEvent: 'native-user-input',
    rawBackendItem: entry, source: { backend, entryId: entry.id } };
}

export function assertPiUserInputHandled(branch: readonly SessionEntry[], deliveryId: string): void {
  const index = branch.findLastIndex(entry => entry.type === 'custom' && entry.customType === PI_USER_INPUT
    && piUserInput(entry.data)?.deliveryId === deliveryId);
  if (index < 0) throw new Error('Native user input is not on this Pi branch');
  const subsequent = branch.slice(index + 1);
  if (subsequent.some(entry => (entry.type === 'message' && entry.message.role === 'user')
    || (entry.type === 'custom' && entry.customType === PI_USER_INPUT)
    || (entry.type === 'custom_message' && entry.customType === 'hive-delivery'))) throw new Error('Native input has a later user message');
  if (!subsequent.some(entry => entry.type === 'custom' && entry.customType === PI_USER_INPUT_RESULT
    && entry.data && typeof entry.data === 'object' && 'deliveryId' in entry.data && entry.data.deliveryId === deliveryId
    && 'disposition' in entry.data && entry.data.disposition === 'handled')) throw new Error('Native input was not handled locally');
}

export function piUserPrompt(input: PiUserInput) {
  if (!input.rawText.startsWith(input.displayText)) throw new TypeError('Native user input does not match its original presentation');
  return { text: input.displayText, context: input.rawText.slice(input.displayText.length) };
}

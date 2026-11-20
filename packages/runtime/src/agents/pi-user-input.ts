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

type PiInputEvent<T> = { type: 'marker'; input: T | undefined } | { type: 'result'; deliveryId: string | undefined }
  | { type: 'user' } | { type: 'delivery' };

export function piInputEvent<T>(entry: SessionEntry, marker: (input: PiUserInput) => T): PiInputEvent<T> | undefined {
  if (entry.type === 'custom' && entry.customType === PI_USER_INPUT) {
    const input = piUserInput(entry.data);
    return {type: 'marker', input: input && marker(input)};
  }
  if (entry.type === 'custom' && entry.customType === PI_USER_INPUT_RESULT) {
    const deliveryId = entry.data && typeof entry.data === 'object' && 'deliveryId' in entry.data ? entry.data.deliveryId : undefined;
    return {type: 'result', deliveryId: typeof deliveryId === 'string' ? deliveryId : undefined};
  }
  if (entry.type === 'message' && entry.message.role === 'user') return {type: 'user'};
  if (entry.type === 'custom_message' && entry.customType === 'hive-delivery') return {type: 'delivery'};
  return;
}

export function collectPiInputSources<T extends {deliveryId: string}>(
  branch: Iterable<{id: string; event: PiInputEvent<T> | undefined}>,
) {
  const users = new Map<string, {entryId: string; input: T}>();
  const notes = new Map<string, T>();
  let pending: {entryId: string; input: T} | undefined;
  for (const {id, event} of branch) {
    if (event?.type === 'marker') {
      pending = event.input ? {entryId: id, input: event.input} : undefined;
      if (event.input) notes.set(id, event.input);
    } else if (event?.type === 'result') {
      if (pending && event.deliveryId === pending.input.deliveryId) pending = undefined;
    } else if (event?.type === 'user' && pending) {
      users.set(id, pending);
      notes.delete(pending.entryId);
      pending = undefined;
    } else if (event?.type === 'delivery') {
      pending = undefined;
    }
  }
  return {users, notes};
}

/** Markers live in Pi's state-only entries; they never become model context. */
export function piUserInputSources(branch: readonly SessionEntry[]) {
  function* events() {
    for (const entry of branch) yield {id: entry.id, event: piInputEvent(entry, input => input)};
  }
  const {users, notes} = collectPiInputSources(events());
  return {users: new Map([...users].map(([id, source]) => [id, source.input])), notes};
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

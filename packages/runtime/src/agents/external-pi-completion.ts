import { PI_USER_INPUT, piUserInput, piUserInputSources } from './pi-user-input.js';
import type { SessionEntry } from '@earendil-works/pi-coding-agent';

/** The caller supplies the terminal assistant entry observed for this settled run. */
export function assertExternalPiCompletion(branch: readonly SessionEntry[], deliveryId: string, assistantEntryId: string): void {
  const deliveryIndex = branch.findLastIndex(entry => (entry.type === 'custom' && entry.customType === PI_USER_INPUT && piUserInput(entry.data)?.deliveryId === deliveryId) || (entry.type === 'custom_message' && entry.customType === 'hive-delivery'
    && !!entry.details && typeof entry.details === 'object' && 'deliveryId' in entry.details && entry.details.deliveryId === deliveryId));
  if (deliveryIndex === -1) throw new Error('Cannot complete a Hive delivery outside the active branch');
  const subsequent = branch.slice(deliveryIndex + 1);
  const sources = piUserInputSources(branch);
  const nativeUserId = [...sources.users].find(([id, input]) => input.deliveryId === deliveryId
    && branch.findIndex(entry => entry.id === id) > deliveryIndex)?.[0];
  if (subsequent.some(entry => (entry.type === 'message' && entry.message.role === 'user' && entry.id !== nativeUserId)
    || (entry.type === 'custom' && entry.customType === PI_USER_INPUT)
    || (entry.type === 'custom_message' && entry.customType === 'hive-delivery'))) {
    throw new Error('Cannot complete a Hive delivery after a later input');
  }
  const last = subsequent.findLast(entry => entry.type === 'message' && entry.message.role === 'assistant');
  if (!last || last.id !== assistantEntryId || last.type !== 'message' || last.message.role !== 'assistant'
    || !['stop', 'length'].includes(last.message.stopReason)) {
    throw new Error('Hive delivery has no matching successful final assistant reply');
  }
}

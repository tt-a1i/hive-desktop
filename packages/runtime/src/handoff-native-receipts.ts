import { closeSync, fstatSync, openSync, readSync, realpathSync } from 'node:fs';
import type { SessionEntry } from '@earendil-works/pi-coding-agent';
import type { ChatRecord } from './chat-index.js';
import type { PiSessionHandle } from './agents/types.js';
import { externalPiBranch } from './agents/external-pi-history.js';
import { PI_USER_INPUT, PI_USER_INPUT_RESULT, piUserInput, piUserInputSources } from './agents/pi-user-input.js';

const maximumBytes = 16 * 1024 * 1024;
const object = (value: unknown): value is Record<string, unknown> => !!value && typeof value === 'object' && !Array.isArray(value);

/** Offline positive proof; malformed/missing/oversize evidence never proves consumption. */
function piConsumed(record: ChatRecord, handle: Pick<PiSessionHandle, 'sessionId' | 'sessionFile'>, deliveryId: string, expectedContext: string): boolean {
 let fd: number | undefined;
 try {
  if (!expectedContext || !handle.sessionFile) return false;
  if (record.sessionFile && realpathSync(record.sessionFile) !== realpathSync(handle.sessionFile)) return false;
  const expectedNative = record.backend === 'pi' ? record.piSessionId : record.externalPiSessionId;
  if (expectedNative && expectedNative !== handle.sessionId) return false;
  fd = openSync(handle.sessionFile, 'r');
  const info = fstatSync(fd);
  if (!info.isFile() || info.size > maximumBytes) return false;
  const bytes = Buffer.alloc(maximumBytes + 1);
  let length = 0;
  while (length < bytes.length) {
   const read = readSync(fd, bytes, length, bytes.length - length, length);
   if (!read) break;
   length += read;
  }
  if (length > maximumBytes) return false;
  const parsed: unknown[] = bytes.subarray(0, length).toString('utf8').split(/\r?\n/).filter(line => line.trim()).map(line => JSON.parse(line));
  const header = parsed.shift();
  if (!object(header) || header.type !== 'session' || header.id !== handle.sessionId) return false;
  const seen = new Set<string>();
  for (const entry of parsed) {
   if (!object(entry) || typeof entry.id !== 'string' || !entry.id || seen.has(entry.id)
    || typeof entry.type !== 'string' || !(entry.parentId === null || typeof entry.parentId === 'string')) return false;
   seen.add(entry.id);
  }
  const entries = parsed as SessionEntry[];
  const branch = externalPiBranch(entries, entries.at(-1)?.id ?? null);
  const sources = piUserInputSources(branch);
  // Duplicate states for one delivery are ambiguous. Never use another input's context.
  const states = branch.flatMap((entry, index) => entry.type === 'custom' && entry.customType === PI_USER_INPUT
   && piUserInput(entry.data)?.deliveryId === deliveryId ? [index] : []);
  if (states.length !== 1) return false;
  const start = states[0]!;
  const state = branch[start]!;
  if (state.type !== 'custom') return false;
  const input = piUserInput(state.data)!;
  if (!input.rawText.endsWith(expectedContext)) return false;
  const nextInput = branch.findIndex((entry, index) => index > start && entry.type === 'custom' && entry.customType === PI_USER_INPUT);
  const end = nextInput < 0 ? branch.length : nextInput;
  const user = branch.findIndex((entry, index) => index > start && index < end && sources.users.get(entry.id)?.deliveryId === deliveryId);
  if (user < 0) return false;
  const context = branch.slice(start + 1, user).some(entry => entry.type === 'custom_message' && entry.customType === 'hive-user-input-context'
   && object(entry.details) && entry.details.deliveryId === deliveryId && typeof entry.content === 'string' && entry.content.endsWith(expectedContext));
  const results = branch.slice(start + 1, end).filter(entry => entry.type === 'custom' && entry.customType === PI_USER_INPUT_RESULT
   && object(entry.data) && entry.data.deliveryId === deliveryId);
  return context && results.length === 1 && results[0]!.type === 'custom' && object(results[0]!.data) && results[0]!.data.disposition === 'started';
 } catch { return false; }
 finally { if (fd !== undefined) closeSync(fd); }
}

/** Only the handoff's new delivery is filtered; ordinary completion semantics stay intact. */
export function handoffNativeCompletedIds(
 record: ChatRecord,
 handle: Pick<PiSessionHandle, 'sessionId' | 'sessionFile' | 'completedDeliveryIds'>,
 delivery: { deliveryId?: string; nativeSessionId?: string },
 expectedContext: string,
): readonly string[] {
 const completed = handle.completedDeliveryIds();
 const id = delivery.deliveryId;
 if (!id || !completed.includes(id)) return completed;
 const sameNative = !!delivery.nativeSessionId && delivery.nativeSessionId === handle.sessionId;
 const proven = sameNative && (record.backend !== 'pi' && record.backend !== 'pi-rpc' || piConsumed(record, handle, id, expectedContext));
 return proven ? completed : completed.filter(value => value !== id);
}

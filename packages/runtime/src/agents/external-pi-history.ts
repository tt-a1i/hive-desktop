import type { PiUserInput } from './pi-user-input.js';
import type { SessionEntry } from '@earendil-works/pi-coding-agent';
import { createReadStream } from 'node:fs';
import { open, stat } from 'node:fs/promises';
import { createInterface } from 'node:readline';
import { isDeliveryPresentation } from '../delivery-presentation.js';
import { readJsonlTailWindow } from '../history-cursor.js';

function assertPiTranscriptHeader(header: unknown, expected?: string): void {
  if (!header || typeof header !== 'object' || !('type' in header) || header.type !== 'session'
    || !('id' in header) || typeof header.id !== 'string' || !header.id)
    throw new Error('Invalid Pi transcript header');
  if (expected !== undefined && header.id !== expected)
    throw new Error('The saved Pi transcript does not match the original native session ID');
}

async function* piTranscriptLines(file: string, signal?: AbortSignal): AsyncGenerator<string> {
  const input = createReadStream(file, {encoding: 'utf8', ...(signal && {signal})});
  const lines = createInterface({input, crlfDelay: Infinity});
  try {
    for await (const line of lines) {
      signal?.throwIfAborted();
      if (line.trim()) yield line;
    }
  } finally {
    lines.close();
    input.destroy();
  }
}

export async function verifyPiTranscriptIdentity(file: string, expected: string, signal?: AbortSignal): Promise<void> {
  if (!(await stat(file)).isFile()) throw new Error('Pi transcript is not a regular file');
  for await (const header of piTranscriptLines(file, signal)) {
    assertPiTranscriptHeader(JSON.parse(header), expected);
    return;
  }
  throw new Error('Missing Pi transcript header');
}

export async function readPiTranscript(file: string, expectedId?: string, signal?: AbortSignal): Promise<SessionEntry[]> {
  const before = await stat(file, {bigint: true});
  if (!before.isFile()) throw new Error('Pi transcript is not a regular file');
  signal?.throwIfAborted();
  const entries: SessionEntry[] = [];
  const seen = new Set<string>();
  let header = false;
  for await (const raw of piTranscriptLines(file, signal)) {
    let parsed: unknown;
    try { parsed = JSON.parse(raw); } catch { throw new Error('Malformed Pi transcript'); }
    if (!header) {
      assertPiTranscriptHeader(parsed, expectedId);
      header = true;
      continue;
    }
    const entry = parsed as Partial<SessionEntry> | null;
    if (!entry || typeof entry.type !== 'string' || typeof entry.id !== 'string' || !entry.id ||
      !(entry.parentId === null || typeof entry.parentId === 'string') || seen.has(entry.id)) throw new Error('Invalid Pi transcript entry');
    seen.add(entry.id);
    entries.push(entry as SessionEntry);
  }
  signal?.throwIfAborted();
  if (!header) throw new Error('Missing Pi transcript header');
  const after = await stat(file, {bigint: true});
  if (before.dev !== after.dev || before.ino !== after.ino || before.size !== after.size ||
    before.mtimeNs !== after.mtimeNs || before.ctimeNs !== after.ctimeNs) throw new Error('Pi transcript changed while being read; retry');
  return entries;
}

/**
 * Reads only the session header (first line) and a bounded tail byte window of
 * a Pi JSONL transcript from disk using reverse 64 KiB chunked reads.
 * Avoids reading multi-megabyte transcripts completely into memory when only
 * the latest turns (or an earlier byte cursor) are needed.
 */
export async function readPiTranscriptTailWindow(
  file: string,
  options: {
    maxEntries: number;
    beforeByteOffset?: number;
    expectedId?: string;
    signal?: AbortSignal;
  },
): Promise<{
  entries: SessionEntry[];
  startByteOffset: number;
  endByteOffset: number;
  hasEarlierBytes: boolean;
  fileSize: number;
}> {
  options.signal?.throwIfAborted();
  const handle = await open(file, 'r');
  let headerEndOffset = 0;
  try {
    const headBuf = Buffer.allocUnsafe(8192);
    const { bytesRead } = await handle.read(headBuf, 0, headBuf.length, 0);
    const headText = headBuf.subarray(0, bytesRead).toString('utf8');
    const nl = headText.indexOf('\n');
    const headerLine = (nl >= 0 ? headText.slice(0, nl) : headText).trim();
    if (!headerLine) throw new Error('Missing Pi transcript header');
    assertPiTranscriptHeader(JSON.parse(headerLine), options.expectedId);
    headerEndOffset = nl >= 0 ? Buffer.byteLength(headText.slice(0, nl + 1), 'utf8') : bytesRead;
  } finally {
    await handle.close();
  }

  const windowResult = await readJsonlTailWindow(file, {
    maxLines: Math.max(1, options.maxEntries) + 1,
    ...(options.beforeByteOffset !== undefined && { beforeByteOffset: options.beforeByteOffset }),
    ...(options.signal && { signal: options.signal }),
  });

  const entries: SessionEntry[] = [];
  for (const raw of windowResult.lines) {
    let parsed: unknown;
    try {
      parsed = JSON.parse(raw);
    } catch {
      throw new Error('Malformed Pi transcript');
    }
    if (parsed && typeof parsed === 'object' && (parsed as { type?: unknown }).type === 'session') continue;
    const entry = parsed as Partial<SessionEntry> | null;
    if (
      !entry ||
      typeof entry.type !== 'string' ||
      typeof entry.id !== 'string' ||
      !entry.id ||
      !(entry.parentId === null || typeof entry.parentId === 'string')
    ) {
      throw new Error('Invalid Pi transcript entry');
    }
    entries.push(entry as SessionEntry);
  }

  return {
    entries,
    startByteOffset: Math.max(headerEndOffset, windowResult.startByteOffset),
    endByteOffset: windowResult.endByteOffset,
    hasEarlierBytes: windowResult.startByteOffset > headerEndOffset,
    fileSize: windowResult.fileSize,
  };
}

/**
 * Walks backwards from the leaf entry strictly within a tail window.
 * Returns `complete: true` when the parent chain reaches `null` (file start),
 * or `complete: false` when an earlier parent lies before the loaded byte window.
 * Returns `undefined` if a cycle is detected so callers fall back to full read.
 */
export function externalPiTailBranch(
  entries: readonly SessionEntry[],
): { branch: SessionEntry[]; reachedRoot: boolean } | undefined {
  if (entries.length === 0) return { branch: [], reachedRoot: true };
  const byId = new Map(entries.map(entry => [entry.id, entry]));
  const branch: SessionEntry[] = [];
  const visited = new Set<string>();
  let id: string | null = entries[entries.length - 1]!.id;
  while (id !== null) {
    if (visited.has(id)) return undefined;
    visited.add(id);
    const entry = byId.get(id);
    if (!entry) {
      return { branch: branch.reverse(), reachedRoot: false };
    }
    branch.push(entry);
    id = entry.parentId;
  }
  return { branch: branch.reverse(), reachedRoot: true };
}

export function externalPiBranch(entries: readonly SessionEntry[], leafId: string | null): SessionEntry[] {
  const byId = new Map(entries.map(entry => [entry.id, entry]));
  const branch: SessionEntry[] = [];
  const visited = new Set<string>();
  let id = leafId;
  while (id !== null) {
    if (visited.has(id)) throw new Error('External Pi session branch contains a cycle');
    visited.add(id);
    const entry = byId.get(id);
    if (!entry) throw new Error('External Pi session branch parent is missing');
    branch.push(entry); id = entry.parentId;
  }
  return branch.reverse();
}
export function externalPiMessage(entry: SessionEntry, input?: PiUserInput, backend: 'pi' | 'pi-rpc' = 'pi-rpc'): Record<string, unknown> | undefined {
  let message: Record<string, unknown>;
  if (entry.type === 'message') message = { ...entry.message };
  else if (entry.type === 'custom_message') message = { role: 'custom', customType: entry.customType, content: entry.content,
    display: entry.display, details: entry.details, timestamp: Date.parse(entry.timestamp) };
  else return;
  if (message.role === 'custom' && message.customType === 'hive-delivery' && isDeliveryPresentation(message.details)
    && 'deliveryId' in message.details && typeof message.details.deliveryId === 'string') message.presentation = message.details;
  if (message.role === 'user' && input) message.presentation = input;
  return { ...message, rawBackendItem: entry, source: { backend, entryId: entry.id } };
}

/** Pi RPC updates omit cumulative SDK messages; reconstruct only the current content block. */
export function applyExternalPiUpdate(message: Record<string, unknown>, event: Record<string, unknown>): Record<string, unknown> {
  const content = Array.isArray(message.content) ? [...message.content] as Record<string, unknown>[] : [];
  const update = event.assistantMessageEvent as Record<string, unknown>;
  const index = update.contentIndex;
  if (typeof index !== 'number') return message;
  const previous = content[index];
  switch (update.type) {
    case 'text_start': content[index] = { type: 'text', text: '' }; break;
    case 'thinking_start': content[index] = { type: 'thinking', thinking: '' }; break;
    case 'text_delta': content[index] = { ...previous, type: 'text', text: String(previous?.text ?? '') + String(update.delta) }; break;
    case 'thinking_delta': content[index] = { ...previous, type: 'thinking', thinking: String(previous?.thinking ?? '') + String(update.delta) }; break;
    case 'text_end': content[index] = { type: 'text', text: update.content }; break;
    case 'thinking_end': content[index] = { type: 'thinking', thinking: update.content }; break;
    case 'toolcall_start': content[index] = { type: 'toolCall', id: update.id, name: update.toolName, arguments: {} }; break;
    case 'toolcall_end': content[index] = update.toolCall as Record<string, unknown>; break;
  }
  return { ...message, content, ...(event.usage !== undefined && { usage: event.usage }) };
}

import { open, stat, type FileHandle } from 'node:fs/promises';

export interface HistoryPageMeta {
  totalCount: number;
  startIndex: number;
  hasOlder: boolean;
  nextBeforeIndex?: number;
  nextBeforeMessageId?: string;
  hasNewer: boolean;
  nextAfterIndex?: number;
  nextAfterMessageId?: string;
  snapshot?: string;
}

export interface HistoryWindowOptions {
  limit?: number;
  aroundMessageId?: string;
  beforeIndex?: number;
  beforeMessageId?: string;
  afterIndex?: number;
  afterMessageId?: string;
  snapshot?: string;
}

export class StaleHistorySnapshotError extends Error {
  readonly code = 'HISTORY_SNAPSHOT_STALE';
  constructor() { super('Saved history changed. Reopen the conversation to refresh its history.'); }
}

export interface HistorySliceResult<T> {
  history: T[];
  historyMessageIds: string[];
  historyPage: HistoryPageMeta;
}

export interface JsonlTailWindowResult {
  lines: string[];
  startByteOffset: number;
  endByteOffset: number;
  hasEarlierBytes: boolean;
  fileSize: number;
}

export interface ToolOutputByteWindow {
  chunk: string;
  byteOffset: number;
  nextByteOffset: number;
  totalBytes: number;
  hasMore: boolean;
}

export const DEFAULT_HISTORY_PAGE_LIMIT = 120;
export const MAX_HISTORY_PAGE_LIMIT = 500;
export const MAX_PAGED_TOOL_OUTPUT_CHARS = 32_768;
export const DEFAULT_TOOL_OUTPUT_SLICE_BYTES = 32_768;
export const MAX_TOOL_OUTPUT_SLICE_BYTES = 262_144;
export const REVERSE_CHUNK_BYTES = 65_536;

/** Visits one raw line at a time within a caller-verified file version. */
export async function* readHistoryJsonlLines(
  file: FileHandle, size: number, {onRead, signal}: {onRead?: (bytes: number) => void; signal?: AbortSignal} = {},
) {
  let fragments: Buffer[] = [];
  let start = 0;
  for (let pos = 0; pos < size;) {
    signal?.throwIfAborted();
    // Each chunk has its own buffer. The unfinished line may retain it until
    // the next newline, but completed lines are released after indexing.
    const buffer = Buffer.allocUnsafe(Math.min(REVERSE_CHUNK_BYTES, size - pos));
    const {bytesRead} = await file.read(buffer, 0, buffer.length, pos);
    onRead?.(bytesRead);
    if (!bytesRead) throw new StaleHistorySnapshotError();
    const actual = buffer.subarray(0, bytesRead);
    let cursor = 0;
    while (cursor < bytesRead) {
      const newline = actual.indexOf(0x0a, cursor);
      if (newline < 0) {
        fragments.push(actual.subarray(cursor));
        break;
      }
      const tail = buffer.subarray(cursor, newline);
      const raw = fragments.length ? Buffer.concat([...fragments, tail]) : tail;
      yield {start, raw};
      fragments = [];
      cursor = newline + 1;
      start = pos + cursor;
    }
    pos += bytesRead;
  }
  if (fragments.length) yield {start, raw: Buffer.concat(fragments)};
}

/**
 * Reads up to `maxLines` non-empty lines from a JSONL file backwards starting at
 * `beforeByteOffset` (defaults to EOF), scanning in fixed-size chunks so huge
 * cold transcripts never have to be read completely into memory just to render
 * the latest page.
 */
export async function readJsonlTailWindow(
  filePath: string,
  options: {
    maxLines: number;
    beforeByteOffset?: number;
    chunkBytes?: number;
    signal?: AbortSignal;
  },
): Promise<JsonlTailWindowResult> {
  const maxLines = Math.max(1, Math.floor(options.maxLines));
  const chunkBytes = Math.max(4096, Math.floor(options.chunkBytes ?? REVERSE_CHUNK_BYTES));
  options.signal?.throwIfAborted();
  const info = await stat(filePath);
  options.signal?.throwIfAborted();
  const fileSize = info.size;
  const endByteOffset = options.beforeByteOffset === undefined
    ? fileSize
    : Math.min(Math.max(0, Math.floor(options.beforeByteOffset)), fileSize);

  if (endByteOffset === 0) {
    return { lines: [], startByteOffset: 0, endByteOffset: 0, hasEarlierBytes: false, fileSize };
  }

  const handle = await open(filePath, 'r');
  try {
    let cursor = endByteOffset;
    let firstLineStart: number | undefined;
    const lines: string[] = [];
    // Chunks are read newest first. Keep each unfinished line in fragments so
    // its bytes are copied and decoded only after finding the start of the line.
    const fragments: Buffer[] = [];

    function collectLine(head: Buffer, startByte: number) {
      let line = head;
      if (fragments.length > 0) {
        fragments.push(head);
        line = Buffer.concat(fragments.reverse());
        fragments.length = 0;
      }
      const trimmed = line.toString('utf8').trim();
      if (trimmed.length > 0) {
        lines.push(trimmed);
        firstLineStart = startByte;
      }
    }

    options.signal?.throwIfAborted();
    while (cursor > 0 && lines.length < maxLines) {
      const readLen = Math.min(chunkBytes, cursor);
      const readPos = cursor - readLen;
      const buf = Buffer.allocUnsafe(readLen);
      const { bytesRead } = await handle.read(buf, 0, readLen, readPos);
      options.signal?.throwIfAborted();
      if (bytesRead <= 0) break;

      let lineEnd = bytesRead;
      while (lineEnd > 0 && lines.length < maxLines) {
        const newline = buf.lastIndexOf(0x0a, lineEnd - 1);
        if (newline < 0) break;
        collectLine(buf.subarray(newline + 1, lineEnd), readPos + newline + 1);
        lineEnd = newline;
      }

      if (lines.length >= maxLines) break;
      if (lineEnd > 0) fragments.push(buf.subarray(0, lineEnd));
      cursor = readPos;
    }

    if (cursor === 0 && lines.length < maxLines) {
      const firstFragment = fragments.pop();
      if (firstFragment) collectLine(firstFragment, 0);
    }

    const startByteOffset = firstLineStart ?? cursor;
    return {
      lines: lines.reverse(),
      startByteOffset,
      endByteOffset,
      hasEarlierBytes: startByteOffset > 0,
      fileSize,
    };
  } finally {
    await handle.close();
  }
}

/**
 * Slices a UTF-8 safe byte window from a full tool output string so callers can
 * stream huge outputs incrementally by byte offset without splitting multi-byte
 * characters.
 */
export function readToolOutputByteWindow(
  fullText: string,
  byteOffset: number,
  byteLength: number = DEFAULT_TOOL_OUTPUT_SLICE_BYTES,
): ToolOutputByteWindow {
  const buf = Buffer.from(fullText, 'utf8');
  const totalBytes = buf.length;
  let start = Math.min(Math.max(0, Math.floor(byteOffset)), totalBytes);
  // Advance past any UTF-8 continuation bytes (10xxxxxx)
  while (start < totalBytes && (buf[start]! & 0xc0) === 0x80) {
    start++;
  }
  const clampedLen = Math.min(Math.max(1024, Math.floor(byteLength)), MAX_TOOL_OUTPUT_SLICE_BYTES);
  let end = Math.min(totalBytes, start + clampedLen);
  // Back off if end lands in the middle of a multi-byte UTF-8 sequence
  if (end < totalBytes && (buf[end]! & 0xc0) === 0x80) {
    while (end > start && (buf[end]! & 0xc0) === 0x80) {
      end--;
    }
  }
  // Edge case: a single 4-byte char was larger than remaining slice
  if (end === start && start < totalBytes) {
    end = Math.min(totalBytes, start + 1);
    while (end < totalBytes && (buf[end]! & 0xc0) === 0x80) {
      end++;
    }
  }
  const chunk = buf.subarray(start, end).toString('utf8');
  return {
    chunk,
    byteOffset: start,
    nextByteOffset: end,
    totalBytes,
    hasMore: end < totalBytes,
  };
}

/**
 * Bounds oversized tool result text blocks on paginated history reads so a single
 * 2 MB command output does not bloat IPC or desktop layout memory. Full unpaginated
 * reads (such as Markdown export) remain untouched.
 */
export function boundOversizedToolOutput<T>(message: T, maxChars = MAX_PAGED_TOOL_OUTPUT_CHARS): T {
  if (!message || typeof message !== 'object') return message;
  const record = message as Record<string, unknown>;
  if (record.role !== 'toolResult' || !Array.isArray(record.content)) return message;
  let changed = false;
  const nextContent = record.content.map(block => {
    if (!block || typeof block !== 'object') return block;
    const item = block as Record<string, unknown>;
    if (item.type === 'text' && typeof item.text === 'string' && item.text.length > maxChars) {
      changed = true;
      const totalBytes = Buffer.byteLength(item.text, 'utf8');
      let head = item.text.slice(0, maxChars);
      const lastCodeUnit = head.charCodeAt(head.length - 1);
      const nextCodeUnit = item.text.charCodeAt(head.length);
      if (lastCodeUnit >= 0xd800 && lastCodeUnit <= 0xdbff
        && nextCodeUnit >= 0xdc00 && nextCodeUnit <= 0xdfff) {
        head = head.slice(0, -1);
      }
      const loadedBytes = Buffer.byteLength(head, 'utf8');
      const omitted = item.text.length - head.length;
      return {
        ...item,
        text: `${head}\n… [已截断 ${omitted} 字符以保持流畅；可按字节窗口继续加载或导出完整 Markdown]`,
        truncatedChars: omitted,
        totalBytes,
        loadedBytes,
        hasMoreBytes: loadedBytes < totalBytes,
      };
    }
    return block;
  });
  return changed ? ({ ...record, content: nextContent } as T) : message;
}

export function clampHistoryLimit(limit?: number): number {
  if (limit === undefined || !Number.isFinite(limit) || limit <= 0) {
    return DEFAULT_HISTORY_PAGE_LIMIT;
  }
  return Math.min(Math.max(1, Math.floor(limit)), MAX_HISTORY_PAGE_LIMIT);
}

/**
 * Slices a bounded window from an authoritative conversation transcript.
 */
export function sliceHistoryWindow<T>(
  history: readonly T[],
  historyMessageIds: readonly string[],
  options: HistoryWindowOptions,
): HistorySliceResult<T> {
  const {startIndex, endIndex} = resolveHistoryWindow(history.length, options,
    id => historyMessageIds.indexOf(id), id => historyFocusIndex(history, historyMessageIds, id));
  return historyWindowResult(history.slice(startIndex, endIndex), historyMessageIds.slice(startIndex, endIndex),
    history.length, startIndex);
}

export function resolveHistoryWindow(
  totalCount: number,
  options: HistoryWindowOptions,
  findId: (id: string) => number,
  findFocus: (id: string) => number,
  idAt?: (index: number) => string | undefined,
): {startIndex: number; endIndex: number} {
  const limit = clampHistoryLimit(options.limit);
  if (idAt && options.snapshot !== undefined) {
    const cursor = options.beforeIndex ?? options.afterIndex;
    const id = options.beforeMessageId ?? options.afterMessageId;
    if (cursor !== undefined && id !== undefined && idAt(cursor) !== id) throw new StaleHistorySnapshotError();
  }

  let startIndex = 0;
  let endIndex = totalCount;

  if (options.beforeIndex !== undefined) {
    let resolvedEnd = Math.min(Math.max(0, Math.floor(options.beforeIndex)), totalCount);
    if (options.beforeMessageId && options.snapshot === undefined) {
      const exactIdx = findId(options.beforeMessageId);
      if (exactIdx >= 0) {
        resolvedEnd = exactIdx;
      }
    }
    endIndex = resolvedEnd;
    startIndex = Math.max(0, endIndex - limit);
  } else if (options.afterIndex !== undefined) {
    const exactIdx = options.afterMessageId && options.snapshot === undefined ? findId(options.afterMessageId) : -1;
    const afterIndex = exactIdx >= 0 ? exactIdx : Math.floor(options.afterIndex);
    startIndex = Math.min(Math.max(0, afterIndex + 1), totalCount);
    endIndex = Math.min(totalCount, startIndex + limit);
  } else if (totalCount > limit) {
    startIndex = totalCount - limit;
    if (options.aroundMessageId) {
      const targetIdx = findFocus(options.aroundMessageId);
      if (targetIdx >= 0 && targetIdx < startIndex) {
        startIndex = Math.max(0, targetIdx - Math.min(20, Math.floor(limit / 2)));
        endIndex = startIndex + limit;
      }
    }
  }

  return {startIndex, endIndex};
}

export function historyWindowResult<T>(
  history: T[], historyMessageIds: string[], totalCount: number, startIndex: number, snapshot?: string,
): HistorySliceResult<T> {
  const endIndex = startIndex + history.length;
  const hasOlder = startIndex > 0;
  const nextBeforeMessageId = hasOlder ? historyMessageIds[0] : undefined;
  const hasNewer = endIndex < totalCount && history.length > 0;
  const nextAfterMessageId = hasNewer ? historyMessageIds.at(-1) : undefined;

  return {
    history,
    historyMessageIds,
    historyPage: {
      totalCount,
      startIndex,
      hasOlder,
      ...(hasOlder && { nextBeforeIndex: startIndex }),
      ...(nextBeforeMessageId && { nextBeforeMessageId }),
      hasNewer,
      ...(hasNewer && { nextAfterIndex: endIndex - 1 }),
      ...(nextAfterMessageId && { nextAfterMessageId }),
      ...(snapshot && {snapshot}),
    },
  };
}

export function historyRowFocusId(message: unknown): string | undefined {
  if (!message || typeof message !== 'object') return;
  const item = message as Record<string, unknown>;
  const presentation = item.presentation as { deliveryId?: unknown } | undefined;
  if (item.role === 'user' || (item.role === 'custom' && item.customType === 'hive-delivery')) {
    if (typeof presentation?.deliveryId === 'string') return presentation.deliveryId;
  } else if (item.role === 'toolResult' && typeof item.toolCallId === 'string') return item.toolCallId;
  return;
}

export function historyToolCallFocusIds(message: unknown): string[] {
  if (!message || typeof message !== 'object') return [];
  const item = message as Record<string, unknown>;
  return item.role === 'assistant' && Array.isArray(item.content)
    ? item.content.flatMap(block => block?.type === 'toolCall' && typeof block.id === 'string' ? [block.id] : []) : [];
}

function historyFocusIndex(
  history: readonly unknown[],
  ids: readonly string[],
  target: string,
): number {
  const exact = ids.indexOf(target);
  if (exact >= 0) return exact;

  // Search and reading positions use the desktop row identity. The native
  // history ID can differ for deliveries, tool results and unresolved calls.
  const projected = history.findIndex(message => historyRowFocusId(message) === target);
  if (projected >= 0) return projected;
  return history.findIndex(message => historyToolCallFocusIds(message).includes(target));
}

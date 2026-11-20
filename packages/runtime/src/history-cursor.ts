import { open, stat } from 'node:fs/promises';

export interface HistoryPageMeta {
  totalCount: number;
  startIndex: number;
  hasOlder: boolean;
  nextBeforeIndex?: number;
  nextBeforeMessageId?: string;
  hasNewer: boolean;
  nextAfterIndex?: number;
  nextAfterMessageId?: string;
}

export interface HistoryWindowOptions {
  limit?: number;
  aroundMessageId?: string;
  beforeIndex?: number;
  beforeMessageId?: string;
  afterIndex?: number;
  afterMessageId?: string;
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
  const totalCount = history.length;
  const limit = clampHistoryLimit(options.limit);

  let startIndex = 0;
  let endIndex = totalCount;

  if (options.beforeIndex !== undefined) {
    let resolvedEnd = Math.min(Math.max(0, Math.floor(options.beforeIndex)), totalCount);
    if (options.beforeMessageId) {
      const exactIdx = historyMessageIds.indexOf(options.beforeMessageId);
      if (exactIdx >= 0) {
        resolvedEnd = exactIdx;
      }
    }
    endIndex = resolvedEnd;
    startIndex = Math.max(0, endIndex - limit);
  } else if (options.afterIndex !== undefined) {
    const exactIdx = options.afterMessageId ? historyMessageIds.indexOf(options.afterMessageId) : -1;
    const afterIndex = exactIdx >= 0 ? exactIdx : Math.floor(options.afterIndex);
    startIndex = Math.min(Math.max(0, afterIndex + 1), totalCount);
    endIndex = Math.min(totalCount, startIndex + limit);
  } else if (totalCount > limit) {
    startIndex = totalCount - limit;
    if (options.aroundMessageId) {
      const targetIdx = historyFocusIndex(history, historyMessageIds, options.aroundMessageId);
      if (targetIdx >= 0 && targetIdx < startIndex) {
        startIndex = Math.max(0, targetIdx - Math.min(20, Math.floor(limit / 2)));
        endIndex = startIndex + limit;
      }
    }
  }

  const slicedHistory = history.slice(startIndex, endIndex);
  const slicedIds = historyMessageIds.slice(startIndex, endIndex);
  const hasOlder = startIndex > 0;
  const nextBeforeMessageId = hasOlder ? slicedIds[0] : undefined;
  const hasNewer = endIndex < totalCount && slicedHistory.length > 0;
  const nextAfterMessageId = hasNewer ? slicedIds.at(-1) : undefined;

  return {
    history: slicedHistory,
    historyMessageIds: slicedIds,
    historyPage: {
      totalCount,
      startIndex,
      hasOlder,
      ...(hasOlder && { nextBeforeIndex: startIndex }),
      ...(nextBeforeMessageId && { nextBeforeMessageId }),
      hasNewer,
      ...(hasNewer && { nextAfterIndex: endIndex - 1 }),
      ...(nextAfterMessageId && { nextAfterMessageId }),
    },
  };
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
  const projected = history.findIndex(message => {
    if (!message || typeof message !== 'object') return false;
    const item = message as Record<string, unknown>;
    const presentation = item.presentation as { deliveryId?: unknown } | undefined;
    if (item.role === 'user' || (item.role === 'custom' && item.customType === 'hive-delivery')) {
      return presentation?.deliveryId === target;
    }
    return item.role === 'toolResult' && item.toolCallId === target;
  });
  if (projected >= 0) return projected;
  return history.findIndex(message => {
    if (!message || typeof message !== 'object') return false;
    const item = message as Record<string, unknown>;
    return item.role === 'assistant' && Array.isArray(item.content)
      && item.content.some(block => block?.type === 'toolCall' && block.id === target);
  });
}

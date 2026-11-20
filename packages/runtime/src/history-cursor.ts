export interface HistoryPageMeta {
  totalCount: number;
  startIndex: number;
  hasOlder: boolean;
  nextBeforeIndex?: number;
  nextBeforeMessageId?: string;
}

export interface HistorySliceResult<T> {
  history: T[];
  historyMessageIds: string[];
  historyPage: HistoryPageMeta;
}

export const DEFAULT_HISTORY_PAGE_LIMIT = 120;
export const MAX_HISTORY_PAGE_LIMIT = 500;

export function clampHistoryLimit(limit?: number): number {
  if (limit === undefined || !Number.isFinite(limit) || limit <= 0) {
    return DEFAULT_HISTORY_PAGE_LIMIT;
  }
  return Math.min(Math.max(1, Math.floor(limit)), MAX_HISTORY_PAGE_LIMIT);
}

/**
 * Slices a bounded window from an authoritative conversation transcript.
 *
 * - On initial open (`beforeIndex === undefined`): returns the latest `limit` messages,
 *   or if `aroundMessageId` is found earlier in the transcript, expands the start index
 *   to include `aroundMessageId` (with 20 messages of preceding context) through the tail,
 *   preserving contiguous live event appending, search jump targeting, and reading-anchor restore.
 * - On backward pagination (`beforeIndex !== undefined`): verifies `beforeMessageId` if given
 *   (adjusting `end` if the message shifted), and returns `[max(0, end - limit), end)`.
 */
export function sliceHistoryWindow<T>(
  history: readonly T[],
  historyMessageIds: readonly string[],
  options: {
    limit?: number;
    aroundMessageId?: string;
    beforeIndex?: number;
    beforeMessageId?: string;
  },
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
  } else if (totalCount > limit) {
    startIndex = totalCount - limit;
    if (options.aroundMessageId) {
      const targetIdx = historyMessageIds.indexOf(options.aroundMessageId);
      if (targetIdx >= 0 && targetIdx < startIndex) {
        startIndex = Math.max(0, targetIdx - 20);
      }
    }
  }

  const slicedHistory = history.slice(startIndex, endIndex);
  const slicedIds = historyMessageIds.slice(startIndex, endIndex);
  const hasOlder = startIndex > 0;
  const nextBeforeMessageId = hasOlder ? slicedIds[0] : undefined;

  return {
    history: slicedHistory,
    historyMessageIds: slicedIds,
    historyPage: {
      totalCount,
      startIndex,
      hasOlder,
      ...(hasOlder && { nextBeforeIndex: startIndex }),
      ...(nextBeforeMessageId && { nextBeforeMessageId }),
    },
  };
}

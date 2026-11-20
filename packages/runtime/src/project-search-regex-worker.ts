import { parentPort, workerData } from 'node:worker_threads';

export type RegexRange = [number, number];
export type RegexLine = {line: number; start: number; end: number; anchor: number; ranges: RegexRange[]};
export type RegexScan = {total: number; lines: RegexLine[]};

type Configuration = {query: string; caseSensitive: boolean; wordClass?: string; rangeLimit: number};
type ScanRequest = {kind: 'scan'; text: string; offset: number; limit: number};
export type RegexRequest = ScanRequest | {kind: 'matchPaths'; paths: readonly string[]};
export type RegexResult = ({kind: 'scanned'} & RegexScan) | {kind: 'matchedPaths'; indices: number[]};
const port = parentPort!;
const {query, caseSensitive, wordClass, rangeLimit} = workerData as Configuration;
const flags = caseSensitive ? 'gu' : 'giu';

function highlightRanges(pattern: RegExp, text: string, first: RegExpExecArray): RegexRange[] {
  const ranges: RegexRange[] = [];
  for (let match: RegExpExecArray | null = first; match; match = pattern.exec(text)) {
    if (match[0].length) {
      ranges.push([match.index, match.index + match[0].length]);
      if (ranges.length >= rangeLimit) break;
    } else {
      // RegExp.exec leaves lastIndex at a zero-width match. Move past one full
      // code point to continue looking for visible ranges on the same line.
      const codePoint = text.codePointAt(pattern.lastIndex);
      pattern.lastIndex += codePoint !== undefined && codePoint > 0xffff ? 2 : 1;
    }
  }
  return ranges;
}

function scan(pattern: RegExp, {text, offset, limit}: ScanRequest): RegexScan {
  const lines: RegexLine[] = [];
  let total = 0;
  for (let start = 0, line = 1; start < text.length; line++) {
    const newline = text.indexOf('\n', start);
    const next = newline === -1 ? text.length : newline;
    const end = next > start && text[next - 1] === '\r' ? next - 1 : next;
    const value = text.slice(start, end);
    pattern.lastIndex = 0;
    const first = pattern.exec(value);
    if (first) {
      if (total >= offset && lines.length < limit) {
        lines.push({line, start, end, anchor: first.index, ranges: highlightRanges(pattern, value, first)});
      }
      total += 1;
    }
    start = next + 1;
  }
  return {total, lines};
}

function matchPaths(pattern: RegExp, paths: readonly string[]): RegexResult {
  const indices: number[] = [];
  for (const [index, path] of paths.entries()) {
    pattern.lastIndex = 0;
    if (pattern.test(path)) indices.push(index);
  }
  return {kind: 'matchedPaths', indices};
}

try {
  // Validate the user expression before combining it with our static word
  // assertions. Both compilation and execution stay in this worker.
  const userPattern = new RegExp(query, flags);
  const pattern = wordClass
    ? new RegExp(`(?<!${wordClass})(?:${query})(?!${wordClass})`, flags)
    : userPattern;
  port.on('message', (request: RegexRequest) => {
    try {
      const result: RegexResult = request.kind === 'scan'
        ? {kind: 'scanned', ...scan(pattern, request)} : matchPaths(pattern, request.paths);
      port.postMessage(result);
    } catch {
      port.postMessage({kind: 'failed'});
    }
  });
  port.postMessage({kind: 'ready'});
} catch {
  port.postMessage({kind: 'invalid'});
  port.close();
}

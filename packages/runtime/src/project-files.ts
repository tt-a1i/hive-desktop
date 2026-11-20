import { execFile } from 'node:child_process';
import { constants } from 'node:fs';
import { lstat, open, readdir, realpath, stat } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { WORKSPACE_UPLOAD_MAX_BYTES } from '@tt-a1i/hive/dist/src/server/upload-limits.js';
import { isAbsolute, join, relative, resolve, sep } from 'node:path';
import { promisify } from 'node:util';
import { startProjectRegexSearch } from './project-search-regex.js';
import type { RegexRange } from './project-search-regex-worker.js';

const exec = promisify(execFile);
const PAGE_SIZE = 100;
const PREVIEW_LIMIT = 1024 * 1024;
const CONTENT_SEARCH_LIMIT = 64 * 1024 * 1024;
const SNIPPET_BYTES = 192;
const SNIPPET_CONTEXT_BYTES = 16;
const SNIPPET_RANGE_LIMIT = 8;
const UNICODE_MAPPING_LIMIT = 4096;
const graphemes = new Intl.Segmenter('und', {granularity: 'grapheme'});
const EXCLUDED = new Set(['.git', 'node_modules', 'vendor', 'target', 'dist', 'build', '.next', '.venv', 'venv', '__pycache__']);

function inside(root: string, path: string): boolean {
  const rel = relative(root, path);
  return rel !== '' && rel !== '..' && !rel.startsWith(`..${sep}`) && !isAbsolute(rel);
}
function validRelative(path: string): boolean {
  return !!path && !path.includes('\0') && !isAbsolute(path) && !/^[A-Za-z]:/.test(path)
    && !path.split(/[\\/]/).some(part => !part || part === '.' || part === '..' || part === '.git');
}
async function rootDirectory(root: string): Promise<string> {
  const canonical = await realpath(root);
  if (!(await stat(canonical)).isDirectory()) throw new Error('Project root must be a directory');
  return canonical;
}

/** Resolves the current file target. Callers must independently secure any later open
 * against replacement races; a path alone cannot guarantee that later access is safe. */
export async function resolveProjectFile(projectRoot: string, relativePath: string): Promise<string> {
  return resolveFileInRoot(await rootDirectory(projectRoot), relativePath);
}

export async function resolveProjectFileForEditor(projectRoot: string, relativePath: string) {
  if (relativePath.includes('\\')) throw new TypeError('Invalid project file path');
  const root = await rootDirectory(projectRoot);
  const path = await resolveFileInRoot(root, relativePath);
  if (path !== resolve(root, relativePath)) {
    throw new Error('File is outside project or not a regular file');
  }
  return {root, path};
}

async function resolveFileInRoot(root: string, relativePath: string): Promise<string> {
  if (typeof relativePath !== 'string' || !validRelative(relativePath)) throw new TypeError('Invalid project file path');
  const path = resolve(root, relativePath);
  if (!inside(root, path)) throw new Error('File is outside project');
  const canonical = await realpath(path);
  if (!inside(root, canonical) || !validRelative(relative(root, canonical)) || !(await stat(canonical)).isFile()) throw new Error('File is outside project or not a regular file');
  return canonical;
}

async function gitFiles(root: string, signal?: AbortSignal): Promise<string[] | null> {
  const options = { cwd: root, signal, maxBuffer: 64 * 1024 * 1024, env: { ...process.env, LC_ALL: 'C', GIT_OPTIONAL_LOCKS: '0', GIT_CONFIG_NOSYSTEM: '1', GIT_CONFIG_GLOBAL: process.platform === 'win32' ? 'NUL' : '/dev/null' } };
  const prefix = ['-c', 'core.fsmonitor=false', '-c', 'core.hooksPath=/dev/null'];
  try {
    await exec('git', [...prefix, 'rev-parse', '--is-inside-work-tree'], options);
  } catch (error) {
    const e = error as { code?: string | number; stderr?: string };
    if (e.code === 'ENOENT' || (e.code === 128 && e.stderr?.includes('not a git repository'))) return null;
    throw error;
  }
  // An explicitly opened directory may itself be ignored by a containing repo.
  // In that case Git would hide every untracked file beneath the project root.
  try {
    await exec('git', [...prefix, 'check-ignore', '-q', '--', '.'], options);
    return null;
  } catch (error) {
    if ((error as { code?: string | number }).code !== 1) throw error;
  }
  const { stdout } = await exec('git', [...prefix, 'ls-files', '--cached', '--others', '--exclude-standard', '-z', '--', '.'], options);
  return stdout.split('\0').filter(Boolean);
}
async function walk(root: string, current = '', signal?: AbortSignal): Promise<string[]> {
  signal?.throwIfAborted();
  const result: string[] = [];
  for (const entry of await readdir(join(root, current), { withFileTypes: true })) {
    if (EXCLUDED.has(entry.name) || entry.isSymbolicLink()) continue;
    const path = join(current, entry.name);
    if (entry.isDirectory()) {
      for (const child of await walk(root, path, signal)) result.push(child);
    } else if (entry.isFile()) result.push(path);
  }
  return result;
}

function validateSearch(query: string, offset: number) {
  if (typeof query !== 'string' || query.length > 200 || /[\0\r\n]/.test(query)) throw new TypeError('Invalid project file query');
  if (!Number.isSafeInteger(offset) || offset < 0) throw new TypeError('Invalid project file offset');
}

async function regularCandidate(root: string, path: string) {
  // Never traverse symlink directories; ignore stale deleted Git index entries.
  let current = root;
  for (const part of path.split('/')) {
    current = join(current, part);
    if ((await lstat(current)).isSymbolicLink()) return false;
  }
  return (await stat(current)).isFile();
}

function searchText(value: string, caseSensitive: boolean): string {
  const normalized = value.normalize('NFC');
  return caseSensitive ? normalized : normalized.toLowerCase();
}

// Unicode word characters, including connector punctuation such as underscore.
const WORD_CHARACTER = /[\p{Alphabetic}\p{Join_Control}\p{Mark}\p{Decimal_Number}\p{Connector_Punctuation}]/u;

function wordAt(value: string, offset: number): boolean {
  const codePoint = value.codePointAt(offset);
  if (codePoint === undefined) return false;
  if (codePoint < 128) return codePoint === 0x5f || (codePoint >= 0x30 && codePoint <= 0x39)
    || (codePoint >= 0x41 && codePoint <= 0x5a) || (codePoint >= 0x61 && codePoint <= 0x7a);
  return WORD_CHARACTER.test(String.fromCodePoint(codePoint));
}

function nextLiteralMatch(value: string, needle: string, offset: number, wholeWord: boolean): number {
  let match = value.indexOf(needle, offset);
  if (!wholeWord) return match;
  while (match !== -1) {
    const previousUnit = value.charCodeAt(match - 1);
    const previous = match - (previousUnit >= 0xdc00 && previousUnit <= 0xdfff ? 2 : 1);
    if (!wordAt(value, previous) && !wordAt(value, match + needle.length)) return match;
    match = value.indexOf(needle, match + 1);
  }
  return -1;
}

type SearchSnippet = {
  text: string;
  ranges: [number, number][];
  leadingEllipsis: boolean;
  trailingEllipsis: boolean;
};

function* literalRanges(searchable: string, needle: string, start: number, end: number, wholeWord: boolean) {
  let match = nextLiteralMatch(searchable, needle, start, wholeWord);
  while (match !== -1 && match + needle.length <= end) {
    const next = match + needle.length;
    yield [match, next] as RegexRange;
    match = nextLiteralMatch(searchable, needle, next, wholeWord);
  }
}

function snippetRanges(start: number, end: number, sourceByte: (offset: number) => number | undefined,
  matches: Iterable<RegexRange>): SearchSnippet['ranges'] {
  const ranges: SearchSnippet['ranges'] = [];
  const base = sourceByte(start)!;
  for (const [match, matchEnd] of matches) {
    if (match < start || matchEnd > end) continue;
    const a = sourceByte(match);
    const b = sourceByte(matchEnd);
    if (a !== undefined && b !== undefined && a < b) ranges.push([a - base, b - base]);
    if (ranges.length >= SNIPPET_RANGE_LIMIT) break;
  }
  return ranges;
}

function snippetWindowStart(matchByte: number, matchBytes: number): number {
  const context = Math.min(SNIPPET_CONTEXT_BYTES, Math.max(0, SNIPPET_BYTES - matchBytes));
  return Math.max(0, matchByte - context);
}

function lineSnippet(
  line: string, searchable: string, [firstMatch, firstMatchEnd]: RegexRange,
  normalize: (segment: string) => string, matches: (start: number, end: number) => Iterable<RegexRange>,
): SearchSnippet | undefined {
  const source = line.endsWith('\r') ? line.slice(0, -1) : line;
  const result = (start: number, end: number, ranges: SearchSnippet['ranges']): SearchSnippet | undefined => {
    const text = source.slice(start, end);
    // The excerpt is a native, single-line label. Skip undisplayable controls;
    // the file and its exact line remain available to open.
    if (!text || /[\u0000-\u0008\u000a-\u001f\u007f-\u009f\u2028\u2029]/u.test(text)) return undefined;
    return {text, ranges, leadingEllipsis: start > 0, trailingEllipsis: end < source.length};
  };
  if (/^[\t\x20-\x7e]*$/.test(source)) {
    const start = Math.min(source.length, snippetWindowStart(firstMatch, firstMatchEnd - firstMatch));
    const end = Math.min(source.length, start + SNIPPET_BYTES);
    return result(start, end, snippetRanges(start, end, offset => offset, matches(start, end)));
  }

  if (source.length <= UNICODE_MAPPING_LIMIT) {
    const boundaries = [{source: 0, normalized: 0, bytes: 0}];
    const normalizedParts: string[] = [];
    for (const {segment, index} of graphemes.segment(source)) {
      const normalized = normalize(segment);
      const previous = boundaries[boundaries.length - 1]!;
      normalizedParts.push(normalized);
      boundaries.push({
        source: index + segment.length,
        normalized: previous.normalized + normalized.length,
        bytes: previous.bytes + Buffer.byteLength(segment),
      });
    }
    const foldedLine = line === source ? searchable : searchable.slice(0, -1);
    // Contextual case folds can differ from independently folded graphemes.
    // Use offsets only after proving that both transformations agree.
    if (normalizedParts.join('') === foldedLine) {
      const byMatch = new Map(boundaries.map(boundary => [boundary.normalized, boundary.bytes]));
      const firstByte = byMatch.get(firstMatch) ?? 0;
      const lastByte = byMatch.get(firstMatchEnd) ?? firstByte;
      const wantedStart = snippetWindowStart(firstByte, lastByte - firstByte);
      const start = boundaries.find(boundary => boundary.bytes >= wantedStart)!;
      let end = start;
      for (const boundary of boundaries) {
        if (boundary.bytes > start.bytes + SNIPPET_BYTES) break;
        if (boundary.bytes >= start.bytes) end = boundary;
      }
      return result(start.source, end.source, snippetRanges(start.normalized, end.normalized,
        offset => byMatch.get(offset), matches(start.normalized, end.normalized)));
    }
  }

  // Bound both folding work and output for a long Unicode line. The last
  // grapheme of a clipped prefix could be partial, so leave it out.
  const prefix = source.slice(0, SNIPPET_BYTES + 2).replace(/[\uD800-\uDBFF]$/, '');
  let bytes = 0;
  let end = 0;
  for (const {segment, index} of graphemes.segment(prefix)) {
    const next = index + segment.length;
    if (next === prefix.length && prefix.length < source.length) break;
    bytes += Buffer.byteLength(segment);
    if (bytes > SNIPPET_BYTES) break;
    end = next;
  }
  return result(0, end, []);
}

/** Search is uncached; each request observes the current file list and ignore rules. */
export async function searchProjectFiles(projectRoot: string, query: string, offset = 0, signal?: AbortSignal, caseSensitive = false): Promise<{ paths: string[]; total: number }> {
  validateSearch(query, offset);
  const root = await rootDirectory(projectRoot);
  const candidates = await gitFiles(root, signal) ?? await walk(root, '', signal);
  const matches: string[] = [];
  const needle = searchText(query, caseSensitive);
  for (const candidate of new Set(candidates)) {
    signal?.throwIfAborted();
    const path = candidate.split(sep).join('/');
    // Normalize only for matching. The listed path is also used to open the file.
    if (!validRelative(path) || !searchText(path, caseSensitive).includes(needle)) continue;
    try {
      if (await regularCandidate(root, path)) matches.push(path);
    } catch (error) {
      const code = (error as NodeJS.ErrnoException).code;
      if (code !== 'ENOENT' && code !== 'ENOTDIR') throw error;
    }
  }
  matches.sort((a, b) => a < b ? -1 : a > b ? 1 : 0);
  return { paths: matches.slice(offset, offset + PAGE_SIZE), total: matches.length };
}

export async function listProjectDirectory(projectRoot: string, directory: string, offset = 0) {
  if (directory !== '' && (!validRelative(directory) || directory.includes('\\'))) throw new TypeError('Invalid project file path');
  if (!Number.isSafeInteger(offset) || offset < 0) throw new TypeError('Invalid project file offset');
  const root = await rootDirectory(projectRoot);
  const path = resolve(root, directory);
  if (directory && !inside(root, path)) throw new Error('File is outside project');
  if (await realpath(path) !== path) throw new Error('File is outside project or not a regular file');
  const entries = (await readdir(path, {withFileTypes: true})).flatMap(entry => {
    if (EXCLUDED.has(entry.name) || entry.name.includes('\\')) return [];
    const relativePath = directory ? `${directory}/${entry.name}` : entry.name;
    if (!validRelative(relativePath)) return [];
    if (entry.isDirectory()) return [{path: relativePath, directory: true}];
    if (entry.isFile()) return [{path: relativePath, directory: false}];
    return [];
  });
  if (await realpath(path) !== path) throw new Error('Project file changed while reading');
  entries.sort((a, b) => a.directory !== b.directory ? Number(b.directory) - Number(a.directory) : a.path.localeCompare(b.path, 'en'));
  return {entries: entries.slice(offset, offset + PAGE_SIZE), total: entries.length};
}

export async function searchProjectContents(projectRoot: string, query: string, offset = 0, signal?: AbortSignal, caseSensitive = false, wholeWord = false, regex = false) {
  validateSearch(query, offset);
  const root = await rootDirectory(projectRoot);
  if (!query) return {matches: [], total: 0, limited: false};
  const worker = regex ? await startProjectRegexSearch(query, caseSensitive,
    wholeWord ? WORD_CHARACTER.source : undefined, SNIPPET_RANGE_LIMIT, signal) : undefined;
  try {
    return await searchProjectLines(root, query, offset, worker?.signal ?? signal, caseSensitive, wholeWord, worker);
  } catch (error) {
    worker?.signal.throwIfAborted();
    throw error;
  } finally {
    await worker?.close();
  }
}

async function searchProjectLines(
  root: string, query: string, offset: number, signal: AbortSignal | undefined,
  caseSensitive: boolean, wholeWord: boolean, worker: Awaited<ReturnType<typeof startProjectRegexSearch>> | undefined,
) {
  const needle = worker ? '' : searchText(query, caseSensitive);
  if (!worker && !needle) return {matches: [], total: 0, limited: false};
  const candidates = await gitFiles(root, signal) ?? await walk(root, '', signal);
  const paths = [...new Set(candidates.map(path => path.split(sep).join('/')))].sort();
  const matches: {path: string; line: number; snippet?: SearchSnippet}[] = [];
  let scanned = 0;
  let total = 0;
  let limited = false;
  for (const path of paths) {
    signal?.throwIfAborted();
    if (!validRelative(path) || path.includes('\\')) continue;
    let bytes: Buffer;
    try {
      bytes = await readProjectBytes(root, path, Math.min(PREVIEW_LIMIT, CONTENT_SEARCH_LIMIT - scanned), true);
    } catch (error) {
      // Unreadable or replaced files do not prevent searching other files.
      const code = (error as NodeJS.ErrnoException).code;
      if (error instanceof RangeError) { limited = true; continue; }
      if (error instanceof TypeError || ['ENOENT', 'ENOTDIR', 'EACCES', 'EPERM', 'ELOOP'].includes(code ?? '')
        || (error instanceof Error && (error.message.startsWith('Project file changed') || error.message.startsWith('File is outside project')))) continue;
      throw error;
    }
    scanned += bytes.length;
    let text: string;
    try { text = decodeProjectText(bytes); }
    catch (error) {
      if (error instanceof TypeError) continue;
      throw error;
    }
    signal?.throwIfAborted();
    if (worker) {
      const page = await worker.scan(text, Math.max(0, offset - total), PAGE_SIZE - matches.length);
      for (const {line, start, end, anchor, ranges} of page.lines) {
        const raw = text.slice(start, end);
        const snippet = lineSnippet(raw, raw, ranges[0] ?? [anchor, anchor], segment => segment, () => ranges);
        matches.push({path, line, ...(snippet ? {snippet} : {})});
      }
      total += page.total;
      continue;
    }
    // Walk line boundaries without allocating an array for the entire file.
    for (let start = 0, line = 1; start < text.length; line++) {
      const newline = text.indexOf('\n', start);
      const end = newline === -1 ? text.length : newline;
      if (end > start) {
        const raw = text.slice(start, end);
        const searchable = searchText(raw, caseSensitive);
        const firstMatch = nextLiteralMatch(searchable, needle, 0, wholeWord);
        if (firstMatch !== -1) {
          if (total >= offset && matches.length < PAGE_SIZE) {
            const snippet = lineSnippet(raw, searchable, [firstMatch, firstMatch + needle.length],
              segment => searchText(segment, caseSensitive),
              (from, to) => literalRanges(searchable, needle, from, to, wholeWord));
            matches.push({path, line, ...(snippet ? {snippet} : {})});
          }
          total += 1;
        }
      }
      start = end + 1;
    }
  }
  signal?.throwIfAborted();
  return {matches, total, limited};
}

export async function previewProjectFile(projectRoot: string, relativePath: string) {
  const root = await rootDirectory(projectRoot);
  const bytes = await readProjectBytes(root, relativePath, PREVIEW_LIMIT, true);
  return {text: decodeProjectText(bytes), sha256: createHash('sha256').update(bytes).digest('hex')};
}

function decodeProjectText(bytes: Buffer): string {
  if (bytes.includes(0)) throw new TypeError('Project file is not text');
  try { return new TextDecoder('utf-8', { fatal: true, ignoreBOM: true }).decode(bytes); }
  catch { throw new TypeError('Project file is not UTF-8 text'); }
}

/** Return bounded bytes, never a path for a later unverified read. */
async function readProjectBytes(root: string, relativePath: string, limit: number, direct = false): Promise<Buffer> {
  const canonical = await resolveFileInRoot(root, relativePath);
  const source = resolve(root, relativePath);
  if (direct && (source !== canonical || relativePath.includes('\\'))) throw new Error('File is outside project or not a regular file');
  const handle = await open(canonical, constants.O_RDONLY | (constants.O_NOFOLLOW ?? 0) | (constants.O_NONBLOCK ?? 0));
  try {
    const before = await handle.stat();
    const sourceNow = await realpath(source);
    const current = await stat(sourceNow);
    if (!inside(root, sourceNow) || sourceNow !== canonical || !before.isFile() || before.dev !== current.dev || before.ino !== current.ino)
      throw new Error('Project file changed while opening');
    if (before.size > limit) throw new RangeError(direct ? 'Project file exceeds preview limit' : 'Project file exceeds upload limit');
    const bytes = Buffer.alloc(before.size);
    let offset = 0;
    while (offset < bytes.length) {
      const { bytesRead } = await handle.read(bytes, offset, bytes.length - offset, offset);
      if (!bytesRead) throw new Error('Project file changed while reading');
      offset += bytesRead;
    }
    const after = await handle.stat();
    const finalPath = await realpath(source);
    const finalStat = await stat(finalPath);
    if (!inside(root, finalPath) || finalPath !== canonical || before.dev !== after.dev || before.ino !== after.ino || before.size !== after.size || before.mtimeMs !== after.mtimeMs || after.dev !== finalStat.dev || after.ino !== finalStat.ino || after.size !== finalStat.size || after.mtimeMs !== finalStat.mtimeMs)
      throw new Error('Project file changed while reading');
    return bytes;
  } finally { await handle.close(); }
}

export async function readProjectFile(projectRoot: string, relativePath: string): Promise<{ relativePath: string; bytes: Buffer; sha256: string }> {
  const root = await rootDirectory(projectRoot);
  const bytes = await readProjectBytes(root, relativePath, WORKSPACE_UPLOAD_MAX_BYTES);
  decodeProjectText(bytes);
  return { relativePath: relativePath.split(sep).join('/'), bytes, sha256: createHash('sha256').update(bytes).digest('hex') };
}

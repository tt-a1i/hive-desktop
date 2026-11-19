import { createHash, randomUUID } from 'node:crypto';
import { constants } from 'node:fs';
import { link, lstat, mkdir, open, rm, stat, writeFile } from 'node:fs/promises';
import { extname, join } from 'node:path';
import { ArtifactSourceError, resolveArtifactFile, type ArtifactSourceRequest, type ResolvedArtifact } from './artifact-files.js';

const maxHtmlBytes = 2 * 1024 * 1024;
const maxSnapshotBytes = 6 * maxHtmlBytes + 16 * 1024;
const digest = (bytes: string | Buffer) => createHash('sha256').update(bytes).digest('hex');

export interface HtmlArtifactSnapshot {
  html: string;
  sha256: string;
  sourceSessionId?: string;
  sourceToolCallId?: string;
  warning?: string;
}

async function readBounded(path: string, maxBytes: number, source?: ResolvedArtifact): Promise<string> {
  const flags = constants.O_RDONLY | (process.platform === 'win32' ? 0 : constants.O_NOFOLLOW | constants.O_NONBLOCK);
  const file = await open(path, flags);
  try {
    const before = await file.stat({ bigint: true });
    if (!before.isFile()) throw new ArtifactSourceError('HTML_ARTIFACT_NOT_FILE', 'HTML preview requires a regular file');
    if (before.size > maxBytes) throw new ArtifactSourceError('HTML_ARTIFACT_TOO_LARGE', 'HTML artifact exceeds the 2 MiB preview limit');
    const bytes = Buffer.allocUnsafe(Number(before.size));
    let offset = 0;
    while (offset < bytes.length) {
      const { bytesRead } = await file.read(bytes, offset, bytes.length - offset, offset);
      if (!bytesRead) break;
      offset += bytesRead;
    }
    if (source) {
      const current = await resolveArtifactFile(source.root, path);
      const disk = await stat(current.path, { bigint: true });
      if (current.path !== path || disk.dev !== before.dev || disk.ino !== before.ino) {
        throw new ArtifactSourceError('HTML_ARTIFACT_CHANGED', 'The artifact moved while being read. Preview it again.');
      }
    }
    const after = await file.stat({ bigint: true });
    if (offset !== bytes.length || after.size !== before.size || after.mtimeNs !== before.mtimeNs || after.ctimeNs !== before.ctimeNs) {
      throw new ArtifactSourceError('HTML_ARTIFACT_CHANGED', 'The artifact changed while being read. Preview it again.');
    }
    try { return new TextDecoder('utf-8', { fatal: true }).decode(bytes); }
    catch (error) { throw new ArtifactSourceError('HTML_ARTIFACT_ENCODING', 'HTML preview requires UTF-8 text', { cause: error }); }
  } finally {
    await file.close();
  }
}

async function readSnapshot(path: string, sourceToolCallId?: string): Promise<HtmlArtifactSnapshot | undefined> {
  let data: string;
  try { data = await readBounded(path, maxSnapshotBytes); }
  catch (error) {
    if ((error as NodeJS.ErrnoException).code === 'ENOENT') return undefined;
    throw error;
  }
  let record;
  try { record = JSON.parse(data); }
  catch (error) { throw new ArtifactSourceError('HTML_ARTIFACT_SNAPSHOT_INVALID', 'Saved HTML preview could not be read', { cause: error }); }
  if (record?.version !== 1 || typeof record.html !== 'string' || Buffer.byteLength(record.html) > maxHtmlBytes
    || record.sha256 !== digest(record.html) || record.sourceToolCallId !== sourceToolCallId
    || (record.sourceSessionId !== undefined && typeof record.sourceSessionId !== 'string')
    || (record.warning !== undefined && typeof record.warning !== 'string')) {
    throw new ArtifactSourceError('HTML_ARTIFACT_SNAPSHOT_INVALID', 'Saved HTML preview failed its integrity check');
  }
  return { html: record.html, sha256: record.sha256,
    ...(record.sourceSessionId && { sourceSessionId: record.sourceSessionId }),
    ...(sourceToolCallId && { sourceToolCallId }), ...(record.warning && { warning: record.warning }) };
}

function snapshotLocation(dataDir: string, workspaceId: string, request: ArtifactSourceRequest) {
  if (!['.html', '.htm'].includes(extname(request.path).toLowerCase())) {
    throw new ArtifactSourceError('HTML_ARTIFACT_INVALID_TYPE', 'Interactive preview requires an HTML artifact');
  }
  const directory = join(dataDir, 'html-artifacts', digest(workspaceId));
  // Keep the exact keys used by reports and by older path-only previews.
  const source = [request.path, request.dispatchId, request.sourceAgentId, request.sourceSessionId];
  if (request.sourceToolCallId) source.push(request.sourceToolCallId);
  return {directory, path: join(directory, `${digest(JSON.stringify(source))}.json`)};
}

/** An older path preview has no known tool call. Open it only on explicit request. */
export async function savedHtmlPathPreview(dataDir: string, workspaceId: string, request: ArtifactSourceRequest, check: boolean) {
  if (!request.sourceSessionId || request.sourceToolCallId || request.dispatchId || request.sourceAgentId)
    throw new ArtifactSourceError('ARTIFACT_INVALID_REQUEST', 'An older path preview requires only its original session and path');
  const {path} = snapshotLocation(dataDir, workspaceId, request);
  if (check) {
    try { return {available: (await lstat(path)).isFile()}; }
    catch (error) {
      if ((error as NodeJS.ErrnoException).code === 'ENOENT') return {available: false};
      throw error;
    }
  }
  const saved = await readSnapshot(path);
  if (!saved) throw new ArtifactSourceError('HTML_ARTIFACT_SNAPSHOT_MISSING', 'No older preview was saved for this path');
  return saved;
}

/** The first successful capture for a tool call or report remains immutable. */
export async function snapshotHtmlArtifact(dataDir: string, workspaceId: string, request: ArtifactSourceRequest,
  resolveSource: () => Promise<ResolvedArtifact>): Promise<HtmlArtifactSnapshot> {
  const {directory, path} = snapshotLocation(dataDir, workspaceId, request);
  const saved = await readSnapshot(path, request.sourceToolCallId);
  if (saved) return saved;

  const source = await resolveSource();
  const html = await readBounded(source.path, maxHtmlBytes, source);
  if (html.includes('\0')) throw new ArtifactSourceError('HTML_ARTIFACT_ENCODING', 'HTML preview requires UTF-8 text');
  const result: HtmlArtifactSnapshot = { html, sha256: digest(html),
    ...(source.sourceSessionId && { sourceSessionId: source.sourceSessionId }),
    ...(request.sourceToolCallId && {sourceToolCallId: request.sourceToolCallId}),
    ...(source.warning && { warning: source.warning }) };
  await mkdir(directory, { recursive: true, mode: 0o700 });
  const temporary = join(directory, `${randomUUID()}.part`);
  try {
    await writeFile(temporary, JSON.stringify({ version: 1, ...result }), { flag: 'wx', mode: 0o600, flush: true });
    try { await link(temporary, path); }
    catch (error) {
      if ((error as NodeJS.ErrnoException).code !== 'EEXIST') throw error;
      const concurrent = await readSnapshot(path, request.sourceToolCallId);
      if (concurrent) return concurrent;
      throw error;
    }
    return result;
  } finally {
    await rm(temporary, { force: true });
  }
}

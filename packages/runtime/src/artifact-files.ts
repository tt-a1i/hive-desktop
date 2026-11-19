import { realpath, stat } from 'node:fs/promises';
import { isAbsolute, relative, resolve, sep } from 'node:path';

export interface ArtifactSourceRequest { path: string; dispatchId?: string; sourceAgentId?: string; sourceSessionId?: string; sourceToolCallId?: string }
export interface ResolvedArtifact { path: string; root: string; sourceSessionId?: string; legacy?: boolean; warning?: string }
export class ArtifactSourceError extends Error {
  constructor(readonly code: string, message: string, options?: ErrorOptions) {
    super(`[${code}] ${message}`, options); this.name = 'ArtifactSourceError';
  }
}
export function artifactSourceRequest(value: unknown): ArtifactSourceRequest {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new ArtifactSourceError('ARTIFACT_INVALID_REQUEST', 'Artifact source information is required');
  const item = value as Record<string, unknown>;
  if (Object.keys(item).some(key => !['path', 'dispatchId', 'sourceAgentId', 'sourceSessionId', 'sourceToolCallId'].includes(key)))
    throw new ArtifactSourceError('ARTIFACT_INVALID_REQUEST', 'Artifact directories are resolved from saved source identity, not caller-supplied directories');
  if (typeof item.path !== 'string' || !item.path || item.path.length > 32768 || item.path.includes('\0'))
    throw new ArtifactSourceError('ARTIFACT_INVALID_PATH', 'Supply a nonempty artifact file path');
  for (const key of ['dispatchId', 'sourceAgentId', 'sourceSessionId', 'sourceToolCallId']) if (item[key] !== undefined
    && (typeof item[key] !== 'string' || !item[key] || (item[key] as string).length > 4096 || (item[key] as string).includes('\0')))
    throw new ArtifactSourceError('ARTIFACT_INVALID_REQUEST', 'Artifact source identities must be nonempty strings');
  if (item.sourceToolCallId !== undefined && (!item.sourceSessionId || item.dispatchId || item.sourceAgentId))
    throw new ArtifactSourceError('ARTIFACT_INVALID_REQUEST', 'A tool preview requires its original session and tool call');
  return item as unknown as ArtifactSourceRequest;
}
/** The UI must verify this canonical root and file again when it opens them. */
export async function resolveArtifactFile(executionRoot: string, path: string): Promise<{path: string; root: string}> {
  try {
    const root = await realpath(executionRoot);
    if (!(await stat(root)).isDirectory()) throw new ArtifactSourceError('ARTIFACT_SOURCE_UNAVAILABLE', 'The original execution directory is unavailable');
    const candidate = resolve(root, path);
    const inside = (file: string) => { const part = relative(root, file); return !!part && part !== '..' && !part.startsWith(`..${sep}`) && !isAbsolute(part); };
    if (!inside(candidate)) throw new ArtifactSourceError('ARTIFACT_OUTSIDE_SOURCE', 'Artifact path is outside its original execution directory');
    const canonical = await realpath(candidate);
    if (!inside(canonical)) throw new ArtifactSourceError('ARTIFACT_OUTSIDE_SOURCE', 'Artifact symbolic link points outside its original execution directory');
    if (!(await stat(canonical)).isFile()) throw new ArtifactSourceError('ARTIFACT_NOT_FILE', 'Artifact is not a regular file');
    return {path: canonical, root};
  } catch (error) {
    if (error instanceof ArtifactSourceError) throw error;
    throw new ArtifactSourceError('ARTIFACT_UNAVAILABLE', 'Artifact or its original execution directory is missing or unreadable; no alternate directory was used', {cause: error});
  }
}

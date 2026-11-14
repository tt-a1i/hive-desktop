import { randomUUID } from 'node:crypto';
import { mkdir, readFile, rename, unlink, writeFile } from 'node:fs/promises';
import { join } from 'node:path';

export type ProjectFileReference = { relativePath: string; sha256: string };
export class ProjectFileReferences {
  constructor(private readonly directory: string) {}
  private path(id: string): string {
    if (!/^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/i.test(id)) throw new TypeError('Invalid upload ID');
    return join(this.directory, `${id}.json`);
  }
  async put(workspaceId: string, id: string, projectFile: ProjectFileReference): Promise<void> {
    await mkdir(this.directory, { recursive: true, mode: 0o700 });
    const target = this.path(id);
    const temporary = join(this.directory, `.${randomUUID()}.tmp`);
    try {
      await writeFile(temporary, JSON.stringify({ workspaceId, projectFile }), { mode: 0o600, flag: 'wx' });
      await rename(temporary, target);
    } finally { await unlink(temporary).catch((error: NodeJS.ErrnoException) => { if (error.code !== 'ENOENT') throw error; }); }
  }
  async get(workspaceId: string, id: string): Promise<ProjectFileReference | undefined> {
    let raw: string;
    try { raw = await readFile(this.path(id), 'utf8'); }
    catch (error) { if ((error as NodeJS.ErrnoException).code === 'ENOENT') return undefined; throw error; }
    const value: unknown = JSON.parse(raw);
    if (!value || typeof value !== 'object' || !('workspaceId' in value) || !('projectFile' in value)) throw new Error('Invalid project file reference');
    if (value.workspaceId !== workspaceId) throw new Error('Attachment does not belong to this workspace');
    const file = value.projectFile;
    if (!file || typeof file !== 'object' || !('relativePath' in file) || typeof file.relativePath !== 'string' || !('sha256' in file) || typeof file.sha256 !== 'string' || !/^[a-f0-9]{64}$/.test(file.sha256)) throw new Error('Invalid project file reference');
    return { relativePath: file.relativePath, sha256: file.sha256 };
  }
}

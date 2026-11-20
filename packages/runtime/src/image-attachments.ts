import { WORKSPACE_UPLOAD_MAX_BYTES } from '@tt-a1i/hive/dist/src/server/upload-limits.js';
import { open, stat } from 'node:fs/promises';
import { createReadStream, type BigIntStats } from 'node:fs';
import { createHash } from 'node:crypto';
import { basename, isAbsolute, extname } from 'node:path';
import {
  promptImages,
  imageMimeType,
  type PromptImage,
} from './prompt-images.js';
import { ProjectFileReferences, type ProjectFileReference } from './project-file-references.js';
import type { HiveHost } from './hive-host.js';

export interface ChatAttachment {
  id: string;
  workspaceId: string;
  name: string;
  mimeType: string;
  size: number;
  projectFile?: ProjectFileReference;
}
export interface StagedAttachment {
  attachment: ChatAttachment;
  image?: PromptImage;
  filePath?: string;
}
export interface QuestionAttachmentReference {
  attachment: ChatAttachment;
  filePath: string;
  verifiedCopy?: BigIntStats;
}
const MARKER = '\n\n<!-- hive-desktop-images:v1 ';
const END = ' -->';
export function attachmentIds(value: unknown): string[] {
  if (value === undefined) return [];
  if (
    !Array.isArray(value) ||
    value.length > 10 ||
    value.some(
      (id) =>
        typeof id !== 'string' || !/^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/i.test(id),
    ) ||
    new Set(value).size !== value.length
  )
    throw new TypeError('附件标识必须为最多 10 个不同的上传 ID。');
  return [...value];
}
// Versioned native-client metadata travels in the existing durable user-input
// text. Escaping a literal marker uses an outer empty list, decoded only once.
export function encodeImageInput(text: string, ids: readonly string[]): string {
  const normalized = attachmentIds(ids);
  return normalized.length || text.includes(MARKER) ? text + MARKER + JSON.stringify(normalized) + END : text;
}
export function decodeImageInput(source: string): { text: string; ids: string[] } {
  const at = source.lastIndexOf(MARKER);
  if (at < 0 || !source.endsWith(END)) return { text: source, ids: [] };
  let value: unknown;
  try {
    value = JSON.parse(source.slice(at + MARKER.length, -END.length));
  } catch (error) {
    throw new TypeError('附件元数据无效。', { cause: error });
  }
  const ids = attachmentIds(value);
  const text = source.slice(0, at);
  if (encodeImageInput(text, ids) !== source) throw new TypeError('附件元数据不是规范格式。');
  return { text, ids };
}
function projectCopyChangedMessage(name: string): string {
  return `项目文件附件 ${JSON.stringify(name)} 的副本不可用或已更改，请移除附件后重新选择文件。`;
}
export class ProjectCopyChangedError extends Error {
  constructor(name: string) {
    super(projectCopyChangedMessage(name));
  }
}
export class QuestionAttachmentCopyError extends Error {
  static readonly publicMessage = 'Question attachment copy is unavailable or changed. Remove it and select the file again.';
  constructor(readonly attachmentId: string, projectFileName?: string) {
    super(projectFileName === undefined ? QuestionAttachmentCopyError.publicMessage : projectCopyChangedMessage(projectFileName));
  }
}
export function sameCopy(before: BigIntStats, after: BigIntStats): boolean {
  return before.dev === after.dev && before.ino === after.ino && before.size === after.size
    && before.mtimeNs === after.mtimeNs && before.ctimeNs === after.ctimeNs;
}
async function verifyCopy(filePath: string, size: number, sha256: string, changed: () => Error): Promise<BigIntStats> {
  const file = await open(filePath, 'r');
  try {
    const before = await file.stat({bigint: true});
    if (!before.isFile() || before.size !== BigInt(size)) throw changed();
    const hash = createHash('sha256');
    let read = 0;
    for await (const chunk of createReadStream(filePath, {fd: file, autoClose: false})) {
      const bytes = chunk as Buffer;
      read += bytes.length;
      if (read > size) throw changed();
      hash.update(bytes);
    }
    const after = await file.stat({bigint: true});
    if (read !== size || hash.digest('hex') !== sha256 || !sameCopy(before, after)) throw changed();
    return after;
  } finally { await file.close(); }
}
/** Verify project snapshots without loading the complete copy into memory. */
async function verifyProjectCopy(filePath: string, size: number, reference: ProjectFileReference, name: string): Promise<void> {
  const verified = await verifyCopy(filePath, size, reference.sha256, () => new ProjectCopyChangedError(name));
  if (!sameCopy(verified, await stat(filePath, {bigint: true}))) throw new ProjectCopyChangedError(name);
}
export class ChatAttachments {
  constructor(private readonly hive: HiveHost, private readonly references?: ProjectFileReferences) {}
  async addClipboard(workspaceId: string, value: unknown): Promise<StagedAttachment> {
    const image = promptImages([value])[0]!;
    return this.store(
      workspaceId,
      `粘贴图片.${image.mimeType === 'image/jpeg' ? 'jpg' : image.mimeType.slice(6)}`,
      image,
      Buffer.from(image.data, 'base64'),
    );
  }
  async add(workspaceId: string, path: unknown): Promise<StagedAttachment> {
    if (typeof path !== 'string' || !isAbsolute(path)) throw new TypeError('请选择本地文件的绝对路径。');
    if (!(await stat(path)).isFile()) throw new TypeError('所选路径不是普通文件。');
    const file = await open(path, 'r');
    let bytes: Buffer;
    try {
      const info = await file.stat();
      if (!info.isFile() || info.size > WORKSPACE_UPLOAD_MAX_BYTES) throw new RangeError('单个文件不能超过 100 MB。');
      const buffer = Buffer.alloc(info.size + 1);
      let size = 0;
      while (size < buffer.length) {
        const result = await file.read(buffer, size, buffer.length - size, null);
        if (!result.bytesRead) break;
        size += result.bytesRead;
      }
      if (size > info.size) throw new RangeError('文件在读取时变大，请保存文件后重新选择。');
      bytes = buffer.subarray(0, size);
    } finally { await file.close(); }
    const mimeType = imageMimeType(bytes);
    if (mimeType) {
      const image = promptImages([{type:'image',mimeType,data:bytes.toString('base64')}])[0]!;
      return this.store(workspaceId, basename(path), image, bytes);
    }
    if (['.png','.jpg','.jpeg','.gif','.webp'].includes(extname(path).toLowerCase())) throw new TypeError('图片文件内容与格式不匹配，请检查文件后重试。');
    const record = await this.hive.saveDesktopUpload(workspaceId, basename(path), 'application/octet-stream', bytes);
    const saved = await this.hive.locateUpload(workspaceId,record.id);
    if (!saved || typeof saved.localPath !== 'string' || !isAbsolute(saved.localPath)) throw new Error('文件保存后无法定位，请重新选择。');
    return {attachment:this.ref(workspaceId,record.id,record.originalName,record.mimeType,record.sizeBytes),filePath:saved.localPath};
  }
  async addProjectFile(workspaceId: string, projectFile: ProjectFileReference, bytes: Buffer): Promise<StagedAttachment> {
    if (!this.references) throw new Error('Project file registry unavailable');
    const record = await this.upload(workspaceId, basename(projectFile.relativePath), 'application/octet-stream', bytes.toString('base64'));
    const saved = await this.hive.locateUpload(workspaceId, record.id);
    if (!saved || typeof saved.localPath !== 'string' || !isAbsolute(saved.localPath)) throw new Error('文件保存后无法定位，请重新选择。');
    await this.references.put(workspaceId, record.id, projectFile);
    return { attachment: { ...this.ref(workspaceId, record.id, record.original_name, 'application/octet-stream', record.size_bytes), projectFile }, filePath: saved.localPath };
  }
  async projectReference(workspaceId: string, id: string): Promise<ProjectFileReference | undefined> {
    return this.references?.get(workspaceId, id);
  }
  private ref(workspaceId:string,id:string,name:string,mimeType:string,size:number):ChatAttachment {
    return {id,workspaceId,name,mimeType,size};
  }
  private async upload(workspaceId:string,name:string,mimeType:string,data:string) {
    return await this.hive.request('POST',`/api/workspaces/${encodeURIComponent(workspaceId)}/uploads`,{filename:name,mime_type:mimeType,data}) as {id:string;original_name:string;size_bytes:number};
  }
  private async store(workspaceId: string, name: string, image: PromptImage, bytes: Buffer): Promise<StagedAttachment> {
    const record = await this.hive.saveDesktopUpload(workspaceId, name, image.mimeType, bytes);
    return {attachment:this.ref(workspaceId,record.id,record.originalName,record.mimeType,record.sizeBytes),image};
  }
  /** Internal path references for native question answers, including images.
   * Normal image payloads remain inline image data and do not expose this path. */
  async questionReferences(workspaceId: string, input: unknown): Promise<QuestionAttachmentReference[]> {
    const result: QuestionAttachmentReference[] = [];
    for (const id of attachmentIds(input)) {
      const upload = await this.hive.locateUpload(workspaceId, id);
      if (!upload || typeof upload.localPath !== 'string' || !isAbsolute(upload.localPath)) throw new QuestionAttachmentCopyError(id);
      const {localPath: filePath, sourceDigestVersion, sourceSha256} = upload;
      const {originalName, mimeType, sizeBytes} = upload.record;
      const projectFile = await this.projectReference(workspaceId, id);
      const attachment = {...this.ref(workspaceId, id, originalName, mimeType, sizeBytes), ...(projectFile ? {projectFile} : {})};
      const changed = () => new QuestionAttachmentCopyError(id, projectFile ? originalName : undefined);
      // Only a Hive row explicitly marked as legacy can lack source evidence.
      if (!(sourceDigestVersion === 0 && sourceSha256 === null)
        && !(sourceDigestVersion === 1 && typeof sourceSha256 === 'string' && /^[a-f0-9]{64}$/.test(sourceSha256))) throw changed();
      const sha256 = projectFile?.sha256 ?? sourceSha256;
      try {
        if (sha256) {
          const verifiedCopy = await verifyCopy(filePath, sizeBytes, sha256, changed);
          result.push({attachment, filePath, verifiedCopy});
        } else {
          const info = await stat(filePath);
          if (!info.isFile() || info.size !== sizeBytes) throw changed();
          result.push({attachment, filePath});
        }
      } catch {
        // Local file I/O can expose the upload path. Return only the selected upload ID.
        throw changed();
      }
    }
    return result;
  }
  async get(workspaceId: string, input: unknown): Promise<StagedAttachment[]> {
    const ids = attachmentIds(input);
    const result: StagedAttachment[] = [];
    for (const id of ids) {
      const upload = await this.hive.locateUpload(workspaceId, id);
      if (!upload) throw new Error('附件不存在或不属于当前工作区，请重新选择。');
      if (typeof upload.localPath !== 'string' || !isAbsolute(upload.localPath)) throw new Error('附件存储未提供有效的本地路径。');
      const nativeImage = ['image/png','image/jpeg','image/gif','image/webp'].includes(upload.record.mimeType);
      const bytes = nativeImage ? await this.hive.readUpload(workspaceId,id) : undefined;
      if (nativeImage && !bytes) throw new Error('图片附件无法读取，请重新选择。');
      const image = bytes ? promptImages([{type:'image',mimeType:upload.record.mimeType,data:bytes.data.toString('base64')}])[0] : undefined;
      const projectFile = await this.projectReference(workspaceId, id);
      if (projectFile) await verifyProjectCopy(upload.localPath, upload.record.sizeBytes, projectFile, upload.record.originalName);
      result.push({attachment:{...this.ref(workspaceId,id,upload.record.originalName,upload.record.mimeType,upload.record.sizeBytes), ...(projectFile ? {projectFile} : {})},
        ...(image ? {image} : {filePath:upload.localPath})});
    }
    promptImages(result.flatMap(item=>item.image ? [item.image] : []));
    return result;
  }
}

import { WORKSPACE_UPLOAD_MAX_BYTES } from '@tt-a1i/hive/dist/src/server/upload-limits.js';
import { open, stat } from 'node:fs/promises';
import { basename, isAbsolute, extname } from 'node:path';
import {
  promptImages,
  imageMimeType,
  type PromptImage,
} from './prompt-images.js';
import type { HiveHost } from './hive-host.js';

export interface ChatAttachment {
  id: string;
  workspaceId: string;
  name: string;
  mimeType: string;
  size: number;
}
export interface StagedAttachment {
  attachment: ChatAttachment;
  image?: PromptImage;
  filePath?: string;
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
export class ChatAttachments {
  constructor(private readonly hive: HiveHost) {}
  async addClipboard(workspaceId: string, value: unknown): Promise<StagedAttachment> {
    const image = promptImages([value])[0]!;
    return this.store(
      workspaceId,
      `粘贴图片.${image.mimeType === 'image/jpeg' ? 'jpg' : image.mimeType.slice(6)}`,
      image,
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
      return this.store(workspaceId, basename(path), image);
    }
    if (['.png','.jpg','.jpeg','.gif','.webp'].includes(extname(path).toLowerCase())) throw new TypeError('图片文件内容与格式不匹配，请检查文件后重试。');
    const record = await this.upload(workspaceId,basename(path),'application/octet-stream',bytes.toString('base64'));
    const saved = await this.hive.locateUpload(workspaceId,record.id);
    if (!saved || typeof saved.localPath !== 'string' || !isAbsolute(saved.localPath)) throw new Error('文件保存后无法定位，请重新选择。');
    return {attachment:this.ref(workspaceId,record.id,record.original_name,'application/octet-stream',record.size_bytes),filePath:saved.localPath};
  }
  private ref(workspaceId:string,id:string,name:string,mimeType:string,size:number):ChatAttachment {
    return {id,workspaceId,name,mimeType,size};
  }
  private async upload(workspaceId:string,name:string,mimeType:string,data:string) {
    return await this.hive.request('POST',`/api/workspaces/${encodeURIComponent(workspaceId)}/uploads`,{filename:name,mime_type:mimeType,data}) as {id:string;original_name:string;size_bytes:number};
  }
  private async store(workspaceId: string, name: string, image: PromptImage): Promise<StagedAttachment> {
    const record=await this.upload(workspaceId,name,image.mimeType,image.data);
    return {attachment:this.ref(workspaceId,record.id,record.original_name,image.mimeType,record.size_bytes),image};
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
      result.push({attachment:this.ref(workspaceId,id,upload.record.originalName,upload.record.mimeType,upload.record.sizeBytes),
        ...(image ? {image} : {filePath:upload.localPath})});
    }
    promptImages(result.flatMap(item=>item.image ? [item.image] : []));
    return result;
  }
}

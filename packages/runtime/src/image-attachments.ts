import { open, stat } from 'node:fs/promises';
import { basename, isAbsolute } from 'node:path';
import {
  promptImages,
  imageMimeType,
  MAX_IMAGE_BASE64,
  type PromptImage,
  type ImageMimeType,
} from './prompt-images.js';
import type { HiveHost } from './hive-host.js';

export interface ImageAttachment {
  id: string;
  workspaceId: string;
  name: string;
  mimeType: ImageMimeType;
  size: number;
}
export interface StagedImage {
  attachment: ImageAttachment;
  image: PromptImage;
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
export class ImageAttachments {
  constructor(private readonly hive: HiveHost) {}
  async addClipboard(workspaceId: string, value: unknown): Promise<StagedImage> {
    const image = promptImages([value])[0]!;
    return this.store(
      workspaceId,
      `粘贴图片.${image.mimeType === 'image/jpeg' ? 'jpg' : image.mimeType.slice(6)}`,
      image,
    );
  }
  async add(workspaceId: string, path: unknown): Promise<StagedImage> {
    if (typeof path !== 'string' || !isAbsolute(path)) throw new TypeError('请选择本地图片文件的绝对路径。');
    if (!(await stat(path)).isFile()) throw new TypeError('所选路径不是普通图片文件。');
    const file = await open(path, 'r');
    let bytes: Buffer;
    try {
      const info = await file.stat();
      const limit = Math.floor(MAX_IMAGE_BASE64 / 4) * 3;
      if (!info.isFile() || info.size > limit) throw new RangeError('图片过大，编码后须小于 5 MB。');
      const buffer = Buffer.alloc(limit + 1);
      let size = 0;
      while (size < buffer.length) {
        const result = await file.read(buffer, size, buffer.length - size, null);
        if (!result.bytesRead) break;
        size += result.bytesRead;
      }
      if (size > limit) throw new RangeError('图片在读取时超出大小限制。');
      bytes = buffer.subarray(0, size);
    } finally {
      await file.close();
    }
    const mimeType = imageMimeType(bytes);
    if (!mimeType) throw new TypeError('仅支持 PNG、JPEG、GIF、WebP 图片。');
    const image = promptImages([{ type: 'image', mimeType, data: bytes.toString('base64') }])[0]!;
    return this.store(workspaceId, basename(path), image);
  }
  private async store(workspaceId: string, name: string, image: PromptImage): Promise<StagedImage> {
    const record = (await this.hive.request(
      'POST',
      `/api/workspaces/${encodeURIComponent(workspaceId)}/uploads`,
      { filename: name, mime_type: image.mimeType, data: image.data },
    )) as { id: string; workspace_id: string; original_name: string; size_bytes: number };
    return {
      attachment: {
        id: record.id,
        workspaceId: record.workspace_id,
        name: record.original_name,
        mimeType: image.mimeType,
        size: record.size_bytes,
      },
      image,
    };
  }
  async get(workspaceId: string, input: unknown): Promise<StagedImage[]> {
    const ids = attachmentIds(input);
    const result: StagedImage[] = [];
    for (const id of ids) {
      const upload = await this.hive.readUpload(workspaceId, id);
      if (!upload) throw new Error('图片附件不存在或不属于当前工作区，请重新选择。');
      const image = promptImages([
        { type: 'image', mimeType: upload.record.mimeType, data: upload.data.toString('base64') },
      ])[0]!;
      result.push({
        attachment: {
          id,
          workspaceId,
          name: upload.record.originalName,
          mimeType: image.mimeType,
          size: upload.data.length,
        },
        image,
      });
    }
    promptImages(result.map((item) => item.image));
    return result;
  }
}

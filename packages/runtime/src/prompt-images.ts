export type ImageMimeType = 'image/png' | 'image/jpeg' | 'image/gif' | 'image/webp';
export interface PromptImage { type: 'image'; data: string; mimeType: ImageMimeType }
type TextContent = { type: 'text'; text: string };

// Desktop transport limits, separate from a provider/model's own constraints.
export const MAX_IMAGE_BASE64 = 5_000_000;
export const MAX_TOTAL_IMAGE_BASE64 = 20_000_000;
export const MAX_PROMPT_IMAGES = 10;

export function promptImages(value: unknown): PromptImage[] {
  if (value === undefined) return [];
  if (!Array.isArray(value) || value.length > MAX_PROMPT_IMAGES) throw new TypeError('图片列表最多包含 10 项。');
  let total = 0;
  return value.map(image => {
    if (!image || typeof image !== 'object' || image.type !== 'image' || typeof image.data !== 'string'
      || typeof image.mimeType !== 'string') throw new TypeError('图片需包含类型、MIME 与 base64 内容。');
    const {data, mimeType} = image;
    total += data.length;
    if (!data.length || data.length > MAX_IMAGE_BASE64 || total > MAX_TOTAL_IMAGE_BASE64) throw new RangeError('单张图片的 base64 最多 5 MB，单次图片合计最多 20 MB。');
    const bytes = Buffer.from(data, 'base64');
    if (bytes.toString('base64') !== data) throw new TypeError('图片内容不是规范的 base64。');
    const valid = mimeType === 'image/png' ? bytes.subarray(0,8).equals(Buffer.from([137,80,78,71,13,10,26,10]))
      : mimeType === 'image/jpeg' ? bytes[0]===255 && bytes[1]===216 && bytes[2]===255
      : mimeType === 'image/gif' ? ['GIF87a','GIF89a'].includes(bytes.subarray(0,6).toString('ascii'))
      : mimeType === 'image/webp' ? bytes.subarray(0,4).toString('ascii')==='RIFF' && bytes.subarray(8,12).toString('ascii')==='WEBP'
      : false;
    if (!valid) throw new TypeError('图片格式与内容不匹配，仅支持 PNG、JPEG、GIF、WebP。');
    return {type:'image', data, mimeType};
  });
}

export function imageMessage(text: string, images: readonly PromptImage[]): string | (TextContent | PromptImage)[] {
  return images.length ? [...(text ? [{type:'text' as const,text}] : []), ...images] : text;
}

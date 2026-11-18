/** Projection for the desktop reader. Native session storage is left intact. */
function object(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function image(block: unknown): block is Record<string, unknown> & {data: string} {
  return object(block) && block.type === 'image' && typeof block.data === 'string';
}

/** Keep the display content and the native fields the desktop consumes. */
export function desktopHistoryMessage(message: unknown, messageId: string | undefined): unknown {
  if (!object(message) || (typeof message.content !== 'string' && !Array.isArray(message.content))) return message;
  const raw = message.rawBackendItem;
  const native = object(raw) ? {rawBackendItem: {type: raw.type, id: raw.id, hiveInterrupted: raw.hiveInterrupted}} : undefined;
  if (!messageId || !Array.isArray(message.content)) return native ? {...message, ...native} : message;
  let count = 0;
  const content = message.content.map((block: unknown) => {
    if (!image(block)) return block;
    count++;
    const {data: _data, ...metadata} = block;
    return metadata;
  });
  if (count === 0) return native ? {...message, ...native} : message;
  return {...message, ...native, content, desktopHistoryImages: {messageId, count}};
}

export function desktopHistoryImage(message: unknown, imageIndex: number): string | undefined {
  if (!object(message) || !Array.isArray(message.content)) return undefined;
  for (const block of message.content) {
    if (image(block) && imageIndex-- === 0) return block.data;
  }
  return undefined;
}

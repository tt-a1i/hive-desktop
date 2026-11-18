function object(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function image(block: unknown): block is Record<string, unknown> & {data: string} {
  return object(block) && block.type === 'image' && typeof block.data === 'string';
}

/** Keep image bytes in the history source until the desktop asks for one. */
export function deferDesktopHistoryImages(message: unknown, messageId: string | undefined): unknown {
  if (!messageId || !object(message) || !Array.isArray(message.content)) return message;
  let count = 0;
  const content = message.content.map((block: unknown) => {
    if (!image(block)) return block;
    count++;
    const {data: _data, ...metadata} = block;
    return metadata;
  });
  if (count === 0) return message;
  const raw = message.rawBackendItem;
  // Codex can include the same image again in the native item. The desktop
  // reads only its edit boundary and interruption metadata.
  const rawBackendItem = object(raw) ? {type: raw.type, id: raw.id, hiveInterrupted: raw.hiveInterrupted} : raw;
  return {...message, content, rawBackendItem, desktopHistoryImages: {messageId, count}};
}

export function desktopHistoryImage(message: unknown, imageIndex: number): string | undefined {
  if (!object(message) || !Array.isArray(message.content)) return undefined;
  for (const block of message.content) {
    if (image(block) && imageIndex-- === 0) return block.data;
  }
  return undefined;
}

import { decodeImageInput, encodeImageInput } from './image-attachments.js';
import type { DeliveryItem } from './delivery-queue.js';

/** Preserve the original file context, Hive instructions, image bytes and upload IDs. */
export function replaceQueuedInput(item: DeliveryItem, text: string): DeliveryItem {
  const presentation = item.presentation;
  if (presentation?.origin !== 'user' || !item.text.startsWith(presentation.displayText)) {
    throw new Error('Queued input has no safe text replacement boundary');
  }
  const ids = decodeImageInput(presentation.sourceText ?? presentation.displayText).ids;
  const sourceText = encodeImageInput(text, ids);
  return { ...item, text: text + item.text.slice(presentation.displayText.length),
    presentation: { ...presentation, displayText: text, sourceText } };
}

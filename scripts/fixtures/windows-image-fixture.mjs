import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { deflateSync } from 'node:zlib';

export const digest = data => createHash('sha256').update(data).digest('hex');

function crc32(bytes) {
  let crc = 0xffffffff;
  for (const byte of bytes) {
    crc ^= byte;
    for (let bit = 0; bit < 8; bit++) {
      crc = (crc >>> 1) ^ (0xedb88320 & -(crc & 1));
    }
  }
  return (crc ^ 0xffffffff) >>> 0;
}

function chunk(name, data) {
  const type = Buffer.from(name);
  const length = Buffer.alloc(4);
  const crc = Buffer.alloc(4);
  length.writeUInt32BE(data.length);
  crc.writeUInt32BE(crc32(Buffer.concat([type, data])));
  return Buffer.concat([length, type, data, crc]);
}

function png(width, height, seed) {
  const stride = width * 3 + 1;
  const raw = Buffer.alloc(stride * height);
  let value = seed >>> 0;
  for (let y = 0; y < height; y++) {
    for (let column = 1; column < stride; column++) {
      value ^= value << 13;
      value ^= value >>> 17;
      value ^= value << 5;
      raw[y * stride + column] = value & 255;
    }
  }
  const header = Buffer.alloc(13);
  header.writeUInt32BE(width);
  header.writeUInt32BE(height, 4);
  header[8] = 8;
  header[9] = 2;
  return Buffer.concat([
    Buffer.from('89504e470d0a1a0a', 'hex'),
    chunk('IHDR', header),
    chunk('IDAT', deflateSync(raw)),
    chunk('IEND', Buffer.alloc(0)),
  ]);
}

export function images() {
  return [png(848, 569, 12345), png(900, 650, 67890), png(992, 684, 24680)];
}

export function imageIndex(row) {
  if (row === 0) return 0;
  if (row === 47) return 2;
  return 1;
}

export function verifyImages(manager, originals) {
  const messages = manager.buildSessionContext().messages.filter(message =>
    Array.isArray(message.content) && message.content.some(block => block.type === 'image'));
  assert.equal(messages.length, 48);
  for (const [index, message] of messages.entries()) {
    const image = message.content.find(block => block.type === 'image');
    assert.deepEqual(Buffer.from(image.data, 'base64'), originals[imageIndex(index)]);
  }
}

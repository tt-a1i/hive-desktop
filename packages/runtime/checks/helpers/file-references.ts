import assert from 'node:assert/strict';

interface FileReference {
  name: string;
  path: string;
  size: number;
}

// Attachment context contains one JSON metadata object per text line. Decode
// that inner JSON too, so Windows paths are compared as paths, not JSON bytes.
export function fileReferences(value: unknown): FileReference[] {
  if (typeof value === 'string') {
    const references: FileReference[] = [];
    for (const line of value.split('\n')) {
      let parsed: unknown;
      try { parsed = JSON.parse(line); } catch { continue; }
      if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)
        && 'name' in parsed && typeof parsed.name === 'string'
        && 'path' in parsed && typeof parsed.path === 'string'
        && 'size' in parsed && typeof parsed.size === 'number') {
        references.push({ name: parsed.name, path: parsed.path, size: parsed.size });
      }
    }
    return references;
  }
  if (Array.isArray(value)) return value.flatMap(fileReferences);
  if (value && typeof value === 'object') return Object.values(value).flatMap(fileReferences);
  return [];
}

export function assertFileReference(value: unknown, uploaded: {
  filePath?: string;
  attachment: { name: string; size: number };
}): void {
  assert.ok(uploaded.filePath, 'Uploaded file must have a saved copy path');
  const expected = { name: uploaded.attachment.name, path: uploaded.filePath, size: uploaded.attachment.size };
  assert.ok(fileReferences(value).some(reference => reference.name === expected.name
    && reference.path === expected.path && reference.size === expected.size),
  `Missing exact attachment reference: ${JSON.stringify(expected)}`);
}

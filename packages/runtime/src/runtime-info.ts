import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';

export interface RuntimeInfo {
  version: string | null;
  nodeVersion: string;
  hiveVersion: string | null;
  platform: string;
  arch: string;
}

function versionAt(path: string | URL, expectedName: string): string | null {
  try {
    const metadata: unknown = JSON.parse(readFileSync(path, 'utf8'));
    if (!metadata || typeof metadata !== 'object' || Array.isArray(metadata)) return null;
    const { name, version } = metadata as Record<string, unknown>;
    return name === expectedName && typeof version === 'string' && version.trim() ? version : null;
  } catch {
    return null;
  }
}

export function captureRuntimeInfo(): RuntimeInfo {
  let hiveVersion: string | null = null;
  try {
    hiveVersion = versionAt(createRequire(import.meta.url).resolve('@tt-a1i/hive/package.json'), '@tt-a1i/hive');
  } catch {
    // Missing or inaccessible package metadata must not prevent startup.
  }
  return {
    version: versionAt(new URL('../package.json', import.meta.url), '@hive-desktop/runtime'),
    nodeVersion: process.version,
    hiveVersion,
    platform: process.platform,
    arch: process.arch,
  };
}

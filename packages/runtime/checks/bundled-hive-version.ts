/** Check the embedded host's version route with all external fetches denied. */
import assert from 'node:assert/strict';
import {mkdtemp, rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {createHiveHost} from '../src/hive-host.js';

const root = await mkdtemp(join(tmpdir(), 'hive-embedded-version-'));
const externalRequests: string[] = [];
const fetch = globalThis.fetch;
globalThis.fetch = (input, init) => {
  const url = new URL(input instanceof Request ? input.url : input);
  if (url.hostname === '127.0.0.1') return fetch(input, init);
  externalRequests.push(url.origin);
  throw new Error('External request denied by the offline check');
};
let host: Awaited<ReturnType<typeof createHiveHost>> | undefined;
try {
  host = await createHiveHost({dataDir: root});
  const version = await host.request('GET', '/api/version') as Record<string, unknown>;
  assert.equal(version.package_name, '@tt-a1i/hive');
  assert.match(String(version.current_version), /^\d+\.\d+\.\d+/);
  assert.equal(version.latest_version, version.current_version);
  assert.equal(version.can_run_hive_update, false);
  assert.equal(version.install_hint, '');
  assert.equal(version.update_available, false);
  assert.deepEqual(externalRequests, []);
  console.log('BUNDLED_HIVE_VERSION_OK: offline start and version query, no CLI update hint.');
} finally {
  try { await host?.close(); } finally {
    globalThis.fetch = fetch;
    await rm(root, {recursive: true, force: true});
  }
}

import assert from "node:assert/strict";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createHiveHost, HiveHttpError } from "../src/hive-host.js";

const root = await mkdtemp(join(tmpdir(), "hive-desktop-autostaff-check-"));
const dataDir = join(root, "state");
const path = "/api/settings/team-autostaff";
let host: Awaited<ReturnType<typeof createHiveHost>> | undefined;
try {
  host = await createHiveHost({ dataDir });
  assert.deepEqual(await host.request("GET", path), { enabled: true });
  assert.deepEqual(await host.request("PUT", path, { enabled: false }), { enabled: false });
  assert.deepEqual(await host.request("GET", path), { enabled: false });
  await assert.rejects(host.request("PUT", path, { enabled: "true" }),
    (error: unknown) => error instanceof HiveHttpError && error.status === 400);
  assert.deepEqual(await host.request("GET", path), { enabled: false });
  await host.close();
  host = await createHiveHost({ dataDir });
  assert.deepEqual(await host.request("GET", path), { enabled: false });
  assert.deepEqual(await host.request("PUT", path, { enabled: true }), { enabled: true });
  await host.close();
  host = await createHiveHost({ dataDir });
  assert.deepEqual(await host.request("GET", path), { enabled: true });
  console.error("Team autostaff real HTTP + SQLite persistence check passed");
} finally {
  await host?.close();
  await rm(root, { recursive: true, force: true });
}

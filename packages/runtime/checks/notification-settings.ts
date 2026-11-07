import assert from "node:assert/strict";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createHiveHost } from "../src/hive-host.js";

const root = await mkdtemp(join(tmpdir(), "hive-desktop-notification-check-"));
const dataDir = join(root, "state");
const key = "notifications.webhook-url";
const path = `/api/settings/app-state/${key}`;
const value = "http://127.0.0.1:9/synthetic";
let host: Awaited<ReturnType<typeof createHiveHost>> | undefined;
try {
  host = await createHiveHost({ dataDir });
  assert.equal((await host.request("GET", path) as { value: unknown }).value, null);
  assert.equal(await host.request("PUT", path, { value }), null);
  assert.equal((await host.request("GET", path) as { value: unknown }).value, value);
  await host.close();
  host = await createHiveHost({ dataDir });
  assert.equal((await host.request("GET", path) as { value: unknown }).value, value);
  assert.equal(await host.request("PUT", path, { value: "" }), null);
  await host.close();
  host = await createHiveHost({ dataDir });
  assert.equal((await host.request("GET", path) as { value: unknown }).value, "");
  console.error("Notification settings real HTTP + SQLite persistence check passed");
} finally {
  await host?.close();
  await rm(root, { recursive: true, force: true });
}

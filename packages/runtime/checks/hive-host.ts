import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdtemp, realpath, readFile, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createHiveHost, HiveHttpError } from "../src/hive-host.js";

const root = await mkdtemp(join(tmpdir(), "hive-desktop-host-check-"));
const previousDataDir = process.env.HIVE_DATA_DIR;
let host: Awaited<ReturnType<typeof createHiveHost>> | undefined;
try {
	host = await createHiveHost({ dataDir: join(root, "state") });
	assert.deepEqual(await host.request("GET", "/api/workspaces"), []);
	await assert.rejects(createHiveHost({ dataDir: join(root, "other") }));
	await assert.rejects(
		host.request("GET", "https://example.com/api/workspaces"),
	);
	await assert.rejects(host.request("GET", "/api/../outside"));
	await assert.rejects(
		host.request("POST", "/api/workspaces", {
			path: join(root, "missing"),
			autostart_orchestrator: false,
		}),
		(error: unknown) => error instanceof HiveHttpError && error.status === 400,
	);
	const path = await realpath(root);
	const created = (await host.request("POST", "/api/workspaces", {
		path,
		name: "Desktop host check",
		autostart_orchestrator: false,
	})) as {
		id: string;
		path: string;
		name: string;
		orchestrator_start: { run_id: string | null };
	};
	assert.equal(typeof created.id, "string");
	assert.equal(created.path, path);
	assert.equal(created.name, "Desktop host check");
	assert.equal(created.orchestrator_start.run_id, null);
	const listed = (await host.request("GET", "/api/workspaces")) as Array<{
		id: string;
		path: string;
	}>;
	assert.equal(listed.length, 1);
	assert.equal(listed[0]?.id, created.id);
	assert.equal(listed[0]?.path, path);
	const tasks = `/api/workspaces/${created.id}/tasks`;
	const file = join(path, ".hive", "tasks.md");
	const hash = (value: string) => createHash("sha256").update(value).digest("hex");
	await host.request("PUT", tasks, { content: "共同底稿" });
	const base = (await host.request("GET", tasks) as { content: string }).content;
	const candidates = ["第一份修改", "第二份修改"];
	const writes = await Promise.allSettled(candidates.map(content =>
		host!.request("PUT", tasks, { content, base_sha256: hash(base) })));
	const winner = writes.findIndex(result => result.status === "fulfilled");
	assert.ok(winner >= 0);
	const loser = writes[1 - winner]!;
	assert.equal(loser.status, "rejected", "Only one writer may replace the same base");
	assert.ok(loser.reason instanceof HiveHttpError && loser.reason.status === 409);
	assert.equal(await readFile(file, "utf8"), candidates[winner]);
	await writeFile(file, "Agent 的新进展");
	await assert.rejects(host.request("PUT", tasks, { content: "旧草稿", base_sha256: hash(candidates[winner]!) }), { status: 409 });
	assert.equal(await readFile(file, "utf8"), "Agent 的新进展");
	const port = host.port;
	await Promise.all([host.close(), host.close()]);
	assert.equal(process.env.HIVE_DATA_DIR, previousDataDir);
	await assert.rejects(fetch(`http://127.0.0.1:${port}/api/ui/session`));
	await assert.rejects(host.request("GET", "/api/workspaces"));
	console.error("Hive host real HTTP + SQLite check passed");
} finally {
	await host?.close();
	await rm(root, { recursive: true, force: true });
}

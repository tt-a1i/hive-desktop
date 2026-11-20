import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdtemp, realpath, readFile, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { DispatchMessageRecord } from "@tt-a1i/hive/dist/src/shared/team-collaboration.js";
import { createHiveHost, HiveHttpError, withInboxDispatchBackends, withInboxMessageSenders } from "../src/hive-host.js";

const olderQuestion = {kind: 'question_waiting_answer', message_id: 'older-question', dispatch_id: 'first-dispatch'};
const snapshot = {
	workspace_id: 'workspace-a', recent_dispatch_messages: [],
	attention: [olderQuestion, {...olderQuestion, dispatch_id: 'other-dispatch'}, {...olderQuestion, message_id: 'other-workspace'}],
};
const message: DispatchMessageRecord = {
	id: 'older-question', workspaceId: 'workspace-a', dispatchId: 'first-dispatch',
	sourceDispatchId: null, sequence: 1, fromAgentId: 'worker', recipientAgentId: 'orchestrator',
	kind: 'question', replyTo: null, text: '', createdAt: 1, deliveryState: 'delivered',
	deliveredAt: 1, deliveryError: null,
};
const source = {listActionableDispatchMessages(workspaceId: string) {
	assert.equal(workspaceId, 'workspace-a');
	return [message, {...message, id: 'other-workspace', workspaceId: 'workspace-b'}];
}};
assert.deepEqual(withInboxMessageSenders(snapshot, 'workspace-a', source), {
	...snapshot,
	attention: [{...olderQuestion, message_sender_id: 'worker', message_created_at: 1}, ...snapshot.attention.slice(1)],
});
assert.equal(withInboxMessageSenders(snapshot, 'workspace-b', source), snapshot);
const dispatchId = '123e4567-e89b-12d3-a456-426614174000';
const unattributed = '123e4567-e89b-12d3-a456-426614174001';
const backendSnapshot = {...snapshot, attention: [
	{...olderQuestion, dispatch_id: dispatchId, message_sender_id: 'worker'},
	{kind: 'message_delivery_pending', dispatch_id: dispatchId},
	{kind: 'dispatch_waiting_report', dispatch_id: unattributed, dispatch_backend: 'pi'},
	{kind: 'stopped_with_queue', worker_id: 'worker', dispatch_backend: 'pi'},
]};
const lookups: [string, string][] = [];
assert.deepEqual(withInboxDispatchBackends(backendSnapshot, 'workspace-a', (workspace, id) => {
	lookups.push([workspace, id]);
	return id === dispatchId ? 'codex' : undefined;
}), {...backendSnapshot, attention: backendSnapshot.attention.map((row, index) => ({
	...row, dispatch_backend: index < 2 ? 'codex' : null,
}))});
assert.deepEqual(lookups, [['workspace-a', dispatchId], ['workspace-a', unattributed]]);

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
	await assert.rejects(
		host.request("POST", "/api/workspaces", {
			path,
			name: "Duplicate project",
			autostart_orchestrator: false,
		}),
		(error: unknown) => {
			assert.ok(error instanceof HiveHttpError);
			assert.equal(error.status, 409);
			assert.equal(typeof error.body, "object");
			const detail = (error.body as { error: string }).error;
			assert.equal(detail, `Workspace path already exists: ${path}`);
			assert.equal(error.message, `Hive request failed (409): ${detail}`);
			return true;
		},
	);
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

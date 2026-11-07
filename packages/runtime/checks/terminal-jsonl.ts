/** Actual desktop JSONL process, Hive shell and PTY; no model calls. */
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createInterface } from "node:readline";
import { mkdir, mkdtemp, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { randomUUID } from "node:crypto";
if (process.platform === "win32")
	throw new Error("This POSIX PTY check has not been validated on Windows");
const dir = await mkdtemp(join(tmpdir(), "hive-terminal-jsonl-"));
const project = join(dir, "project");
await mkdir(project);
const child = spawn(process.execPath, ["--import", "tsx", "src/main.ts"], {
	cwd: process.cwd(),
	env: { ...process.env, HIVE_DESKTOP_DATA_DIR: join(dir, "state") },
	stdio: ["pipe", "pipe", "pipe"],
});
let stderr = "";
child.stderr.on("data", (v) => (stderr += String(v)));
let id = 0;
const pending = new Map<
	number,
	{
		resolve: (v: any) => void;
		reject: (e: Error) => void;
	}
>();
const events: any[] = [];
createInterface({ input: child.stdout }).on("line", (line) => {
	const m = JSON.parse(line);
	if (typeof m.id === "number") {
		const p = pending.get(m.id);
		if (p) {
			pending.delete(m.id);
			p.resolve(m);
		}
	} else events.push(m);
});
child.on("exit", () => {
	for (const p of pending.values())
		p.reject(new Error("Sidecar exited: " + stderr));
	pending.clear();
});
const rpc = (method: string, params: unknown = {}) =>
	new Promise<any>((resolve, reject) => {
		const n = ++id;
		pending.set(n, { resolve, reject });
		child.stdin.write(JSON.stringify({ id: n, method, params }) + "\n");
	});
const wait = async <T>(f: () => T, ok: (v: T) => boolean) => {
	const end = Date.now() + 10000;
	for (;;) {
		const v = f();
		if (ok(v)) return v;
		if (Date.now() > end) throw Error("JSONL event timeout");
		await new Promise((r) => setTimeout(r, 10));
	}
};
try {
	assert.ok((await rpc("initialize")).result.hivePort);
	const create = await rpc("hive.request", {
		method: "POST",
		path: "/api/workspaces",
		body: { path: project, name: "JSONL shell", autostart_orchestrator: false },
	});
	const workspaceId = create.result.id;
	const start = await rpc("terminal.start", { workspaceId });
	const runId = start.result.runId;
	assert.ok(runId);
	const attachmentId = randomUUID();
	const attached = await rpc("terminal.attach", {
		workspaceId,
		runId,
		attachmentId,
		cols: 80,
		rows: 24,
	});
	assert.equal(attached.result.ok, true);
	const restore = await wait(
		() =>
			events
				.map((e) => e.data)
				.find((e) => e?.type === "restore" && e.attachmentId === attachmentId),
		Boolean,
	);
	assert.equal(
		(
			await rpc("terminal.restoreComplete", {
				attachmentId,
				sequence: restore.sequence,
			})
		).result.ok,
		true,
	);
	const tag = "JSONL_" + randomUUID();
	const octal = [...Buffer.from(tag)]
		.map((n) => "\\" + n.toString(8).padStart(3, "0"))
		.join("");
	assert.equal(
		(
			await rpc("terminal.input", {
				attachmentId,
				bytes: [...Buffer.from(`stty -echo; printf '${octal}\\n'\n`)],
			})
		).result.ok,
		true,
	);
	let position = 0;
	let output = Buffer.alloc(0);
	await wait(
		() => {
			for (; position < events.length; position++) {
				const e = events[position].data;
				if (e?.attachmentId === attachmentId && e.type === "output")
					output = Buffer.concat([output, Buffer.from(e.bytes)]);
			}
			return output;
		},
		(v) => v.includes(Buffer.from(tag)),
	);
	assert.equal(
		(await rpc("terminal.stop", { workspaceId, runId })).result.ok,
		true,
	);
	assert.equal((await rpc("shutdown")).result.ok, true);
	await writeFile(
		join(dir, "receipt.json"),
		JSON.stringify(
			{
				pass: true,
				noModel: true,
				actualJsonl: true,
				realPty: true,
				realHive: true,
				restoreAndInputObserved: true,
				scope: "separate process; does not assert GUI rendering",
			},
			null,
			2,
		),
	);
	console.log("Evidence:", dir);
} catch (error) {
	await writeFile(
		join(dir, "failure.json"),
		JSON.stringify(
			{ error: String(error), stack: (error as Error).stack, stderr },
			null,
			2,
		),
	);
	console.error("Evidence:", dir);
	throw error;
} finally {
	child.stdin.end();
	child.kill();
}

/** Real Hive PTY byte input; raw-mode child records stdin, no model. */
import assert from "node:assert/strict";
import { mkdtemp, mkdir, writeFile, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { randomUUID } from "node:crypto";
import { createDesktopRuntime } from "../src/desktop-runtime.js";
import type { TerminalEvent } from "../src/terminal-connection.js";
if (process.platform === "win32")
	throw new Error("POSIX raw PTY check not validated on Windows");
const dir = await mkdtemp(join(tmpdir(), "hive-terminal-input-bytes-"));
const project = join(dir, "project");
await mkdir(project);
const target = join(dir, "recorded.bin"),
	script = join(project, "owned-recorder.cjs"),
	tag = randomUUID().replaceAll("-", "");
const ready = "RAW_READY_" + tag,
	done = "RAW_DONE_" + tag;
const chinese = Buffer.from("中文蜂巢", "utf8");
assert.equal(chinese.toString("hex"), "e4b8ade69687e89c82e5b7a2");
const payload = Buffer.concat([
	chinese,
	Buffer.from("\x1b[200~"),
	Buffer.from("粘贴", "utf8"),
	Buffer.from("\x1b[201~"),
	Buffer.from([3, 27, 91, 77, 0x80, 0xff, 0x20]),
]);
await writeFile(
	script,
	`const fs=require('node:fs');const target=${JSON.stringify(target)},size=${payload.length};let chunks=[],n=0;process.stdin.setRawMode(true);process.stdin.resume();process.stdout.write(${JSON.stringify(ready)}+'\\n');process.stdin.on('data',b=>{chunks.push(b);n+=b.length;if(n>=size){process.stdin.pause();fs.writeFileSync(target,Buffer.concat(chunks));process.stdout.write(${JSON.stringify(done)}+'\\n');process.exit(0)}});\n`,
);
const events: TerminalEvent[] = [];
const runtime = await createDesktopRuntime(join(dir, "state"), () => {}, {
	executableEnvironment: {},
	onTerminalEvent: (event) => events.push(event),
});
const term = runtime.terminal;
const attachmentId = randomUUID();
let readIndex = 0,
	output = Buffer.alloc(0);
const wait = async <T>(
	f: () => T | Promise<T>,
	ok: (x: T) => boolean,
	label: string,
) => {
	const end = Date.now() + 10000;
	for (;;) {
		const x = await f();
		if (ok(x)) return x;
		if (Date.now() > end) throw Error("Timed out: " + label);
		await new Promise((r) => setTimeout(r, 10));
	}
};
const collect = async () => {
	for (; readIndex < events.length; readIndex++) {
		const e = events[readIndex]!;
		if (e.type === "output" && e.attachmentId === attachmentId) {
			const b = Buffer.from(e.bytes);
			output = Buffer.concat([output, b]);
			await term.ack({ attachmentId, sequence: e.sequence, bytes: b.length });
		}
	}
	return output;
};
try {
	const ws = (await runtime.hive.request("POST", "/api/workspaces", {
		path: project,
		name: "Raw input check",
		autostart_orchestrator: false,
	})) as {
		id: string;
	};
	const run = await term.start(ws.id);
	await term.attach({
		workspaceId: ws.id,
		runId: run.runId,
		attachmentId,
		cols: 80,
		rows: 24,
	});
	const restore = await wait(
		() =>
			events.find(
				(e) => e.type === "restore" && e.attachmentId === attachmentId,
			),
		Boolean,
		"restore",
	);
	assert.ok(restore && restore.type === "restore");
	await term.restoreComplete({ attachmentId, sequence: restore.sequence });
	const command = `stty -echo; node '${script.replaceAll("'", "'\\''")}'\n`;
	await term.input({ attachmentId, bytes: [...Buffer.from(command)] });
	await wait(
		collect,
		(b) => b.includes(Buffer.from(ready)),
		"raw recorder ready",
	);
	await term.input({ attachmentId, bytes: [...payload] });
	await wait(
		collect,
		(b) => b.includes(Buffer.from(done)),
		"raw recorder done",
	);
	const recorded = await readFile(target);
	assert.deepEqual(recorded, payload);
	await term.stop({ workspaceId: ws.id, runId: run.runId });
	await writeFile(
		join(dir, "receipt.json"),
		JSON.stringify(
			{
				pass: true,
				noModel: true,
				realHiveHttpSqlite: true,
				realPty: true,
				realTerminalBridge: true,
				platform: process.platform,
				rawInputHex: recorded.toString("hex"),
				chineseInputHex: chinese.toString("hex"),
				bracketedPaste: true,
				controlAndHighBitMouseBytes: true,
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
			{
				error: String(error),
				stack: (error as Error).stack,
				events: events.map((e) =>
					e.type === "output" || e.type === "restore"
						? { ...e, bytes: `<${e.bytes.length} bytes>` }
						: e,
				),
			},
			null,
			2,
		),
	);
	console.error("Evidence:", dir);
	throw error;
} finally {
	await runtime.close();
}

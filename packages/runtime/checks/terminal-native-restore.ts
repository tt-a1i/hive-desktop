/** Real Hive PTY + native paired sockets across three observable output stages. */
import assert from "node:assert/strict";
import { once } from "node:events";
import WebSocket from "ws";
import { mkdtemp, mkdir, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { randomUUID } from "node:crypto";
import { createDesktopRuntime } from "../src/desktop-runtime.js";
if (process.platform === "win32")
	throw new Error("POSIX shell check not validated on Windows");
const directory = await mkdtemp(
	join(tmpdir(), "hive-terminal-native-restore-"),
);
const project = join(directory, "project");
await mkdir(project);
const runtime = await createDesktopRuntime(join(directory, "state"), () => {}, {
	executableEnvironment: {},
});
const sockets: ReturnType<typeof runtime.hive.openTerminalSocket>[] = [];
const token = randomUUID().replaceAll("-", "").slice(0, 12);
const history = "HIST_" + token,
	between = "BETW_" + token,
	after = "AFTER_" + token,
	controlFirst = "CTRL_" + token;
const octal = (s: string) =>
	[...Buffer.from(s)]
		.map((x) => "\\" + x.toString(8).padStart(3, "0"))
		.join("");
const wait = async <T>(f: () => T, ok: (v: T) => boolean) => {
	const end = Date.now() + 10000;
	for (;;) {
		const v = f();
		if (ok(v)) return v;
		if (Date.now() > end) throw new Error("Native socket timed out");
		await new Promise((r) => setTimeout(r, 10));
	}
};
const snapshotFor = (
	socket: ReturnType<typeof runtime.hive.openTerminalSocket>,
) =>
	new Promise<string>((resolve, reject) => {
		socket.on("message", (raw) => {
			const x = JSON.parse(raw.toString());
			if (x.type === "restore") resolve(x.snapshot);
		});
		socket.once("error", reject);
	});
try {
	const ws = (await runtime.hive.request("POST", "/api/workspaces", {
		path: project,
		name: "native restore boundary",
		autostart_orchestrator: false,
	})) as {
		id: string;
	};
	const run = await runtime.terminal.start(ws.id);
	const open = (kind: "io" | "control", client: string) => {
		const s = runtime.hive.openTerminalSocket(run.runId, kind, client, 120, 40);
		sockets.push(s);
		return s;
	};
	// Establish genuine history before the tested IO socket exists.
	const preId = randomUUID(),
		preIo = open("io", preId);
	let preData = "";
	preIo.on("message", (raw) => {
		preData += raw.toString();
	});
	await once(preIo, "open");
	const preControl = open("control", preId),
		preSnapshot = snapshotFor(preControl);
	await once(preControl, "open");
	await preSnapshot;
	preIo.send(`stty -echo; printf '${octal(history)}\\n'\n`);
	await wait(
		() => preData,
		(v) => v.includes(history),
	);
	preIo.terminate();
	preControl.terminate();
	// This live output occurs after IO attach and before the control snapshot cut.
	const client = randomUUID(),
		io = open("io", client);
	let live = "";
	io.on("message", (raw) => {
		live += raw.toString();
	});
	await once(io, "open");
	io.send(`printf '${octal(between)}\\n'\n`);
	await wait(
		() => live,
		(v) => v.includes(between),
	);
	const control = open("control", client),
		snapshot = snapshotFor(control);
	await once(control, "open");
	const restored = await snapshot;
	assert.ok(restored.includes(history), "Pre-IO history must be in snapshot");
	assert.equal(
		restored.includes(between),
		false,
		"Pre-control live output must not be replayed in snapshot",
	);
	assert.equal(live.split(between).length - 1, 1);
	control.send(JSON.stringify({ type: "restore_complete" }));
	io.send(`printf '${octal(after)}\\n'\n`);
	await wait(
		() => live,
		(v) => v.includes(after),
	);
	assert.equal(live.split(after).length - 1, 1);
	assert.equal(restored.includes(after), false);
	// Control-first callers remain usable as well.
	const base = `http://127.0.0.1:${runtime.hive.port}`;
	const session = await fetch(base + "/api/ui/session");
	const cookie = session.headers.get("set-cookie")!.split(";")[0]!;
	await session.body?.cancel();
	const legacy = (kind: "io" | "control", clientId: string) => {
		const u = new URL(
			`/ws/terminal/${run.runId}/${kind}`,
			base.replace("http:", "ws:"),
		);
		u.search = new URLSearchParams({
			clientId,
			cols: "120",
			rows: "40",
		}).toString();
		const socket = new WebSocket(u, { headers: { cookie }, origin: base });
		sockets.push(socket);
		return socket;
	};
	const cfId = randomUUID(),
		cfControl = legacy("control", cfId),
		cfSnapshot = snapshotFor(cfControl);
	await once(cfControl, "open");
	const cfRestore = await cfSnapshot;
	assert.ok(cfRestore.includes(after));
	const cfIo = legacy("io", cfId);
	let cfLive = "";
	cfIo.on("message", (raw) => {
		cfLive += raw.toString();
	});
	await once(cfIo, "open");
	cfControl.send(JSON.stringify({ type: "restore_complete" }));
	cfIo.send(`printf '${octal(controlFirst)}\\n'\n`);
	await wait(
		() => cfLive,
		(v) => v.includes(controlFirst),
	);
	assert.equal(cfLive.split(controlFirst).length - 1, 1);
	await writeFile(
		join(directory, "receipt.json"),
		JSON.stringify(
			{
				pass: true,
				noModel: true,
				realHive: true,
				realPty: true,
				actualNativeDualWebSocket: true,
				historyInSnapshot: true,
				betweenOnlyLive: true,
				afterOnlyLive: true,
				legacyControlFirstCompatible: true,
				scope: "deterministic staged markers; not a probability claim",
			},
			null,
			2,
		),
	);
	console.log("Evidence:", directory);
	await runtime.terminal.stop({ workspaceId: ws.id, runId: run.runId });
} catch (error) {
	await writeFile(
		join(directory, "failure.json"),
		JSON.stringify(
			{ error: String(error), stack: (error as Error).stack },
			null,
			2,
		),
	);
	console.error("Evidence:", directory);
	throw error;
} finally {
	for (const socket of sockets) socket.terminate();
	await runtime.close();
}

/** Real Hive HTTP/SQLite + PTY + dual WebSocket terminal bridge; no AI/model calls. */
import assert from "node:assert/strict";
import { mkdtemp, mkdir, writeFile, stat, realpath } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { randomUUID } from "node:crypto";
import { createDesktopRuntime } from "../src/desktop-runtime.js";
import type { TerminalEvent } from "../src/terminal-connection.js";
if (process.platform === "win32")
	throw new Error("This POSIX PTY check has not been validated on Windows");
const directory = await mkdtemp(join(tmpdir(), "hive-terminal-check-"));
const project = join(directory, "project");
const other = join(directory, "other");
await mkdir(project);
await mkdir(other);
const terminalEvents: TerminalEvent[] = [];
const wait = async <T>(
	read: () => T | Promise<T>,
	ready: (value: T) => boolean,
	label: string,
) => {
	const until = Date.now() + 10000;
	for (;;) {
		const value = await read();
		if (ready(value)) return value;
		if (Date.now() > until) throw new Error("Timed out: " + label);
		await new Promise((resolve) => setTimeout(resolve, 10));
	}
};
type Event = TerminalEvent;
const runtime = await createDesktopRuntime(join(directory, "state"), () => {}, {
	executableEnvironment: {},
	onTerminalEvent: (event) => terminalEvents.push(event),
});
const terminal = runtime.terminal;
const event = (a: string, type: string) =>
	terminalEvents.find((e) => e.attachmentId === a && e.type === type) as
		| Event
		| undefined;
const input = (attachmentId: string, s: string) =>
	terminal.input({ attachmentId, bytes: [...Buffer.from(s)] });
const shquote = (s: string) => "'" + s.replaceAll("'", "'\\''") + "'";
const octal = (s: string) =>
	[...Buffer.from(s)]
		.map((n) => "\\" + n.toString(8).padStart(3, "0"))
		.join("");
const positions = new Map<string, number>();
const collected = new Map<string, Buffer>();
async function drain(id: string) {
	const i = positions.get(id) ?? 0;
	const fresh = terminalEvents.slice(i) as Event[];
	positions.set(id, terminalEvents.length);
	for (const e of fresh) {
		if (e.attachmentId !== id || e.type !== "output") continue;
		const chunk = Buffer.from(e.bytes!);
		collected.set(
			id,
			Buffer.concat([collected.get(id) ?? Buffer.alloc(0), chunk]),
		);
		await terminal.ack({
			attachmentId: id,
			sequence: e.sequence,
			bytes: chunk.length,
		});
	}
	return collected.get(id) ?? Buffer.alloc(0);
}
async function untilText(id: string, text: string) {
	return wait(
		async () => await drain(id),
		(b) => b.includes(Buffer.from(text)),
		text,
	);
}
async function attach(workspaceId: string, runId: string) {
	const attachmentId = randomUUID();
	await terminal.attach({
		workspaceId,
		runId,
		attachmentId,
		cols: 90,
		rows: 30,
	});
	const restore = (await wait(
		() => event(attachmentId, "restore"),
		Boolean,
		"restore",
	)) as Extract<Event, { type: "restore" }>;
	assert.equal(restore.sequence, 1);
	await assert.rejects(terminal.input({ attachmentId, bytes: [120] }));
	await assert.rejects(terminal.restoreComplete({ attachmentId, sequence: 2 }));
	await terminal.restoreComplete({ attachmentId, sequence: restore.sequence });
	return { attachmentId, restore };
}
try {
	const a = (await runtime.hive.request("POST", "/api/workspaces", {
		path: project,
		name: "terminal-check",
		autostart_orchestrator: false,
	})) as {
		id: string;
	};
	const b = (await runtime.hive.request("POST", "/api/workspaces", {
		path: other,
		name: "other",
		autostart_orchestrator: false,
	})) as {
		id: string;
	};
	const run = await terminal.start(a.id);
	await assert.rejects(
		terminal.attach({
			workspaceId: b.id,
			runId: run.runId,
			attachmentId: randomUUID(),
			cols: 80,
			rows: 24,
		}),
	);
	await assert.rejects(terminal.stop({ workspaceId: b.id, runId: run.runId }));
	await assert.rejects(
		terminal.attach({
			workspaceId: a.id,
			runId: randomUUID(),
			attachmentId: randomUUID(),
			cols: 80,
			rows: 24,
		}),
	);
	const first = await attach(a.id, run.runId);
	let id = first.attachmentId;
	const ready = "READY_" + randomUUID();
	await input(id, `stty -echo; printf '${octal(ready)}\\n'\n`);
	await untilText(id, ready);
	const pidTag = "SHELLPID_" + randomUUID();
	await input(id, `printf '${octal(pidTag)}:%s\\n' "$$"\n`);
	const pidPattern = new RegExp(pidTag + ":(\\d+)\\r?\\n");
	const pidOut = await wait(
		() => drain(id),
		(v) => pidPattern.test(v.toString()),
		"shell PID",
	);
	const pidMatch = pidOut.toString().match(pidPattern);
	assert.ok(pidMatch);
	const shellPid = Number(pidMatch[1]);
	process.kill(shellPid, 0);
	const marker = "UTF8_" + randomUUID();
	const content = `${marker}:你好:\u001b[31m红色\u001b[0m`;
	await input(id, `printf '${octal(content)}\\n'; pwd\n`);
	const observed = await untilText(id, await realpath(project));
	assert.ok(observed.includes(Buffer.from(content)));
	assert.equal(observed.toString().split(marker).length - 1, 1);
	// Split one UTF-8 code point across actual writes with an explicit filesystem gate.
	const chunkReady = join(directory, "utf8-ready"),
		chunkRelease = join(directory, "utf8-release"),
		chunkStart = "SPLIT_" + randomUUID(),
		chunkEnd = "_END";
	const splitProgram = `const fs=require('node:fs');process.stdout.write(${JSON.stringify(chunkStart)});process.stdout.write(Buffer.from([228,189]));fs.writeFileSync(${JSON.stringify(chunkReady)},'ready');function step(){if(!fs.existsSync(${JSON.stringify(chunkRelease)})){setImmediate(step);return;}process.stdout.write(Buffer.from([160]));console.log(${JSON.stringify(chunkEnd)})}step()`;
	await input(id, `node -e ${shquote(splitProgram)}\n`);
	await wait(
		async () => {
			try {
				await stat(chunkReady);
				return true;
			} catch (e) {
				if ((e as NodeJS.ErrnoException).code === "ENOENT") return false;
				throw e;
			}
		},
		Boolean,
		"split UTF8 first write",
	);
	await writeFile(chunkRelease, "release");
	const split = await untilText(id, chunkStart + "你" + chunkEnd);
	assert.ok(split.includes(Buffer.from(chunkStart + "你" + chunkEnd)));
	await terminal.resize({ attachmentId: id, cols: 101, rows: 37 });
	const resize = "SIZE_" + randomUUID();
	await input(id, `printf '${octal(resize)}:'; stty size\n`);
	await wait(
		async () => await drain(id),
		(v) => v.includes(Buffer.from(resize + ":37 101")),
		"resized PTY",
	);
	await terminal.detach({ attachmentId: id });
	assert.ok(
		(await terminal.list(a.id)).runs.some((x) => x.runId === run.runId),
	);
	const second = await attach(a.id, run.runId);
	id = second.attachmentId;
	assert.ok(Buffer.from(second.restore.bytes!).includes(Buffer.from(marker)));
	const after = "REATTACHED_" + randomUUID();
	await input(id, `printf '${octal(after)}\\n'\n`);
	const live = await untilText(id, after);
	assert.equal(live.toString().split(after).length - 1, 1);
	// A real foreground command is interrupted; the shell trap and next command remain usable.
	const armed = "INT_ARM_" + randomUUID(),
		trapped = "INT_TRAP_" + randomUUID(),
		afterInt = "INT_AFTER_" + randomUUID();
	await input(
		id,
		`trap "printf '${octal(trapped)}\\n'" INT; printf '${octal(armed)}\\n'; sleep 300; trap - INT\n`,
	);
	await untilText(id, armed);
	await terminal.input({ attachmentId: id, bytes: [3] });
	await untilText(id, trapped);
	await input(id, `printf '${octal(afterInt)}\\n'\n`);
	await untilText(id, afterInt);
	// Observe a second real attachment while a producer is still writing, retaining any overlap evidence.
	const raceId = randomUUID().replaceAll("-", ""),
		raceGate = join(directory, "race-release"),
		raceEnd = "RACE_END_" + raceId;
	const raceProgram = `const fs=require('node:fs');let n=0;function tick(){if(fs.existsSync(${JSON.stringify(raceGate)})){console.log(${JSON.stringify(raceEnd)});return;}process.stdout.write('RACE_${raceId}_'+String(n++).padStart(6,'0')+'\\n');setImmediate(tick)}tick()`;
	await input(id, `node -e ${shquote(raceProgram)}\n`);
	await untilText(id, "RACE_" + raceId + "_");
	const concurrent = await attach(a.id, run.runId);
	await writeFile(raceGate, "release");
	await wait(
		async () => {
			await drain(id);
			return drain(concurrent.attachmentId);
		},
		(v) => v.includes(Buffer.from(raceEnd)),
		"race producer completion",
	);
	const pattern = new RegExp("RACE_" + raceId + "_\\d{6}", "g");
	const snapshotMatches =
		Buffer.from(concurrent.restore.bytes!).toString().match(pattern) ?? [];
	const liveMatches =
		(collected.get(concurrent.attachmentId) ?? Buffer.alloc(0))
			.toString()
			.match(pattern) ?? [];
	const overlap = [
		...new Set(snapshotMatches.filter((x) => liveMatches.includes(x))),
	];
	await writeFile(
		join(directory, "restore-live-observation.json"),
		JSON.stringify(
			{
				snapshotCount: snapshotMatches.length,
				liveCount: liveMatches.length,
				overlap,
			},
			null,
			2,
		),
	);
	await terminal.detach({ attachmentId: concurrent.attachmentId });
	// Observe a real producer at the unacknowledged high-water point, then consume all frames.
	const flowMarker = join(directory, "producer-complete"),
		flowEnd = "FLOW_END_" + randomUUID();
	const producer = `const fs=require('node:fs');process.stdout.write(Buffer.alloc(2097152,120),()=>{fs.writeFileSync(${JSON.stringify(flowMarker)},'done');console.log(${JSON.stringify(flowEnd)})})`;
	const offset = terminalEvents.length;
	await input(id, `node -e ${shquote(producer)}\n`);
	await wait(
		() =>
			terminalEvents
				.slice(offset)
				.filter((e) => e.attachmentId === id && e.type === "output")
				.reduce((n, e) => n + (e.bytes as number[]).length, 0),
		(n) => n >= 100 * 1024,
		"flow high water",
	);
	const firstFrame = terminalEvents
		.slice(offset)
		.find((e) => e.attachmentId === id && e.type === "output") as Extract<
		Event,
		{ type: "output" }
	>;
	await assert.rejects(
		terminal.ack({
			attachmentId: id,
			sequence: firstFrame.sequence,
			bytes: firstFrame.bytes!.length + 1,
		}),
	);
	await assert.rejects(
		stat(flowMarker),
		(e: unknown) => (e as NodeJS.ErrnoException).code === "ENOENT",
	);
	await untilText(id, flowEnd);
	assert.equal((await stat(flowMarker)).isFile(), true);
	await terminal.stop({ workspaceId: a.id, runId: run.runId });
	await wait(
		async () => terminal.list(a.id),
		(v) => !v.runs.some((x) => x.runId === run.runId),
		"stop",
	);
	await wait(
		() => {
			try {
				process.kill(shellPid, 0);
				return false;
			} catch (e) {
				if ((e as NodeJS.ErrnoException).code === "ESRCH") return true;
				throw e;
			}
		},
		Boolean,
		"shell process exit",
	);
	const natural = await terminal.start(a.id);
	const third = await attach(a.id, natural.runId);
	const restoring = randomUUID();
	await terminal.attach({workspaceId: a.id, runId: natural.runId, attachmentId: restoring, cols: 90, rows: 30});
	await wait(() => event(restoring, "restore"), Boolean, "second viewer snapshot");
	const tail = "TAIL_" + randomUUID();
	await input(third.attachmentId, `printf '${octal(tail)}'; exit\n`);
	await wait(() => event(third.attachmentId, "exit"), Boolean, "natural exit");
	await terminal.restoreComplete({attachmentId: restoring, sequence: 1});
	await untilText(restoring, tail);
	await wait(() => event(restoring, "exit"), Boolean, "restored viewer exit");
	const ordered = terminalEvents.filter(e => e.attachmentId === restoring);
	assert.equal(ordered.filter(e => e.type === "exit").length, 1);
	assert.ok(ordered.findIndex(e => e.type === "exit") > ordered.findLastIndex(e => e.type === "output"));
	const pidOf = async (attachmentId: string) => {
		const tag = "CLEANUP_PID_" + randomUUID();
		await input(
			attachmentId,
			`stty -echo; printf '${octal(tag)}:%s\\n' "$$"\n`,
		);
		const regex = new RegExp(tag + ":(\\d+)\\r?\\n");
		const out = await wait(
			() => drain(attachmentId),
			(v) => regex.test(v.toString()),
			"cleanup pid",
		);
		return Number(out.toString().match(regex)![1]);
	};
	const gone = async (pid: number) =>
		wait(
			() => {
				try {
					process.kill(pid, 0);
					return false;
				} catch (e) {
					if ((e as NodeJS.ErrnoException).code === "ESRCH") return true;
					throw e;
				}
			},
			Boolean,
			"cleanup process exit",
		);
	const cleanup = await terminal.start(a.id);
	const ca = await attach(a.id, cleanup.runId);
	const cleanupPid = await pidOf(ca.attachmentId);
	await runtime.removeWorkspace(a.id);
	await gone(cleanupPid);
	assert.equal((await terminal.list(b.id)).runs.length, 0);
	const finalRun = await terminal.start(b.id);
	const finalAttach = await attach(b.id, finalRun.runId);
	const finalPid = await pidOf(finalAttach.attachmentId);
	await runtime.close();
	await gone(finalPid);
	await writeFile(
		join(directory, "receipt.json"),
		JSON.stringify(
			{
				pass: true,
				noModel: true,
				platform: process.platform,
				realHiveHttpSqlite: true,
				realPty: true,
				dualNativeWebSocket: true,
				utf8AnsiAndSplitNativeWrites: true,
				cwd: true,
				resize: true,
				detachReattach: true,
				wrongWorkspaceRejected: true,
				unackedHighWaterObserved: true,
				producerNotFinishedAtObservation: true,
				producerFinishedAfterAck: true,
				ptyPauseDirectlyObserved: false,
				controlCAndContinuedShell: true,
				stop: true,
				naturalExit: true,
				workspaceRemoval: true,
				runtimeClose: true,
			},
			null,
			2,
		),
	);
	console.log("Evidence:", directory);
} catch (error) {
	await writeFile(
		join(directory, "failure.json"),
		JSON.stringify(
			{
				error: String(error),
				stack: (error as Error).stack,
				events: terminalEvents.map((e) => ({
					...e,
					...(Array.isArray(e.bytes)
						? { bytes: `<${e.bytes.length} bytes>` }
						: {}),
				})),
			},
			null,
			2,
		),
	);
	console.error("Evidence:", directory);
	throw error;
} finally {
	await runtime.close();
}

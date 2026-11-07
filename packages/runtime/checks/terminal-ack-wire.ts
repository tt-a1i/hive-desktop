/** Real loopback WebSockets + production TerminalConnection, synthetic Hive peer. No PTY or model. */
import assert from "node:assert/strict";
import { once } from "node:events";
import { mkdtemp, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { randomUUID } from "node:crypto";
import WebSocket, { WebSocketServer } from "ws";
import {
	TerminalConnection,
	type TerminalEvent,
} from "../src/terminal-connection.js";
import type { HiveHost } from "../src/hive-host.js";
const dir = await mkdtemp(join(tmpdir(), "hive-terminal-ack-wire-"));
const server = new WebSocketServer({ host: "127.0.0.1", port: 0 });
await once(server, "listening");
const address = server.address();
assert.ok(address && typeof address !== "string");
const controls: Record<string, unknown>[] = [];
let io: WebSocket | undefined;
server.on("connection", (socket, request) => {
	if (request.url === "/io") io = socket;
	else {
		socket.on("message", (data) => controls.push(JSON.parse(data.toString())));
		socket.send(JSON.stringify({ type: "restore", snapshot: "" }));
	}
});
const host = {
	openTerminalSocket(_runId: string, channel: string) {
		return new WebSocket(`ws://127.0.0.1:${address.port}/${channel}`);
	},
} as unknown as HiveHost;
const id = randomUUID();
const events: TerminalEvent[] = [];
const conn = new TerminalConnection(
	{ workspaceId: "owned", runId: "run", attachmentId: id, cols: 80, rows: 24 },
	host,
	(e) => events.push(e),
	() => {},
);
const wait = async <T>(f: () => T, ok: (v: T) => boolean) => {
	const end = Date.now() + 5000;
	for (;;) {
		const v = f();
		if (ok(v)) return v;
		if (Date.now() > end) throw new Error("ACK wire gate timed out");
		await new Promise((r) => setTimeout(r, 5));
	}
};
try {
	await conn.connect();
	await wait(() => events.find((e) => e.type === "restore"), Boolean);
	await conn.restoreComplete(1);
	const bytes = Buffer.from("消费后确认\u001b[31m", "utf8");
	assert.ok(io);
	io.send(bytes);
	const output = (await wait(
		() => events.find((e) => e.type === "output"),
		Boolean,
	)) as Extract<
		TerminalEvent,
		{
			type: "output";
		}
	>;
	assert.deepEqual(Buffer.from(output.bytes), bytes);
	// Resize is a FIFO fence on the same control socket: no earlier output_ack reached the peer.
	await conn.resize(101, 37);
	await wait(() => controls.some((x) => x.type === "resize"), Boolean);
	assert.equal(
		controls.some((x) => x.type === "output_ack"),
		false,
	);
	await assert.rejects(conn.ack(output.sequence + 1, bytes.length));
	await assert.rejects(conn.ack(output.sequence, bytes.length + 1));
	await conn.ack(output.sequence, bytes.length);
	await wait(
		() => controls.filter((x) => x.type === "output_ack"),
		(v) => v.length === 1,
	);
	await assert.rejects(conn.ack(output.sequence, bytes.length));
	// A second same-socket fence proves no duplicate credit followed the valid acknowledgement.
	await conn.resize(102, 38);
	await wait(
		() => controls.some((x) => x.type === "resize" && x.cols === 102),
		Boolean,
	);
	assert.deepEqual(
		controls.filter((x) => x.type === "output_ack"),
		[{ type: "output_ack", bytes: bytes.length }],
	);
	await writeFile(
		join(dir, "receipt.json"),
		JSON.stringify(
			{
				pass: true,
				syntheticNativePeer: true,
				realLoopbackWebSockets: true,
				productionTerminalConnection: true,
				noModel: true,
				noPty: true,
				unconsumedBeforeControlFenceNoAck: true,
				matchingAckExactlyOnce: true,
				invalidAndDuplicateRejected: true,
				scope: "wire credit accounting; not PTY pause",
			},
			null,
			2,
		),
	);
	console.log("Evidence:", dir);
} catch (e) {
	await writeFile(
		join(dir, "failure.json"),
		JSON.stringify(
			{ error: String(e), stack: (e as Error).stack, controls },
			null,
			2,
		),
	);
	console.error("Evidence:", dir);
	throw e;
} finally {
	await conn.close("check-finished");
	for (const client of server.clients) client.terminate();
	await new Promise<void>((r) => server.close(() => r()));
}

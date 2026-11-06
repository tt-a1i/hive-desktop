// Backend-independent lifecycle contract check; no claim of model/HTTP integration.
import assert from "node:assert/strict";
import { createStructuredRuntime } from "../src/structured-runtime.js";
import type {
	HiveAgentRuntime,
	HiveAgentRuntimeFactoryContext,
	PersistedAgentRun,
} from "../src/hive-runtime-contract.js";

const persisted = new Map<string, PersistedAgentRun>();
const accepted: Array<{ text: string; triggerTurn: boolean }> = [];
let exits = 0,
	stops = 0,
	readies = 0,
	fallbackStarts = 0,
	fallbackCloses = 0;
let release!: () => void;
const blocked = new Promise<void>((resolve) => {
	release = resolve;
});
let onExit!: (error?: unknown) => void;
const config = new Map<
	string,
	import("../src/hive-runtime-contract.js").AgentLaunchConfigInput
>([
	["w:orchestrator", { command: "hive:pi" }],
	["external", { command: "pi" }],
]);
const defaultRuntime = {
	peekAgentLaunchConfig: (_w: string, a: string) => config.get(a),
	getActiveRunByAgentId: () => undefined,
	startAgent: async () => {
		fallbackStarts++;
		return { runId: "external" };
	},
	peekAgentToken: () => undefined,
	validateAgentToken: () => false,
	close: async () => {
		fallbackCloses++;
	},
} as unknown as HiveAgentRuntime;
const context = {
	defaultRuntime,
	agentRunStore: {
		insertAgentRun(
			runId: string,
			agentId: string,
			startedAt: number,
			pid: null,
			status: PersistedAgentRun["status"],
		) {
			persisted.set(runId, {
				runId,
				agentId,
				startedAt,
				pid,
				status,
				endedAt: null,
				exitCode: null,
			});
		},
		updatePersistedRun(
			runId: string,
			status: PersistedAgentRun["status"],
			exitCode: number | null,
			endedAt: number | null,
		) {
			Object.assign(persisted.get(runId)!, { status, exitCode, endedAt });
		},
	},
	sessionStore: {
		getLastSessionId: () => undefined,
		setLastSessionId: () => {},
	},
	onAgentExit: () => {
		exits++;
	},
	getAgent: () => ({
		id: "w:orchestrator",
		name: "Lead",
		workspaceId: "w",
		role: "orchestrator",
		description: "Lead",
		status: "idle",
		pendingTaskCount: 0,
	}),
	getFlags: () => ({ workflowsEnabled: false }),
	restartPolicy: {
		injectPostStartMessage: () => false,
		markUserStopped: () => {},
	},
	memoryInjection: { buildDigest: () => null, buildDispatchDigest: () => null },
} as unknown as HiveAgentRuntimeFactoryContext;
const runtime = createStructuredRuntime(context, {
	async start(input) {
		onExit = input.onExit;
		return {
			sessionId: "sdk-session",
			ready() {
				assert.equal(accepted[0]?.triggerTurn, false);
				assert.ok(
					runtime.getActiveRunByAgentId("w", "w:orchestrator")?.startupReadyAt,
				);
				readies++;
			},
			async deliver(text, options) {
				if (text === "block") await blocked;
				accepted.push({ text, triggerTurn: options?.triggerTurn ?? true });
			},
			async stop() {
				stops++;
				onExit();
			},
		};
	},
});
config.set("w:orchestrator", {
	command: "hive:pi",
	args: ["provider", "model"],
	resumedSessionId: "lead-only",
	cwd: "/lead-only",
});
assert.deepEqual(runtime.resolveSpawnLaunchConfig!("w"), {
	command: "hive:pi",
	args: ["provider", "model"],
	commandPresetId: null,
});
assert.deepEqual(runtime.resolveSpawnLaunchConfig!("w", "hive:pi"), {
	command: "hive:pi",
	args: ["provider", "model"],
	commandPresetId: null,
});
assert.equal(runtime.resolveSpawnLaunchConfig!("w", "pi"), undefined);
config.set("w:orchestrator", { command: "hive:pi" });
const workspace = { id: "w", name: "Workspace", path: "/tmp" };
const run = await runtime.startAgent(workspace, "w:orchestrator", {
	hivePort: "123",
});
assert.equal(accepted[0]?.triggerTurn, false);
assert.match(
	accepted[0]!.text,
	/Hive session binding: workspace_id=w; agent_id=w:orchestrator/,
);
assert.equal(persisted.get(run.runId)?.status, "running");
assert.ok(run.startupReadyAt);
assert.equal(readies, 1);
assert.ok(
	runtime.validateAgentToken(
		"w:orchestrator",
		runtime.peekAgentToken("w:orchestrator"),
	),
);
const first = runtime.deliverSystemMessageToAgent(
	"w",
	"w:orchestrator",
	"block",
);
const skipped = runtime.deliverSystemMessageToAgent(
	"w",
	"w:orchestrator",
	"must not arrive",
	{ beforeWrite: () => false },
);
const dispatch = runtime.writeSendPrompt(
	"w",
	"w:orchestrator",
	"d",
	"Lead",
	"task",
	"dispatch reaches backend",
);
assert.equal(typeof dispatch.then, "function");
assert.equal(typeof dispatch.catch, "function");
const last = runtime.deliverSystemMessageToAgent("w", "w:orchestrator", "last");
assert.equal(accepted.length, 1);
release();
assert.equal(await skipped, undefined);
assert.equal(await dispatch.catch(() => "failed"), undefined);
await Promise.all([first, last]);
assert.deepEqual(
	accepted
		.slice(1)
		.map((item) =>
			item.text.startsWith('<hive-message kind="dispatch"')
				? "dispatch"
				: item.text,
		),
	["block", "dispatch", "last"],
);
assert.match(accepted[2]!.text, /dispatch_id: d/);
assert.match(accepted[2]!.text, /dispatch reaches backend/);
assert.equal(run.status, "running"); // Turn acceptance does not exit a run.
await runtime.startAgent(workspace, "external", { hivePort: "123" });
assert.equal(fallbackStarts, 1); // Bare pi remains external.
runtime.stopAgentRun(run.runId);
assert.equal(await runtime.waitForRunExit(run.runId, 1000), true);
onExit(new Error("duplicate backend callback"));
assert.equal(exits, 1);
assert.equal(stops, 1);
assert.equal(persisted.get(run.runId)?.status, "exited");
assert.equal(runtime.validateAgentToken("w:orchestrator", "anything"), false);
await runtime.close();
assert.equal(fallbackCloses, 1);
console.error("Structured lifecycle/serial delivery contract check passed");
const failing = createStructuredRuntime(context, {
	async start() {
		throw new Error("backend unavailable");
	},
});
await assert.rejects(
	failing.startAgent(workspace, "w:orchestrator", { hivePort: "123" }),
);
const failed = [...persisted.values()].at(-1)!;
assert.equal(failed.status, "error");
assert.equal(exits, 2);
assert.equal(failing.peekAgentToken("w:orchestrator"), undefined);
await failing.close();
let finishStart!: () => void;
const starting = new Promise<void>((resolve) => {
	finishStart = resolve;
});
const stopping = createStructuredRuntime(context, {
	async start() {
		await starting;
		return {
			sessionId: "stopped-during-start",
			async deliver() {
				throw new Error("stopped run must never receive startup");
			},
			async stop() {
				stops++;
			},
		};
	},
});
const startPromise = stopping.startAgent(workspace, "w:orchestrator", {
	hivePort: "123",
});
const pendingRun = stopping.getActiveRunByAgentId("w", "w:orchestrator")!;
assert.equal(pendingRun.status, "starting");
stopping.stopAgentRun(pendingRun.runId);
finishStart();
await startPromise;
assert.equal(pendingRun.status, "exited");
assert.equal(exits, 3);
assert.equal(stops, 2);
assert.equal(readies, 1);
assert.equal(stopping.peekAgentToken("w:orchestrator"), undefined);
await stopping.close();
console.error("Structured startup failure and stop-during-start checks passed");

const freshAccepted: Array<{ text: string; triggerTurn: boolean }> = [];
let recoveryResumeHint: string | null | undefined = "not-called";
const previousSessionContext = {
	...context,
	sessionStore: { ...context.sessionStore, getLastSessionId: () => "chat-A" },
	restartPolicy: {
		...context.restartPolicy,
		injectPostStartMessage(input) {
			recoveryResumeHint = input.startConfig.resumedSessionId;
			return false;
		},
	},
} satisfies HiveAgentRuntimeFactoryContext;
const freshChat = createStructuredRuntime(previousSessionContext, {
	async start(input) {
		assert.equal(input.config.resumedSessionId, "chat-A");
		return {
			sessionId: "chat-B",
			resumed: false,
			async deliver(text, options) {
				freshAccepted.push({ text, triggerTurn: options?.triggerTurn ?? true });
			},
			async stop() {},
		};
	},
});
const freshRun = await freshChat.startAgent(workspace, "w:orchestrator", {
	hivePort: "123",
});
assert.equal(recoveryResumeHint, undefined);
assert.equal(freshAccepted.length, 1);
assert.equal(freshAccepted[0]?.triggerTurn, false);
assert.match(
	freshAccepted[0]!.text,
	/Hive session binding: workspace_id=w; agent_id=w:orchestrator/,
);
assert.ok(freshRun.startupReadyAt);
await freshChat.close();
console.error(
	"Fresh selected chat overrides previous resume hint check passed",
);

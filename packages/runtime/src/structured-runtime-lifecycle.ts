import { randomUUID } from "node:crypto";
import {
	buildAgentStartupInstructions,
	buildWorkflowAgentStartupInstructions,
} from "@tt-a1i/hive/dist/src/server/agent-startup-instructions.js";
import {
	ConflictError,
	PtyInactiveError,
} from "@tt-a1i/hive/dist/src/server/http-errors.js";
import {
	buildMemoryDigestSafely,
	logMemoryDigestInjection,
	rollbackMemoryDigestInjection,
} from "@tt-a1i/hive/dist/src/server/team-memory-injection.js";
import type {
	HiveAgentRuntimeFactoryContext,
	WorkspaceSummary,
	AgentLaunchConfigInput,
} from "./hive-runtime-contract.js";
import type {
	StructuredRun,
	StructuredSessionPort,
} from "./structured-runtime-types.js";

export function createStructuredLifecycle(
	context: HiveAgentRuntimeFactoryContext,
	port: StructuredSessionPort,
) {
	const runs = new Map<string, StructuredRun>();
	const starts = new Map<string, Promise<StructuredRun["live"]>>();
	const tokens = new Map<string, string>();
	let closing = false;
	const active = (workspaceId: string, agentId: string) =>
		[...runs.values()].find(
			(r) =>
				r.workspaceId === workspaceId &&
				r.live.agentId === agentId &&
				!r.terminal,
		);
	const finish = (run: StructuredRun, error?: unknown) => {
		if (run.terminal) return;
		const status = error === undefined ? "exited" : "error";
		// Durable state first; an unsuccessful database write must remain observable.
		context.agentRunStore.updatePersistedRun(
			run.live.runId,
			status,
			error === undefined ? 0 : 1,
			Date.now(),
		);
		run.terminal = true;
		run.live.status = status;
		run.live.exitCode = error === undefined ? 0 : 1;
		run.live.startupReadyAt = null;
		if (tokens.get(run.live.agentId) === run.token)
			tokens.delete(run.live.agentId);
		run.rejectReady(
			error ?? new PtyInactiveError("Run exited before readiness"),
		);
		run.resolveExit();
		context.onAgentExit(run.workspaceId, run.live.agentId);
	};
	const stop = (run: StructuredRun, error?: unknown): Promise<void> => {
		if (run.stopping) return run.stopping;
		run.stopping = (async () => {
			await run.handle?.stop(error);
			finish(run, error);
		})();
		return run.stopping;
	};
	const start = (
		workspace: WorkspaceSummary,
		agentId: string,
		config: AgentLaunchConfigInput,
		hivePort: string,
	) => {
		if (closing) throw new Error("Agent runtime is closing");
		const existing = active(workspace.id, agentId);
		if (existing) return Promise.resolve(existing.live);
		const key = `${workspace.id}:${agentId}`;
		const pending = starts.get(key);
		if (pending) return pending;
		const promise = (async () => {
			if (
				workspace.controller_mode === "codex_app" &&
				agentId === `${workspace.id}:orchestrator`
			)
				throw new ConflictError(
					"This workspace uses Codex App as its controller",
				);
			const runId = randomUUID(),
				token = randomUUID(),
				startedAt = Date.now();
			let resolveReady!: () => void,
				rejectReady!: (error: unknown) => void,
				resolveExit!: () => void;
			const ready = new Promise<void>((resolve, reject) => {
				resolveReady = resolve;
				rejectReady = reject;
			});
			void ready.catch(() => {});
			const exited = new Promise<void>((resolve) => {
				resolveExit = resolve;
			});
			const run: StructuredRun = {
				workspaceId: workspace.id,
				token,
				exited,
				resolveExit,
				resolveReady,
				rejectReady,
				terminal: false,
				queue: Promise.resolve(),
				live: {
					runId,
					agentId,
					startedAt,
					pid: null,
					status: "starting",
					output: "",
					exitCode: null,
					startupReadyAt: null,
					postStartInputReady: ready,
				},
			};
			try {
				context.agentRunStore.insertAgentRun(
					runId,
					agentId,
					startedAt,
					null,
					"starting",
				);
			} catch (error) {
				rejectReady(error);
				context.onAgentExit(workspace.id, agentId);
				throw error;
			}
			runs.set(runId, run);
			tokens.set(agentId, token);
			try {
				const lastSessionId = context.sessionStore.getLastSessionId(
					workspace.id,
					agentId,
				);
				const startConfig: AgentLaunchConfigInput = {
					...config,
					...(config.resumedSessionId
						? {}
						: lastSessionId
							? { resumedSessionId: lastSessionId }
							: {}),
				};
				run.handle = await port.start({
					workspace,
					agentId,
					runId,
					config: startConfig,
					token,
					hivePort,
					onExit: (error) => finish(run, error),
				});
				// The backend may select a fresh chat despite Hive's previous session hint.
				if (run.handle.resumed === false) delete startConfig.resumedSessionId;
				if (run.terminal || run.live.userStopped || closing) {
					await stop(run);
					return run.live;
				}
				context.sessionStore.setLastSessionId(
					workspace.id,
					agentId,
					run.handle.sessionId,
				);
				context.agentRunStore.updatePersistedRun(runId, "running", null, null);
				run.live.status = "running";
				let recoveryWrite: Promise<void> | undefined;
				const recovering = context.restartPolicy.injectPostStartMessage({
					agentId,
					runId,
					startConfig,
					workspace,
					writeToRun: (target, text) => {
						if (target !== runId)
							return Promise.reject(new Error("Recovery target run mismatch"));
						recoveryWrite = run.handle!.deliver(text, {
							presentation: {
								version: 1,
								origin: "recovery",
								displayText: "已恢复会话，继续处理未完成的工作。",
							},
						});
						return recoveryWrite;
					},
				});
				if (recovering) await recoveryWrite;
				else if (!startConfig.resumedSessionId) {
					const agent = context.getAgent(workspace.id, agentId);
					if (agent) {
						const digest =
							agent.spawnedBy === "workflow"
								? null
								: buildMemoryDigestSafely({
										contextType: "startup",
										memoryInjection: context.memoryInjection,
										workspaceId: workspace.id,
									});
						const ids = logMemoryDigestInjection({
							agentId,
							contextType: "startup",
							memoryDigest: digest,
							memoryInjection: context.memoryInjection,
							workspaceId: workspace.id,
						});
						try {
							const text =
								agent.spawnedBy === "workflow"
									? buildWorkflowAgentStartupInstructions({ agent, workspace })
									: buildAgentStartupInstructions({
											agent,
											workspace,
											flags: context.getFlags(),
											memoryDigest: ids ? (digest?.text ?? null) : null,
										});
							await run.handle.deliver(text, { triggerTurn: false });
						} catch (error) {
							rollbackMemoryDigestInjection({
								injectionIds: ids,
								memoryInjection: context.memoryInjection,
							});
							throw error;
						}
					}
				}
				if (run.terminal || run.live.userStopped)
					throw new PtyInactiveError("Run exited before readiness");
				run.live.startupReadyAt = Date.now();
				resolveReady();
				run.handle.ready?.();
			} catch (error) {
				rejectReady(error);
				await stop(run, error);
				throw error;
			}
			return run.live;
		})().finally(() => {
			starts.delete(key);
		});
		starts.set(key, promise);
		return promise;
	};
	return {
		runs,
		tokens,
		active,
		start,
		stop,
		async close() {
			closing = true;
			await Promise.allSettled(starts.values());
			const results = await Promise.allSettled(
				[...runs.values()].filter((r) => !r.terminal).map((r) => stop(r)),
			);
			const failed = results.find((r) => r.status === "rejected");
			if (failed?.status === "rejected") throw failed.reason;
		},
	};
}

import { timingSafeEqual } from "node:crypto";
import {
	buildOrchestratorStatusPayload,
	buildOrchestratorUserInputPayload,
	buildWorkerDispatchPayload,
	buildWorkerCancelPayload,
} from "@tt-a1i/hive/dist/src/server/agent-stdin-dispatcher.js";
import { PtyInactiveError } from "@tt-a1i/hive/dist/src/server/http-errors.js";
import {
	buildDispatchMemoryDigestSafely,
	logMemoryDigestInjection,
	rollbackMemoryDigestInjection,
} from "@tt-a1i/hive/dist/src/server/team-memory-injection.js";
import type {
	HiveAgentRuntime,
	HiveAgentRuntimeFactoryContext,
} from "./hive-runtime-contract.js";
import { createStructuredLifecycle } from "./structured-runtime-lifecycle.js";
import type {
	StructuredRun,
	StructuredSessionPort,
} from "./structured-runtime-types.js";
export type {
	StructuredSessionHandle,
	StructuredSessionPort,
} from "./structured-runtime-types.js";

/** Only the explicit hive:pi launch marker selects the desktop SDK backend. */
export function createStructuredRuntime(
	context: HiveAgentRuntimeFactoryContext,
	port: StructuredSessionPort,
): HiveAgentRuntime {
	const fallback = context.defaultRuntime;
	const lifecycle = createStructuredLifecycle(context, port);
	const selected = (workspaceId: string, agentId: string) =>
		Boolean(lifecycle.active(workspaceId, agentId)) ||
		fallback.peekAgentLaunchConfig(workspaceId, agentId)?.command === "hive:pi";
	const orchestrator = (workspaceId: string) => `${workspaceId}:orchestrator`;
	const write = (
		workspaceId: string,
		agentId: string,
		text: string,
		options: { requireActiveRun?: boolean; beforeWrite?: () => boolean } = {},
	): Promise<boolean> => {
		const run = lifecycle.active(workspaceId, agentId);
		if (!run) {
			if (options.requireActiveRun)
				throw new PtyInactiveError(`No active run for agent: ${agentId}`);
			return Promise.resolve(false);
		}
		const deliver = async () => {
			await run.live.postStartInputReady;
			if (run.terminal || run.live.userStopped || !run.handle)
				throw new PtyInactiveError(`No active run for agent: ${agentId}`);
			if (options.beforeWrite?.() === false) return false;
			await run.handle.deliver(text);
			return true;
		};
		const result = run.queue.then(deliver, deliver);
		run.queue = result.catch(() => {});
		return result;
	};
	const detached = (promise: Promise<unknown>) => {
		void promise.catch((error) =>
			console.error("[hive-desktop] queued delivery failed", error),
		);
	};
	const ownRun = (runId: string) => lifecycle.runs.get(runId);
	const awaitExit = async (
		run: StructuredRun,
		timeoutMs: number,
	): Promise<boolean> => {
		if (run.terminal) return true;
		if (timeoutMs <= 0) return false;
		let timer: ReturnType<typeof setTimeout> | undefined;
		try {
			return await Promise.race([
				run.exited.then(() => true),
				new Promise<false>((resolve) => {
					timer = setTimeout(() => resolve(false), timeoutMs);
					timer.unref();
				}),
			]);
		} finally {
			if (timer) clearTimeout(timer);
		}
	};
	return {
		...fallback,
		async close() {
			const results = await Promise.allSettled([
				lifecycle.close(),
				fallback.close(),
			]);
			const errors = results
				.filter((r) => r.status === "rejected")
				.map((r) => r.reason);
			if (errors.length)
				throw new AggregateError(errors, "Runtime close failed");
		},
		resolveSpawnLaunchConfig(workspaceId, cli) {
			const config = fallback.peekAgentLaunchConfig(
				workspaceId,
				orchestrator(workspaceId),
			);
			if (
				cli !== "hive:pi" &&
				(cli !== undefined || config?.command !== "hive:pi")
			) {
				return fallback.resolveSpawnLaunchConfig?.(workspaceId, cli);
			}
			return {
				command: "hive:pi",
				args: [...(config?.command === "hive:pi" ? (config.args ?? []) : [])],
				commandPresetId: null,
			};
		},
		getActiveRunByAgentId: (workspaceId, agentId) =>
			lifecycle.active(workspaceId, agentId)?.live ??
			fallback.getActiveRunByAgentId(workspaceId, agentId),
		findLiveRun: (runId) => ownRun(runId)?.live ?? fallback.findLiveRun(runId),
		getLiveRun: (runId) => ownRun(runId)?.live ?? fallback.getLiveRun(runId),
		waitForRunExit: (runId, timeoutMs) => {
			const run = ownRun(runId);
			return run
				? awaitExit(run, timeoutMs)
				: fallback.waitForRunExit(runId, timeoutMs);
		},
		peekAgentToken: (agentId) =>
			lifecycle.tokens.get(agentId) ?? fallback.peekAgentToken(agentId),
		validateAgentToken(agentId, token) {
			const expected = lifecycle.tokens.get(agentId);
			if (!expected) return fallback.validateAgentToken(agentId, token);
			if (!token) return false;
			const a = Buffer.from(expected),
				b = Buffer.from(token);
			return a.length === b.length && timingSafeEqual(a, b);
		},
		pauseRun(runId) {
			if (!ownRun(runId)) fallback.pauseRun(runId);
		},
		resumeRun(runId) {
			if (!ownRun(runId)) fallback.resumeRun(runId);
		},
		resizeAgentRun(runId, cols, rows) {
			if (!ownRun(runId)) fallback.resizeAgentRun(runId, cols, rows);
		},
		startAgent(workspace, agentId, input) {
			const active =
				lifecycle.active(workspace.id, agentId)?.live ??
				fallback.getActiveRunByAgentId(workspace.id, agentId);
			if (active) return Promise.resolve(active);
			const config = fallback.peekAgentLaunchConfig(workspace.id, agentId);
			return config?.command === "hive:pi"
				? lifecycle.start(workspace, agentId, config, input.hivePort)
				: fallback.startAgent(workspace, agentId, input);
		},
		stopAgentRun(runId) {
			const run = ownRun(runId);
			if (!run) return fallback.stopAgentRun(runId);
			if (run.terminal) return;
			run.live.userStopped = true;
			context.restartPolicy.markUserStopped(runId);
			if (run.handle) detached(lifecycle.stop(run));
		},
		writeStatusPrompt(
			workspaceId,
			workerName,
			workerId,
			text,
			artifacts,
			input,
		) {
			return selected(workspaceId, orchestrator(workspaceId))
				? write(
						workspaceId,
						orchestrator(workspaceId),
						buildOrchestratorStatusPayload(
							workerName,
							text,
							artifacts,
							context.getFlags(),
						),
						input,
					).then(() => {})
				: fallback.writeStatusPrompt(
						workspaceId,
						workerName,
						workerId,
						text,
						artifacts,
						input,
					);
		},
		writeSendPrompt(
			workspaceId,
			workerId,
			dispatchId,
			from,
			description,
			text,
		) {
			if (!selected(workspaceId, workerId))
				return fallback.writeSendPrompt(
					workspaceId,
					workerId,
					dispatchId,
					from,
					description,
					text,
				);
			if (!lifecycle.active(workspaceId, workerId))
				throw new PtyInactiveError(`No active run for agent: ${workerId}`);
			const digest = buildDispatchMemoryDigestSafely({
				memoryInjection: context.memoryInjection,
				taskText: text,
				workerDescription: description,
				workspaceId,
			});
			const ids = logMemoryDigestInjection({
				agentId: workerId,
				contextType: "dispatch",
				dispatchId,
				memoryDigest: digest,
				memoryInjection: context.memoryInjection,
				workspaceId,
			});
			const rollback = () =>
				rollbackMemoryDigestInjection({
					injectionIds: ids,
					memoryInjection: context.memoryInjection,
				});
			const payload = buildWorkerDispatchPayload(
				from,
				description,
				dispatchId,
				text,
				ids ? (digest?.text ?? null) : null,
			);
			try {
				return write(workspaceId, workerId, payload, { requireActiveRun: true })
					.then(() => {})
					.catch((error) => {
						rollback();
						throw error;
					});
			} catch (error) {
				rollback();
				throw error;
			}
		},
		writeCancelPrompt(workspaceId, workerId, dispatchId, reason, input) {
			return selected(workspaceId, workerId)
				? write(
						workspaceId,
						workerId,
						buildWorkerCancelPayload(dispatchId, reason),
						input,
					).then(() => {})
				: fallback.writeCancelPrompt(
						workspaceId,
						workerId,
						dispatchId,
						reason,
						input,
					);
		},
		writeUserInputPrompt(workspaceId, text) {
			if (selected(workspaceId, orchestrator(workspaceId)))
				detached(
					write(
						workspaceId,
						orchestrator(workspaceId),
						buildOrchestratorUserInputPayload(text, context.getFlags()),
					),
				);
			else fallback.writeUserInputPrompt(workspaceId, text);
		},
		writeSystemMessageToAgent(workspaceId, agentId, text) {
			if (selected(workspaceId, agentId))
				detached(write(workspaceId, agentId, text));
			else fallback.writeSystemMessageToAgent(workspaceId, agentId, text);
		},
		deliverUserInputToOrchestrator(workspaceId, text, input) {
			return selected(workspaceId, orchestrator(workspaceId))
				? write(
						workspaceId,
						orchestrator(workspaceId),
						buildOrchestratorUserInputPayload(text, context.getFlags()),
						input,
					).then(() => {})
				: fallback.deliverUserInputToOrchestrator(workspaceId, text, input);
		},
		deliverSystemMessageToAgent(workspaceId, agentId, text, input) {
			return selected(workspaceId, agentId)
				? write(workspaceId, agentId, text, input).then(() => {})
				: fallback.deliverSystemMessageToAgent(
						workspaceId,
						agentId,
						text,
						input,
					);
		},
	};
}

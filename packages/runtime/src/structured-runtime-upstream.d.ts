declare module "@tt-a1i/hive/dist/src/server/agent-startup-instructions.js" {
	export function buildAgentStartupInstructions(input: {
		agent: import("./hive-runtime-contract.js").AgentSummary;
		workspace: import("./hive-runtime-contract.js").WorkspaceSummary;
		memoryDigest?: string | null;
		flags: import("./hive-runtime-contract.js").FeatureFlags;
	}): string;
	export function buildWorkflowAgentStartupInstructions(input: {
		agent: import("./hive-runtime-contract.js").AgentSummary;
		workspace: import("./hive-runtime-contract.js").WorkspaceSummary;
	}): string;
}
declare module "@tt-a1i/hive/dist/src/server/agent-stdin-dispatcher.js" {
	export function buildOrchestratorStatusPayload(
		workerName: string,
		text: string,
		artifacts: string[],
		flags: import("./hive-runtime-contract.js").FeatureFlags,
	): string;
	export function buildOrchestratorUserInputPayload(
		text: string,
		flags: import("./hive-runtime-contract.js").FeatureFlags,
	): string;
	export function buildWorkerDispatchPayload(
		from: string,
		description: string,
		dispatchId: string,
		text: string,
		memoryDigest?: string | null,
	): string;
	export function buildWorkerCancelPayload(
		dispatchId: string,
		reason: string,
	): string;
}
declare module "@tt-a1i/hive/dist/src/server/team-memory-injection.js" {
	type Memory = import("./hive-runtime-contract.js").TeamMemoryInjectionService;
	type Digest = import("./hive-runtime-contract.js").MemoryDigestPayload;
	export function buildMemoryDigestSafely(input: {
		contextType: "startup" | "recovery";
		memoryInjection: Memory;
		workspaceId: string;
	}): Digest | null;
	export function buildDispatchMemoryDigestSafely(input: {
		memoryInjection: Memory;
		taskText: string;
		workerDescription: string;
		workspaceId: string;
	}): Digest | null;
	export function logMemoryDigestInjection(input: {
		agentId: string;
		contextType: "startup" | "dispatch";
		dispatchId?: string;
		memoryDigest: Digest | null;
		memoryInjection: Memory;
		workspaceId: string;
	}): string[] | null;
	export function rollbackMemoryDigestInjection(input: {
		injectionIds: string[] | null;
		memoryInjection: Memory;
	}): void;
}
declare module "@tt-a1i/hive/dist/src/server/http-errors.js" {
	export class PtyInactiveError extends Error {
		constructor(message: string);
	}
	export class ConflictError extends Error {
		constructor(message: string);
	}
}

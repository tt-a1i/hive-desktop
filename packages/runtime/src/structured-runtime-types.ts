import type { DeliveryPresentation } from "./delivery-presentation.js";
import type {
	AgentLaunchConfigInput,
	LiveAgentRun,
	WorkspaceSummary,
} from "./hive-runtime-contract.js";

export interface StructuredSessionHandle {
	sessionId: string;
	sessionFile?: string;
	/** Whether the selected backend session actually retained Hive context. */
	resumed?: boolean;
	/** Acceptance into the backend queue, not completion of the model turn. */
	deliver(text: string, options?: { triggerTurn?: boolean; presentation?: DeliveryPresentation }): Promise<void>;
	/** Resume durable queued work after Hive startup/recovery becomes ready. */
	ready?(): void;
	stop(): Promise<void>;
}
export interface StructuredSessionPort {
	start(input: {
		workspace: WorkspaceSummary;
		agentId: string;
		runId: string;
		config: AgentLaunchConfigInput;
		token: string;
		hivePort: string;
		/** Session termination only. A completed turn is not a terminated run. */
		onExit(error?: unknown): void;
	}): Promise<StructuredSessionHandle>;
}
export interface StructuredRun {
	live: LiveAgentRun;
	workspaceId: string;
	token: string;
	handle?: StructuredSessionHandle;
	exited: Promise<void>;
	resolveExit(): void;
	rejectReady(error: unknown): void;
	resolveReady(): void;
	queue: Promise<unknown>;
	terminal: boolean;
	stopping?: Promise<void>;
}

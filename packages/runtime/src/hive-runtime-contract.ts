// Structural boundary for the pinned Hive 2.2.1 runtime factory.
// Mirrors the installed package's dist JavaScript because it ships no declarations.
// A newer Hive checkout has a different writeSendPrompt return type.

export type AgentStatus = "idle" | "working" | "stopped";
export type WorkerRole = "coder" | "reviewer" | "tester" | "custom";
export type WorkerSpawnSource = "workflow" | "orchestrator";

export type ControllerMode = "internal" | "codex_app";

export interface WorkspaceSummary {
	controller_mode?: ControllerMode;
	id: string;
	name: string;
	path: string;
}

export interface AgentSummary {
	id: string;
	workspaceId: string;
	name: string;
	description: string;
	role: WorkerRole | "orchestrator" | "workflow";
	status: AgentStatus;
	pendingTaskCount: number;
	avatar?: string | null;
	ephemeral?: boolean;
	spawnedBy?: WorkerSpawnSource | null;
	/** Epoch ms when this agent's current run finished post-start injection; null if not ready. */
	startupReadyAt?: number | null;
}

export type SessionIdCaptureConfig =
	| { source: "claude_project_jsonl_dir"; pattern: string }
	| { source: "codex_session_jsonl_dir"; pattern: string }
	| { source: "gemini_session_json_dir"; pattern: string }
	| { source: "opencode_session_db"; pattern: string }
	| { source: "qwen_session_json_dir"; pattern: string }
	| { source: "stdout_regex"; pattern: string }
	| { source: "banner_parse"; pattern?: string };

export interface AgentLaunchConfigInput {
	command: string;
	args?: string[];
	commandPresetId?: string | null;
	/** When set, the PTY starts in this directory instead of the workspace root. */
	cwd?: string | null;
	interactiveCommand?: string | null;
	presetAugmentationDisabled?: boolean;
	resumedSessionId?: string | null;
	resumeArgsTemplate?: string | null;
	sessionIdCapture?: SessionIdCaptureConfig | null;
}

export interface PersistedAgentRun {
	runId: string;
	agentId: string;
	status: "starting" | "running" | "exited" | "error";
	exitCode: number | null;
	pid: number | null;
	startedAt: number;
	endedAt: number | null;
}

export interface AgentRunSnapshot {
	runId: string;
	agentId: string;
	pid: number | null;
	status: PersistedAgentRun["status"];
	output: string;
	exitCode: number | null;
}

export interface LiveAgentRun extends AgentRunSnapshot {
	/** Resolves after successful startup/recovery submission or readiness-only resume.
	 * Rejects if startup fails or the run exits before readiness. Automated writes
	 * await this barrier; raw terminal input remains available during setup. */
	postStartInputReady?: Promise<void>;
	startedAt: number;
	userStopped?: boolean;
	/** Epoch ms of successful live-run startup; null on pending or failed startup. */
	startupReadyAt?: number | null;
}
type PersistedRunStatus = PersistedAgentRun["status"];

export interface AgentRunStorePort {
	close?: () => void;
	insertAgentRun: (
		runId: string,
		agentId: string,
		startedAt: number,
		pid: number | null,
		status?: PersistedRunStatus,
		exitCode?: number | null,
		endedAt?: number | null,
	) => void;
	listAgentRuns: (agentId: string) => PersistedAgentRun[];
	listLaunchConfigs: () => Array<{
		agentId: string;
		config: AgentLaunchConfigInput;
		workspaceId: string;
	}>;
	markUnfinishedRunsStale: (endedAt?: number) => void;
	deleteLaunchConfig: (workspaceId: string, agentId: string) => void;
	saveLaunchConfig: (
		workspaceId: string,
		agentId: string,
		input: AgentLaunchConfigInput,
	) => void;
	updatePersistedRun: (
		runId: string,
		status: PersistedRunStatus,
		exitCode: number | null,
		endedAt: number | null,
	) => void;
}

export interface AgentSessionStorePort {
	clearLastSessionId: (workspaceId: string, agentId: string) => void;
	getLastSessionId: (
		workspaceId: string,
		agentId: string,
	) => string | undefined;
	setLastSessionId: (
		workspaceId: string,
		agentId: string,
		sessionId: string,
	) => void;
}
export interface PtyOutputBus {
	clear: (runId: string) => void;
	publish: (runId: string, chunk: string) => void;
	subscribe: (runId: string, listener: (chunk: string) => void) => () => void;
}

export interface FeatureFlags {
	workflowsEnabled: boolean;
}

export interface MemoryDigestPayload {
	memoryIds: string[];
	text: string;
}
export interface TeamMemoryInjectionService {
	buildDigest: (input: {
		contextType: "recovery" | "startup";
		workspaceId: string;
	}) => MemoryDigestPayload | null;
	buildDispatchDigest: (input: {
		taskText: string;
		workerDescription: string;
		workspaceId: string;
	}) => MemoryDigestPayload | null;
	deleteInjections: (injectionIds: string[]) => void;
	logInjections: (input: {
		contextType: "dispatch" | "recovery" | "startup" | "manual_search";
		dispatchId?: string | null;
		memoryIds: string[];
		targetAgentIdSnapshot?: string | null;
		workspaceId: string;
	}) => string[];
}

export interface HiveAgentRuntimeFactoryContext {
	defaultRuntime: HiveAgentRuntime;
	agentRunStore: AgentRunStorePort;
	sessionStore: AgentSessionStorePort;
	onAgentExit: (workspaceId: string, agentId: string) => void;
	getAgent: (workspaceId: string, agentId: string) => AgentSummary | undefined;
	getFlags: () => FeatureFlags;
	restartPolicy: RestartPolicy;
	memoryInjection: TeamMemoryInjectionService;
}

export type HiveAgentRuntimeFactory = (
	context: HiveAgentRuntimeFactoryContext,
) => HiveAgentRuntime | undefined;
export interface RestartPolicy {
	injectPostStartMessage: (input: {
		agentId: string;
		runId: string;
		startConfig: AgentLaunchConfigInput;
		workspace: WorkspaceSummary;
		writeToRun: (runId: string, text: string) => Promise<void>;
	}) => boolean;
	/** Record that `runId` was killed at the user's request (Stop button), so a
	 *  subsequent restart does not inject the crash-recovery handover for a run
	 *  the user deliberately ended. */
	markUserStopped: (runId: string) => void;
}

export interface StartAgentOptions {
	hivePort: string;
}

export interface HiveAgentRuntime {
	close: () => Promise<void>;
	configureAgentLaunch: (
		workspaceId: string,
		agentId: string,
		input: AgentLaunchConfigInput,
	) => void;
	resolveSpawnLaunchConfig?: (
		workspaceId: string,
		cli?: string,
	) => AgentLaunchConfigInput | undefined;
	deleteAgentLaunchConfig: (workspaceId: string, agentId: string) => void;
	getActiveRunByAgentId: (
		workspaceId: string,
		agentId: string,
	) => LiveAgentRun | undefined;
	peekAgentLaunchConfig: (
		workspaceId: string,
		agentId: string,
	) => AgentLaunchConfigInput | undefined;
	findLiveRun: (runId: string) => LiveAgentRun | undefined;
	getLiveRun: (runId: string) => LiveAgentRun;
	waitForRunExit: (runId: string, timeoutMs: number) => Promise<boolean>;
	getPtyOutputBus: () => PtyOutputBus;
	listAgentRuns: (agentId: string) => PersistedAgentRun[];
	pauseRun: (runId: string) => void;
	peekAgentToken: (agentId: string) => string | undefined;
	resizeAgentRun: (runId: string, cols: number, rows: number) => void;
	resumeRun: (runId: string) => void;
	startAgent: (
		workspace: WorkspaceSummary,
		agentId: string,
		input: StartAgentOptions,
	) => Promise<LiveAgentRun>;
	stopAgentRun: (runId: string) => void;
	validateAgentToken: (agentId: string, token: string | undefined) => boolean;
	writeStatusPrompt: (
		workspaceId: string,
		workerName: string,
		workerId: string,
		text: string,
		artifacts: string[],
		input?: { requireActiveRun?: boolean },
	) => Promise<void>;
	writeSendPrompt: (
		workspaceId: string,
		workerId: string,
		dispatchId: string,
		fromAgentName: string,
		workerDescription: string,
		text: string,
	) => Promise<void>;
	writeCancelPrompt: (
		workspaceId: string,
		workerId: string,
		dispatchId: string,
		reason: string,
		input?: { requireActiveRun?: boolean },
	) => Promise<void>;
	writeUserInputPrompt: (workspaceId: string, text: string) => void;
	writeSystemMessageToAgent: (
		workspaceId: string,
		agentId: string,
		text: string,
	) => void;
	deliverUserInputToOrchestrator: (
		workspaceId: string,
		text: string,
		input?: { requireActiveRun?: boolean },
	) => Promise<void>;
	/** Awaitable opaque delivery — used to drain the report outbox so an entry
	 *  is marked delivered only after the PTY write resolves. */
	deliverSystemMessageToAgent: (
		workspaceId: string,
		agentId: string,
		text: string,
		input?: { requireActiveRun?: boolean; beforeWrite?: () => boolean },
	) => Promise<void>;
}

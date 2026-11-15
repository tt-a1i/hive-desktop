import WebSocket from "ws";
import { isAbsolute } from "node:path";
import { runHiveCommand } from "@tt-a1i/hive/dist/src/cli/hive.js";
import { getDefaultRoleDescription } from '@tt-a1i/hive/dist/src/server/role-templates.js';
import { resolveWorkspaceUiLanguage } from '@tt-a1i/hive/dist/src/server/workspace-ui-language.js';
import type { HiveAgentRuntimeFactory } from "./hive-runtime-contract.js";
export interface DesktopMemberInput { name: string; role: import('./hive-runtime-contract.js').WorkerRole; description?: string }

export class HiveHttpError extends Error {
	constructor(
		public readonly status: number,
		public readonly body: unknown,
	) {
		const detail = body && typeof body === "object" && "error" in body
			&& typeof body.error === "string" ? body.error.trim() : "";
		super(`Hive request failed (${status})${detail ? `: ${detail}` : ""}`);
		this.name = "HiveHttpError";
	}
}

export interface HiveUpload { record: { mimeType: string; originalName: string }; data: Buffer }

export interface HiveUploadLocation {localPath:string;record:{mimeType:string;originalName:string;sizeBytes:number}}

export interface HiveUserInputRow { type: 'user_input' | 'user_input_cancelled'; text: string }
export interface SavedScriptContext {scriptRunId: string; cwd: string; command: string; sessionId?: string}
export interface SavedScriptObserver {onCreated(runId: string): void; onOutput(chunk: string): void; onExit(exitCode: number | null, signal?: number): void}
export interface HiveHost {
    readonly projectScriptState: import('./project-script-settings.js').ProjectScriptStatePort;
    executionLedgerRevision(): string;
    resumeExecutionDeliveries(): void;
    startSavedScript(workspaceId: string, context: SavedScriptContext, observer: SavedScriptObserver): Promise<{runId: string}>;
    stopSavedScript(workspaceId: string, scriptRunId: string, nativeRunId: string): void;
    startSessionShell(workspaceId: string, context: {sessionId: string; cwd: string}): Promise<{runId: string}>;
    getUserInput(workspaceId: string, agentId: string, sequence: number): HiveUserInputRow | undefined;
    mutateUserInput(workspaceId: string, agentId: string, sequence: number, expectedText: string, replacement: string | null, persistSpool: () => void): void;
    configureAgentLaunch: import("./hive-runtime-contract.js").HiveAgentRuntime["configureAgentLaunch"];
    createMember(workspaceId: string, input: DesktopMemberInput, command: string): { id: string };
    locateUpload(workspaceId:string,id:string):Promise<HiveUploadLocation | undefined>;
    readUpload(workspaceId: string, id: string): Promise<HiveUpload | null>;
    getNavigation(): string | null;
    saveNavigation(value: string): void;
	readonly port: number;
	request(method: string, path: string, body?: unknown): Promise<unknown>;
	findReportPresentation(workspaceId: string, agentId: string, dispatchId: string, payload: string): unknown;
	isSettledDispatch(workspaceId: string, agentId: string, dispatchId: string): boolean;
	withWorkspaceWorkflowIdle<T>(workspaceId: string, operation: () => Promise<T>): Promise<T>;
	openTerminalSocket(runId: string, channel: 'io' | 'control', clientId: string, cols: number, rows: number): WebSocket;
	close(): Promise<void>;
}

// Hive currently resolves its data directory through process.env, including when
// creating agent environments. Keep it set for the entire host lifetime.
let hostActive = false;

export async function createHiveHost({
	dataDir,
	agentRuntimeFactory,
}: {
	dataDir: string;
	agentRuntimeFactory?: HiveAgentRuntimeFactory;
}): Promise<HiveHost> {
	if (!isAbsolute(dataDir))
		throw new Error("Hive desktop dataDir must be an absolute path");
	if (hostActive) throw new Error("Only one Hive host may run in this process");
	hostActive = true;
	const previousDataDir = process.env.HIVE_DATA_DIR;
	process.env.HIVE_DATA_DIR = dataDir;
	const release = () => {
		if (previousDataDir === undefined) delete process.env.HIVE_DATA_DIR;
		else process.env.HIVE_DATA_DIR = previousDataDir;
		hostActive = false;
	};

	let runtime: Awaited<ReturnType<typeof runHiveCommand>>;
	try {
		runtime = await runHiveCommand(["--port", "0"], {
			openBrowser: false,
			...(agentRuntimeFactory ? { agentRuntimeFactory } : {}),
		});
	} catch (error) {
		release();
		throw error;
	}
	const base = `http://127.0.0.1:${runtime.port}`;
	let cookie: string;
	try {
		const session = await fetch(`${base}/api/ui/session`, {
			redirect: "error",
		});
		if (!session.ok)
			throw new HiveHttpError(session.status, await session.text());
		const value = session.headers.get("set-cookie")?.split(";", 1)[0];
		await session.body?.cancel();
		if (!value?.startsWith("hive_ui_token="))
			throw new Error("Hive did not issue a UI session");
		cookie = value;
	} catch (error) {
		try {
			await runtime.close();
		} finally {
			release();
		}
		throw error;
	}

	let closing: Promise<void> | undefined;
	return {
		port: runtime.port,
        projectScriptState: {
            read(key, workspaceId) {if (closing) throw new Error('Hive host is closed'); return runtime.store.settings.readProjectScriptState(key, workspaceId);},
            compareAndSwap(key, expected, next, workspaceId) {if (closing) throw new Error('Hive host is closed'); return runtime.store.settings.compareAndSwapProjectScriptState(key, expected, next, workspaceId);},
        },
        executionLedgerRevision() {
            if (closing) throw new Error("Hive host is closed");
            return (runtime.store as typeof runtime.store & {executionLedgerRevision(): string}).executionLedgerRevision();
        },
        resumeExecutionDeliveries() {
            if (!closing) (runtime.store as typeof runtime.store & {resumeExecutionDeliveries(): void}).resumeExecutionDeliveries();
        },
        startSavedScript(workspaceId, context, observer) {
            if (closing) throw new Error('Hive host is closed');
            return runtime.store.startSavedScript(workspaceId, context, observer);
        },
        stopSavedScript(workspaceId, scriptRunId, nativeRunId) {
            if (closing) throw new Error('Hive host is closed');
            runtime.store.stopSavedScript(workspaceId, scriptRunId, nativeRunId);
        },
        startSessionShell(workspaceId, context) {
            if (closing) throw new Error("Hive host is closed");
            return runtime.store.startSessionShell(workspaceId, context);
        },
        getUserInput(workspaceId, agentId, sequence) {
            if (closing) throw new Error('Hive host is closed');
            return runtime.store.getDesktopUserInput(workspaceId, agentId, sequence);
        },
        mutateUserInput(workspaceId, agentId, sequence, expectedText, replacement, persistSpool) {
            if (closing) throw new Error('Hive host is closed');
            runtime.store.mutateDesktopUserInput(workspaceId, agentId, sequence, expectedText, replacement, persistSpool);
        },
        configureAgentLaunch(workspaceId, agentId, input) {
            if (closing) throw new Error('Hive host is closed');
            runtime.store.configureAgentLaunch(workspaceId, agentId, input);
        },
        createMember(workspaceId, input, command) {
            if (closing) throw new Error('Hive host is closed');
            return runtime.store.addWorkerWithLaunch(workspaceId, {
                ...input, avatar: null,
                description: input.description ?? getDefaultRoleDescription(input.role, resolveWorkspaceUiLanguage(runtime.store.settings, workspaceId)),
            }, { command, args: [], commandPresetId: null });
        },
        locateUpload: (workspaceId,id) => {
            if (closing) throw new Error("Hive host is closed");
            return runtime.store.locateWorkspaceUpload(workspaceId,id);
        },
        readUpload: (workspaceId, id) => {
            if (closing) throw new Error("Hive host is closed");
            return runtime.store.readWorkspaceUpload(workspaceId, id);
        },
        getNavigation: () => {
            if (closing) throw new Error("Hive host is closed");
            return runtime.store.settings.getAppState("desktop_navigation")?.value ?? null;
        },
        saveNavigation: value => {
            if (closing) throw new Error("Hive host is closed");
            runtime.store.settings.setAppState("desktop_navigation", value);
        },
		async request(method, path, body) {
			if (closing) throw new Error("Hive host is closed");
			// Parse before attaching credentials: URL normalization must not escape
			// the loopback origin or the API namespace.
			if (!path.startsWith("/api/"))
				throw new Error("Hive requests require an /api/ path");
			const url = new URL(path, base);
			if (
				url.origin !== base ||
				!url.pathname.startsWith("/api/") ||
				url.hash
			) {
				throw new Error("Invalid Hive API path");
			}
			const response = await fetch(url, {
				method,
				headers: {
					cookie,
					origin: base,
					...(body === undefined ? {} : { "content-type": "application/json" }),
				},
				...(body === undefined ? {} : { body: JSON.stringify(body) }),
				redirect: "error",
			});
			const text = await response.text();
			const result: unknown =
				text === ""
					? null
					: response.headers.get("content-type")?.includes("application/json")
						? JSON.parse(text)
						: text;
			if (!response.ok) throw new HiveHttpError(response.status, result);
			return result;
		},
		openTerminalSocket(runId, channel, clientId, cols, rows) {
            if (closing) throw new Error("Hive host is closed");
            const url = new URL(`/ws/terminal/${encodeURIComponent(runId)}/${channel}`, base.replace('http:', 'ws:'));
            url.search = new URLSearchParams({ clientId, cols: String(cols), rows: String(rows), restoreBarrier: '1' }).toString();
            return new WebSocket(url, { headers: { cookie }, origin: base, handshakeTimeout: 10000, maxPayload: 8 * 1024 * 1024 });
        },
		findReportPresentation: runtime.store.findReportPresentation,
		isSettledDispatch(workspaceId, agentId, dispatchId) {
			const dispatch = runtime.store.getDispatch(workspaceId, dispatchId);
			return dispatch?.toAgentId === agentId && (dispatch.status === 'reported' || dispatch.status === 'cancelled');
		},
		withWorkspaceWorkflowIdle(workspaceId, operation) {
			if (closing) return Promise.reject(new Error("Hive host is closed"));
			return runtime.store.withWorkspaceWorkflowIdle(workspaceId, operation);
		},
		close() {
			closing ??= runtime.close().finally(release);
			return closing;
		},
	};
}

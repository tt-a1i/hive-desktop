import WebSocket from "ws";
import { isAbsolute } from "node:path";
import { readPackageVersion } from "@tt-a1i/hive/dist/src/server/package-version.js";
import { createVersionService } from "@tt-a1i/hive/dist/src/server/version-service.js";
import { runHiveCommand } from "@tt-a1i/hive/dist/src/cli/hive.js";
import { HttpError } from "@tt-a1i/hive/dist/src/server/http-errors.js";
import type { DesktopDraftCopyReceipt, DesktopDraftUploadCopy, WorkspaceUploadRecord } from "@tt-a1i/hive/dist/src/server/workspace-upload-store.js";
import type { DispatchMessageRecord } from "@tt-a1i/hive/dist/src/shared/team-collaboration.js";
import type { DispatchRecord } from "@tt-a1i/hive/dist/src/server/dispatch-ledger-store.js";
import { getDefaultRoleDescription } from '@tt-a1i/hive/dist/src/server/role-templates.js';
import { resolveWorkspaceUiLanguage } from '@tt-a1i/hive/dist/src/server/workspace-ui-language.js';
import type { HiveAgentRuntimeFactory } from "./hive-runtime-contract.js";
import type { ChatBackend } from './chat-index.js';
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

export interface HiveUploadLocation {
    localPath: string;
    record: {mimeType: string; originalName: string; sizeBytes: number};
    sourceDigestVersion: number;
    sourceSha256: string | null;
}

export interface HiveUserInputRow { type: 'user_input' | 'user_input_cancelled'; text: string }
interface InboxQuestion { workspaceId: string; dispatchId: string; messageId: string }
type ActionableMessageSource = {listActionableDispatchMessages(workspaceId: string): DispatchMessageRecord[]};
type OpenDispatchSource = {listOpenDispatches(workspaceId: string):
    Pick<DispatchRecord, 'id' | 'workspaceId' | 'status' | 'submittedAt' | 'toAgentId'>[]};

// Match Hive's action-center definition of a wait, not a task timeout.
const WAITING_REPORT_MS = 10 * 60 * 1_000;

export function withInboxWaitingReports(
    result: unknown, workspaceId: string, store: OpenDispatchSource,
): unknown {
    if (!result || typeof result !== 'object' || !('workspace_id' in result) || result.workspace_id !== workspaceId
        || !('attention' in result) || !Array.isArray(result.attention)
        || !('generated_at' in result) || typeof result.generated_at !== 'number' || !Number.isSafeInteger(result.generated_at)
        || !('summary' in result) || !result.summary || typeof result.summary !== 'object'
        || !('waiting_reports' in result.summary)) return result;
    const {attention, generated_at: now, summary} = result;
    const count = summary.waiting_reports;
    const isWaitingReport = (row: {kind?: unknown} | null) => row?.kind === 'dispatch_waiting_report';
    const firstWaiting = attention.findIndex(isWaitingReport);
    if (firstWaiting < 0 || typeof count !== 'number' || count <= attention.filter(isWaitingReport).length) return result;

    const workerNames = new Map<string, string>();
    if ('workers' in result && Array.isArray(result.workers)) {
        for (const worker of result.workers) {
            if (typeof worker?.id === 'string' && typeof worker.name === 'string') workerNames.set(worker.id, worker.name);
        }
    }
    const waiting = store.listOpenDispatches(workspaceId).flatMap(dispatch => {
        const submitted = dispatch.submittedAt;
        if (dispatch.workspaceId !== workspaceId || dispatch.status !== 'submitted' || submitted === null
            || submitted > now - WAITING_REPORT_MS) return [];
        return [{
            kind: 'dispatch_waiting_report',
            severity: 'info',
            dispatch_id: dispatch.id,
            submitted_at: submitted,
            minutes_ago: Math.floor((now - submitted) / 60_000),
            worker_id: dispatch.toAgentId,
            worker_name: workerNames.get(dispatch.toAgentId) ?? null,
        }];
    });
    return {
        ...result,
        summary: {...summary, waiting_reports: waiting.length},
        attention: [...attention.slice(0, firstWaiting), ...waiting, ...attention.slice(firstWaiting).filter(row => !isWaitingReport(row))],
    };
}

export function withInboxMessageSenders(
    result: unknown, workspaceId: string, store: ActionableMessageSource,
): unknown {
    if (!result || typeof result !== 'object' || !('workspace_id' in result) || result.workspace_id !== workspaceId
        || !('attention' in result) || !Array.isArray(result.attention)) return result;
    const messages = new Map(store.listActionableDispatchMessages(workspaceId)
        .filter(message => message.workspaceId === workspaceId)
        .map(message => [message.id, message]));
    return {...result, attention: result.attention.map(row => {
        if ((row?.kind !== 'question_waiting_answer' && row?.kind !== 'message_delivery_pending')
            || typeof row.message_id !== 'string') return row;
        const message = messages.get(row.message_id);
        return message && message.dispatchId === row.dispatch_id
            ? {...row, message_sender_id: message.fromAgentId, message_created_at: message.createdAt}
            : row;
    })};
}

export function withInboxDispatchBackends(
    result: unknown, workspaceId: string, lookup: (workspaceId: string, dispatchId: string) => ChatBackend | undefined,
): unknown {
    if (!result || typeof result !== 'object' || !('workspace_id' in result) || result.workspace_id !== workspaceId
        || !('attention' in result) || !Array.isArray(result.attention)) return result;
    const backends = new Map<string, ChatBackend | undefined>();
    return {...result, attention: result.attention.map(row => {
        if (!row || typeof row !== 'object') return row;
        const id = row.dispatch_id;
        const attributed = (row.kind === 'question_waiting_answer' || row.kind === 'message_delivery_pending'
            || row.kind === 'dispatch_waiting_report')
            && typeof id === 'string' && /^[\da-f]{8}(?:-[\da-f]{4}){3}-[\da-f]{12}$/i.test(id);
        if (!attributed) return {...row, dispatch_backend: null};
        if (!backends.has(id)) backends.set(id, lookup(workspaceId, id));
        return {...row, dispatch_backend: backends.get(id) ?? null};
    })};
}

export interface SavedScriptContext {scriptRunId: string; cwd: string; command: string; sessionId?: string}
export interface SavedScriptObserver {onCreated(runId: string): void; onOutput(chunk: string): void; onExit(exitCode: number | null, signal?: number): void}
export interface HiveHost {
    inboxQuestions(workspaceIds: readonly string[]): InboxQuestion[];
    readonly projectScriptState: import('./project-script-settings.js').ProjectScriptStatePort;
    desktopConversationSwitchState(workspaceId:string,agentId?:string): import('./hive-runtime-contract.js').DesktopConversationSwitchState;
    executionLedgerRevision(): string;
    resumeExecutionDeliveries(): void;
    startSavedScript(workspaceId: string, context: SavedScriptContext, observer: SavedScriptObserver): Promise<{runId: string}>;
    stopSavedScript(workspaceId: string, scriptRunId: string, nativeRunId: string): void;
    startSessionShell(workspaceId: string, context: {sessionId: string; cwd: string}): Promise<{runId: string}>;
    getUserInput(workspaceId: string, agentId: string, sequence: number): HiveUserInputRow | undefined;
    mutateUserInput(workspaceId: string, agentId: string, sequence: number, expectedText: string, replacement: string | null, persistSpool: () => void): void;
    configureAgentLaunch: import("./hive-runtime-contract.js").HiveAgentRuntime["configureAgentLaunch"];
    createMember(workspaceId: string, input: DesktopMemberInput, launch: string | import('./hive-runtime-contract.js').AgentLaunchConfigInput): { id: string };
    saveDesktopUpload(workspaceId: string, originalName: string, mimeType: string, data: Buffer): Promise<WorkspaceUploadRecord>;
    getDesktopDraftCopy(id: string): DesktopDraftCopyReceipt | undefined;
    reserveDesktopDraftCopy(id: string, receipt: DesktopDraftCopyReceipt): void;
    saveDesktopDraftUploads(id: string, receipt: DesktopDraftCopyReceipt, copies: AsyncIterable<DesktopDraftUploadCopy>, signal: AbortSignal): Promise<DesktopDraftCopyReceipt>;
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
			// The embedded host is replaced with the signed desktop package.
			versionService: createVersionService({
				fetchLatestVersion: async () => readPackageVersion(),
				createUpdateInstallPlan: () => ({
					canRunHiveUpdate: false,
					installSource: "unknown",
					installCommand: "",
					installArgs: [],
					manualCommand: "",
					note: "This runtime is updated with Hive Desktop.",
				}),
			}),
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
        inboxQuestions(workspaceIds) {
            if (closing) throw new Error('Hive host is closed');
            const store = runtime.store as typeof runtime.store & ActionableMessageSource & {
                listWorkspaces(): Array<{ id: string }>;
            };
            const workspaces = new Set(store.listWorkspaces().map(workspace => workspace.id));
            return [...new Set(workspaceIds)].filter(workspaceId => workspaces.has(workspaceId)).flatMap(workspaceId =>
                // Keep the Action Center's question vs. queued-delivery distinction.
                store.listActionableDispatchMessages(workspaceId)
                    .filter(message => message.kind === 'question' && message.deliveryState !== 'queued')
                    .map(message => ({ workspaceId, dispatchId: message.dispatchId, messageId: message.id })));
        },
        projectScriptState: {
            read(key, workspaceId) {if (closing) throw new Error('Hive host is closed'); return runtime.store.settings.readProjectScriptState(key, workspaceId);},
            compareAndSwap(key, expected, next, workspaceId) {if (closing) throw new Error('Hive host is closed'); return runtime.store.settings.compareAndSwapProjectScriptState(key, expected, next, workspaceId);},
        },
        desktopConversationSwitchState(workspaceId,agentId) {
            if (closing) throw Object.assign(new Error('Hive host is closing or closed'), {code:'DESKTOP_SWITCH_CLOSING'});
            return (runtime.store as typeof runtime.store & {desktopConversationSwitchState(workspaceId:string,agentId?:string): import('./hive-runtime-contract.js').DesktopConversationSwitchState}).desktopConversationSwitchState(workspaceId,agentId);
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
        createMember(workspaceId, input, launch) {
            if (closing) throw new Error('Hive host is closed');
            return runtime.store.addWorkerWithLaunch(workspaceId, {
                ...input, avatar: null,
                description: input.description ?? getDefaultRoleDescription(input.role, resolveWorkspaceUiLanguage(runtime.store.settings, workspaceId)),
            }, typeof launch === 'string' ? {command: launch, args: [], commandPresetId: null} : launch);
        },
        async saveDesktopUpload(workspaceId, originalName, mimeType, data) {
            if (closing) throw new Error("Hive host is closed");
            try {
                return await runtime.store.saveWorkspaceUpload({workspaceId, originalName, mimeType, data, captureDesktopSourceDigest: true});
            } catch (error) {
                if (error instanceof HttpError) throw new HiveHttpError(error.statusCode, {error: error.message});
                throw error;
            }
        },
        getDesktopDraftCopy(id) {
            if (closing) throw new Error("Hive host is closed");
            return runtime.store.getDesktopDraftCopy(id);
        },
        reserveDesktopDraftCopy(id, receipt) {
            if (closing) throw new Error("Hive host is closed");
            runtime.store.reserveDesktopDraftCopy(id, receipt);
        },
        saveDesktopDraftUploads(id, receipt, copies, signal) {
            if (closing) throw new Error("Hive host is closed");
            return runtime.store.saveDesktopDraftUploads(id, receipt, copies, signal);
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
			// The preview is capped. Attach sender identity to every current
			// attention message without another runtime request or more text.
			const inbox = method === "GET" && /^\/api\/ui\/workspaces\/([^/]+)\/action-center$/.exec(url.pathname);
			if (inbox) {
				const workspace = decodeURIComponent(inbox[1]!);
				const store = runtime.store as typeof runtime.store & ActionableMessageSource & OpenDispatchSource;
				return withInboxMessageSenders(withInboxWaitingReports(result, workspace, store), workspace, store);
			}
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

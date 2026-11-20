import WebSocket from "ws";
import { isAbsolute, join } from "node:path";
import { Database } from "@tt-a1i/hive/dist/src/server/sqlite.js";
import { readDispatchMessage } from "@tt-a1i/hive/dist/src/server/dispatch-message-store.js";
import { dispatchMessageEligibilitySql } from "@tt-a1i/hive/dist/src/server/dispatch-message-delivery-policy.js";
import { serializeDispatchMessage } from "@tt-a1i/hive/dist/src/server/dispatch-message-serializer.js";
import { readPackageVersion } from "@tt-a1i/hive/dist/src/server/package-version.js";
import { createVersionService } from "@tt-a1i/hive/dist/src/server/version-service.js";
import { runHiveCommand } from "@tt-a1i/hive/dist/src/cli/hive.js";
import { HttpError } from "@tt-a1i/hive/dist/src/server/http-errors.js";
import type { DispatchMessageRecord } from "@tt-a1i/hive/dist/src/shared/team-collaboration.js";
import type { DesktopDraftCopyReceipt, DesktopDraftUploadCopy, WorkspaceUploadRecord } from "@tt-a1i/hive/dist/src/server/workspace-upload-store.js";
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
interface InboxDispatchInput { dispatchId: string; sequence: number | null; createdAt: number | null }
interface InboxWorkflowFailure { id: string; name: string | null; finishedAt: number }
interface InboxActionableRow {
    kind: 'question_waiting_answer' | 'message_delivery_pending';
    dispatch_id: string;
    message_id: string;
    worker_id?: string;
    message_sender_id?: string;
    message_created_at?: number;
    message_preview?: string | null;
    dispatch_backend?: ChatBackend | null;
    severity?: 'warning';
}
interface InboxActionablePage {
    workspaceId: string;
    questionsOnly: boolean;
    through: number;
    nextAfter: number | null;
    rows: InboxActionableRow[];
}
type WaitingReportDispatch = Pick<DispatchRecord, 'id' | 'workspaceId' | 'status' | 'submittedAt' | 'toAgentId' | 'label' | 'text'>;
type OpenDispatchSource = {
    listOpenDispatches(workspaceId: string): WaitingReportDispatch[];
    getDispatch(workspaceId: string, dispatchId: string): WaitingReportDispatch | undefined;
};

// Match Hive's action-center definition of a wait, not a task timeout.
const WAITING_REPORT_MS = 10 * 60 * 1_000;
const INBOX_ACTIONABLE_LIMIT = 100;
const INBOX_PREVIEW_LIMIT = 96;
const INBOX_MESSAGE_SAMPLE = 384;
const INBOX_WORKFLOW_FAILURE_LIMIT = 20;

function inboxLinePreview(text: string | null | undefined): string | null {
    const line = text?.trimStart().split(/[\r\n\u2028\u2029]/u, 1)[0]?.replace(/\s+/gu, ' ').trim();
    if (!line) return null;
    let length = 0;
    let end = 0;
    for (const character of line) {
        if (length === INBOX_PREVIEW_LIMIT) return `${line.slice(0, end).trimEnd()}…`;
        end += character.length;
        length++;
    }
    return line;
}

function dispatchTitlePreview(dispatch: WaitingReportDispatch): string | null {
    return inboxLinePreview(dispatch.label) ?? inboxLinePreview(dispatch.text);
}

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
    if (firstWaiting < 0 || typeof count !== 'number') return result;
    const exactWaiting = (dispatch: WaitingReportDispatch) => dispatch.workspaceId === workspaceId
        && dispatch.status === 'submitted' && dispatch.submittedAt !== null
        && dispatch.submittedAt <= now - WAITING_REPORT_MS;
    if (count <= attention.filter(isWaitingReport).length) {
        return {...result, attention: attention.map(row => {
            if (!isWaitingReport(row) || typeof row.dispatch_id !== 'string') return row;
            const dispatch = store.getDispatch(workspaceId, row.dispatch_id);
            return dispatch && exactWaiting(dispatch) && dispatch.id === row.dispatch_id
                && dispatch.toAgentId === row.worker_id && dispatch.submittedAt === row.submitted_at
                ? {...row, task_title: dispatchTitlePreview(dispatch)}
                : row;
        })};
    }

    const workerNames = new Map<string, string>();
    if ('workers' in result && Array.isArray(result.workers)) {
        for (const worker of result.workers) {
            if (typeof worker?.id === 'string' && typeof worker.name === 'string') workerNames.set(worker.id, worker.name);
        }
    }
    const waiting = store.listOpenDispatches(workspaceId).flatMap(dispatch => {
        const submitted = dispatch.submittedAt;
        if (submitted === null || !exactWaiting(dispatch)) return [];
        return [{
            kind: 'dispatch_waiting_report',
            severity: 'info',
            dispatch_id: dispatch.id,
            submitted_at: submitted,
            minutes_ago: Math.floor((now - submitted) / 60_000),
            worker_id: dispatch.toAgentId,
            worker_name: workerNames.get(dispatch.toAgentId) ?? null,
            task_title: dispatchTitlePreview(dispatch),
        }];
    });
    return {
        ...result,
        summary: {...summary, waiting_reports: waiting.length},
        attention: [...attention.slice(0, firstWaiting), ...waiting, ...attention.slice(firstWaiting).filter(row => !isWaitingReport(row))],
    };
}

function withInboxActionablePage(result: unknown, page: InboxActionablePage): unknown {
    const {workspaceId, through, nextAfter, rows} = page;
    if (!result || typeof result !== 'object' || !('workspace_id' in result) || result.workspace_id !== workspaceId
        || !('attention' in result) || !Array.isArray(result.attention)) return result;
    const other = result.attention.filter(row => row?.kind !== 'question_waiting_answer' && row?.kind !== 'message_delivery_pending');
    return {...result, attention: [...rows, ...other], inbox_actionable_cursor: {through, nextAfter}};
}

function readInboxActionablePage(
    db: Database, workspaceId: string, questionsOnly: boolean, after: number, through: number,
): InboxActionablePage {
    const scope = questionsOnly
        ? "m.kind = 'question' AND COALESCE(o.state, 'recorded') <> 'queued'"
        : "(m.kind = 'question' OR o.state = 'queued')";
    // The eligibility policy and unanswered predicate match Hive's actionable
    // store. Classify before LIMIT so nonrows cannot consume a page slot.
    // Use the rowid range directly instead of sorting the workspace time index
    // on every page.
    const projection = questionsOnly
        ? ''
        : `, m.recipient_agent_id AS worker_id, m.from_agent_id AS message_sender_id, m.created_at AS message_created_at,
            substr(m.text, 1, ${INBOX_MESSAGE_SAMPLE}) AS message_sample`;
    const records = db.prepare(`
        SELECT m.rowid AS cursor,
            CASE WHEN o.state = 'queued' THEN 'message_delivery_pending' ELSE 'question_waiting_answer' END AS kind,
            m.dispatch_id, m.id AS message_id ${projection}
        FROM dispatch_messages m NOT INDEXED
        LEFT JOIN dispatch_message_outbox o ON o.message_id = m.id
        WHERE m.workspace_id = ? AND m.rowid > ? AND m.rowid <= ?
          AND COALESCE(o.state, 'recorded') <> 'cancelled'
          AND ${dispatchMessageEligibilitySql}
          AND ((m.kind = 'question' AND NOT EXISTS (
            SELECT 1 FROM dispatch_messages answer WHERE answer.reply_to = m.id AND answer.kind = 'answer'
          )) OR o.state IN ('queued','delivering'))
          AND ${scope}
        ORDER BY m.rowid LIMIT ?
    `).all(workspaceId, after, through, INBOX_ACTIONABLE_LIMIT + 1) as (InboxActionableRow & {cursor: number; message_sample?: string})[];
    const page = records.slice(0, INBOX_ACTIONABLE_LIMIT);
    const rows = page.map(({cursor: _cursor, message_sample, ...row}): InboxActionableRow =>
        questionsOnly ? row : {...row, severity: 'warning', message_preview: inboxLinePreview(message_sample)});
    const nextAfter = records.length > page.length ? page[page.length - 1]!.cursor : null;
    return {workspaceId, questionsOnly, through, nextAfter, rows};
}

export function withInboxActionableBackends(
    page: InboxActionablePage, lookup: (workspaceId: string, dispatchId: string) => ChatBackend | undefined,
): InboxActionablePage {
    if (page.questionsOnly) return page;
    const backends = new Map<string, ChatBackend | null>();
    return {...page, rows: page.rows.map(row => {
        if (!backends.has(row.dispatch_id)) backends.set(row.dispatch_id, lookup(page.workspaceId, row.dispatch_id) ?? null);
        return {...row, dispatch_backend: backends.get(row.dispatch_id) ?? null};
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
    inboxWorkflowFailures(workspaceId: string): {workspaceId: string; runs: InboxWorkflowFailure[]; limited: boolean};
    inboxDispatchMessage(workspaceId: string, dispatchId: string, messageId: string): ReturnType<typeof serializeDispatchMessage> | null;
    inboxDispatchInput(workspaceId: string, dispatchIds: unknown, timestampDispatchIds?: unknown): {workspaceId: string; inputs: InboxDispatchInput[]};
    inboxQuestions(workspaceIds: readonly string[]): InboxQuestion[];
    inboxQuestionPages(workspaceIds: readonly string[]): InboxActionablePage[];
    inboxActionablePage(workspaceId: string, after: unknown, through: unknown, questionsOnly: unknown): InboxActionablePage;
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
	let inboxStore: Database | undefined;
    const readInboxStore = () => inboxStore ??= new Database(join(dataDir, 'runtime.sqlite'), {readOnly: true});
    const workspaceScope = () => {
        const store = runtime.store as typeof runtime.store & {listWorkspaces(): {id: string}[]};
        return new Set(store.listWorkspaces().map(workspace => workspace.id));
    };
    // Freeze the insertion boundary across a sweep. The desktop uses this
    // handle only as a bound; rows are always scoped by workspace separately.
    const actionableThrough = (db: Database) =>
        (db.prepare('SELECT COALESCE(MAX(rowid), 0) AS cursor FROM dispatch_messages').get() as {cursor: number}).cursor;
	return {
		port: runtime.port,
        inboxWorkflowFailures(workspaceId) {
            if (closing) throw new Error('Hive host is closed');
            if (!workspaceScope().has(workspaceId)) throw new Error('Unknown Inbox workspace');
            // Order by when the failure occurred. A long-running workflow can
            // finish after newer runs. Nested runs stay in Workflows.
            const records = readInboxStore().prepare(`
                SELECT id, substr(name, 1, ${INBOX_MESSAGE_SAMPLE}) AS name, finished_at AS finishedAt
                FROM workflow_runs
                WHERE workspace_id = ? AND parent_run_id IS NULL AND status = 'failed' AND finished_at > 0
                ORDER BY finished_at DESC, rowid DESC LIMIT ?
            `).all(workspaceId, INBOX_WORKFLOW_FAILURE_LIMIT + 1) as InboxWorkflowFailure[];
            return {workspaceId, runs: records.slice(0, INBOX_WORKFLOW_FAILURE_LIMIT)
                .map(run => ({...run, name: inboxLinePreview(run.name)})), limited: records.length > INBOX_WORKFLOW_FAILURE_LIMIT};
        },
        inboxDispatchMessage(workspaceId, dispatchId, messageId) {
            if (closing) throw new Error('Hive host is closed');
            if (!runtime.store.getDispatch(workspaceId, dispatchId)) return null;
            const message = readDispatchMessage(readInboxStore(), workspaceId, messageId);
            return message?.id === messageId && message.workspaceId === workspaceId
                && message.dispatchId === dispatchId && message.deliveryState !== 'cancelled'
                ? serializeDispatchMessage(message)
                : null;
        },
        inboxDispatchInput(workspaceId, dispatchIds, timestampDispatchIds) {
            if (closing) throw new Error('Hive host is closed');
            if (!Array.isArray(dispatchIds) || dispatchIds.length > 64 || dispatchIds.some(id =>
                typeof id !== 'string' || !/^[\da-f]{8}(?:-[\da-f]{4}){3}-[\da-f]{12}$/i.test(id))) {
                throw new Error('Expected up to 64 dispatch IDs');
            }
            const ids = new Set<string>(dispatchIds);
            if (timestampDispatchIds !== undefined && (!Array.isArray(timestampDispatchIds)
                || timestampDispatchIds.length > 64
                || timestampDispatchIds.some(id => typeof id !== 'string' || !ids.has(id)))) {
                throw new Error('Timestamp dispatch IDs must be among the requested dispatches');
            }
            const timestamps = new Set<string>(timestampDispatchIds as string[] | undefined);
            if (!ids.size) return {workspaceId, inputs: []};
            const db = readInboxStore();
            // Use the dispatch/sequence index for the usual cursor. Only an
            // older timestamp-only anchor needs the exact dispatch's max time.
            const eligible = `
                FROM dispatch_messages m
                LEFT JOIN dispatch_message_outbox o ON o.message_id = m.id
                WHERE m.dispatch_id = ? AND m.workspace_id = ?
                  AND COALESCE(o.state, 'recorded') <> 'cancelled'
            `;
            const newest = db.prepare(`SELECT m.sequence ${eligible} ORDER BY m.sequence DESC LIMIT 1`);
            const newestTime = timestamps.size ? db.prepare(`SELECT MAX(m.created_at) AS createdAt ${eligible}`) : undefined;
            const inputs = [...ids].map(dispatchId => {
                if (!runtime.store.getDispatch(workspaceId, dispatchId)) {
                    return {dispatchId, sequence: null, createdAt: null};
                }
                const sequence = (newest.get(dispatchId, workspaceId) as {sequence: number} | undefined)?.sequence;
                const createdAt = timestamps.has(dispatchId)
                    ? (newestTime?.get(dispatchId, workspaceId) as {createdAt: number | null} | undefined)?.createdAt
                    : undefined;
                return {dispatchId, sequence: sequence ?? null, createdAt: createdAt ?? null};
            });
            return {workspaceId, inputs};
        },
        inboxActionablePage(workspaceId, after, through, questionsOnly) {
            if (closing) throw new Error('Hive host is closed');
            if (!workspaceScope().has(workspaceId)) {
                throw new Error('Unknown Inbox workspace');
            }
            if (questionsOnly !== undefined && typeof questionsOnly !== 'boolean') {
                throw new Error('Expected a boolean Inbox question mode');
            }
            const db = readInboxStore();
            if (after === undefined && through === undefined) {
                return readInboxActionablePage(db, workspaceId, questionsOnly === true, 0, actionableThrough(db));
            }
            if (typeof after !== 'number' || !Number.isSafeInteger(after) || after < 0
                || typeof through !== 'number' || !Number.isSafeInteger(through) || through <= after
                || through > actionableThrough(db)) {
                throw new Error('Invalid Inbox pagination cursor');
            }
            return readInboxActionablePage(db, workspaceId, questionsOnly === true, after, through);
        },
        inboxQuestions(workspaceIds) {
            if (closing) throw new Error('Hive host is closed');
            const store = runtime.store as typeof runtime.store & ActionableMessageSource;
            const workspaces = workspaceScope();
            return [...new Set(workspaceIds)].filter(workspaceId => workspaces.has(workspaceId)).flatMap(workspaceId =>
                store.listActionableDispatchMessages(workspaceId)
                    .filter(message => message.kind === 'question' && message.deliveryState !== 'queued')
                    .map(message => ({ workspaceId, dispatchId: message.dispatchId, messageId: message.id })));
        },
        inboxQuestionPages(workspaceIds) {
            if (closing) throw new Error('Hive host is closed');
            const workspaces = workspaceScope();
            const scope = [...new Set(workspaceIds)].filter(workspaceId => workspaces.has(workspaceId));
            if (!scope.length) return [];
            const db = readInboxStore();
            const through = actionableThrough(db);
            return scope.map(workspaceId => readInboxActionablePage(db, workspaceId, true, 0, through));
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
			// Attach a complete first desktop page. Keep the authenticated
			// HTTP action center and its support preview unchanged.
			const inbox = method === "GET" && /^\/api\/ui\/workspaces\/([^/]+)\/action-center$/.exec(url.pathname);
			if (inbox) {
				const workspace = decodeURIComponent(inbox[1]!);
				const store = runtime.store as typeof runtime.store & OpenDispatchSource;
                const db = readInboxStore();
                const page = readInboxActionablePage(db, workspace, false, 0, actionableThrough(db));
				return withInboxActionablePage(withInboxWaitingReports(result, workspace, store), page);
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
			closing ??= runtime.close().finally(() => {
                try {
                    inboxStore?.close();
                } finally {
                    release();
                }
            });
			return closing;
		},
	};
}

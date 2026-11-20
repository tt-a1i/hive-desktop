// Hive 2.2.1 publishes ESM JavaScript without declarations. Keep this narrow
// declaration tied to the pinned CLI export; desktop does not own Hive's store.
declare module "@tt-a1i/hive/dist/src/cli/hive-remote.js" {
    export function runHiveRemoteCommand(argv: string[]): Promise<number>;
}
declare module "@tt-a1i/hive/dist/src/cli/hive.js" {
	export function runHiveCommand(
		argv: string[],
		options?: {
			openBrowser?: boolean;
			versionService?: import("@tt-a1i/hive/dist/src/server/version-service.js").VersionService;
			agentRuntimeFactory?: import("./hive-runtime-contract.js").HiveAgentRuntimeFactory;
		},
	): Promise<{ port: number; close(): Promise<void>; store: {
        configureAgentLaunch: import("./hive-runtime-contract.js").HiveAgentRuntime["configureAgentLaunch"];
        startSavedScript(workspaceId: string, context: import('./hive-host.js').SavedScriptContext, observer: import('./hive-host.js').SavedScriptObserver): Promise<{runId: string}>;
        stopSavedScript(workspaceId: string, scriptRunId: string, nativeRunId: string): void;
        startSessionShell(workspaceId: string, context: {sessionId: string; cwd: string}): Promise<{runId: string}>;
        addWorkerWithLaunch(workspaceId: string, input: import('./hive-host.js').DesktopMemberInput & { avatar: null }, launch: import('./hive-runtime-contract.js').AgentLaunchConfigInput): import('./hive-runtime-contract.js').AgentSummary;
        settings: {
          readProjectScriptState(key: string, workspaceId: string): string | null;
          compareAndSwapProjectScriptState(key: string, expected: string | null, next: string | null, workspaceId: string): boolean;
          getAppState(key: string): { key: string; value: string | null } | undefined;
          setAppState(key: string, value: string): void };
        saveWorkspaceUpload(input: import("@tt-a1i/hive/dist/src/server/workspace-upload-store.js").SaveWorkspaceUploadInput):
          Promise<import("@tt-a1i/hive/dist/src/server/workspace-upload-store.js").WorkspaceUploadRecord>;
        locateWorkspaceUpload(workspaceId:string,id:string):Promise<import("./hive-host.js").HiveUploadLocation|undefined>;
        readWorkspaceUpload(workspaceId:string,id:string):Promise<import("./hive-host.js").HiveUpload|null>;
        findReportPresentation(workspaceId: string, agentId: string, dispatchId: string, payload: string): unknown;
        getDispatch(workspaceId: string, dispatchId: string): { toAgentId: string; status: string } | undefined;
        getDesktopUserInput(workspaceId: string, agentId: string, sequence: number): import('./hive-host.js').HiveUserInputRow | undefined;
        mutateDesktopUserInput(workspaceId: string, agentId: string, sequence: number, expectedText: string, replacement: string | null, persistSpool: () => void): void;
        withWorkspaceWorkflowIdle<T>(workspaceId: string, operation: () => Promise<T>): Promise<T>;
    } }>;
}

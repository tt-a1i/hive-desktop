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
			agentRuntimeFactory?: import("./hive-runtime-contract.js").HiveAgentRuntimeFactory;
		},
	): Promise<{ port: number; close(): Promise<void>; store: {
        settings: { getAppState(key: string): { value: string | null } | undefined;
          setAppState(key: string, value: string): void };
        locateWorkspaceUpload(workspaceId:string,id:string):Promise<import("./hive-host.js").HiveUploadLocation|undefined>;
        readWorkspaceUpload(workspaceId:string,id:string):Promise<import("./hive-host.js").HiveUpload|null>;
        findReportPresentation(workspaceId: string, agentId: string, dispatchId: string, payload: string): unknown;
        getDispatch(workspaceId: string, dispatchId: string): { toAgentId: string; status: string } | undefined;
        withWorkspaceWorkflowIdle<T>(workspaceId: string, operation: () => Promise<T>): Promise<T>;
    } }>;
}

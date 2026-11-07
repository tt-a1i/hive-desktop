// Hive 2.2.1 publishes ESM JavaScript without declarations. Keep this narrow
// declaration tied to the pinned CLI export; desktop does not own Hive's store.
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
        findReportPresentation(workspaceId: string, agentId: string, dispatchId: string, payload: string): unknown;
        withWorkspaceWorkflowIdle<T>(workspaceId: string, operation: () => Promise<T>): Promise<T>;
    } }>;
}

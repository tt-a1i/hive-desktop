// Hive 2.2.1 publishes ESM JavaScript without declarations. Keep this narrow
// declaration tied to the pinned CLI export; desktop does not own Hive's store.
declare module "@tt-a1i/hive/dist/src/cli/hive.js" {
	export function runHiveCommand(
		argv: string[],
		options?: { openBrowser?: boolean },
	): Promise<{ port: number; close(): Promise<void> }>;
}

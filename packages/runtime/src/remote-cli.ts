// Run Hive's existing remote CLI against this desktop's independent database.
// Resolve the data directory before importing Hive (which reads process.env).
import { isAbsolute, join } from 'node:path';

const [dataDir, ...argv] = process.argv.slice(2);
if (!dataDir || !isAbsolute(dataDir)) {
  throw new Error('Expected an absolute Hive Desktop data directory');
}
process.env.HIVE_DATA_DIR = join(dataDir, 'hive');
const { runHiveRemoteCommand } = await import('@tt-a1i/hive/dist/src/cli/hive-remote.js');
process.exitCode = await runHiveRemoteCommand(argv);

// Invoked only by the setup command saved for a private fixture repository.
const { existsSync, mkdirSync, readFileSync, realpathSync, writeFileSync } = require('node:fs');
const { join } = require('node:path');

async function main() {
  const directory = process.argv[2];
  const cwd = realpathSync(process.cwd());
  if (readFileSync(join(cwd, 'fixture.txt'), 'utf8') !== 'hive setup fixture\n') throw new Error('Unexpected working copy');
  // A third invocation fails immediately. mkdir is the claim for an attempt.
  const attempt = existsSync(join(directory, '1')) ? 2 : 1;
  const record = join(directory, String(attempt));
  mkdirSync(record);
  writeFileSync(join(record, 'process.json'), JSON.stringify({ pid: process.pid, parentPid: process.ppid, cwd }));
  console.log('FIXTURE_SETUP_ENTERED_' + attempt);
  const gate = join(record, 'release');
  const deadline = Date.now() + 90000;
  while (!existsSync(gate)) {
    if (Date.now() >= deadline) throw new Error('Fixture gate timed out');
    await new Promise(resolve => setTimeout(resolve, 100));
  }
  if (attempt === 1) {
    console.log('FIXTURE_SETUP_FAILED_23');
    process.exitCode = 23;
  } else {
    writeFileSync(join(cwd, '.hive-setup-fixture-result'), 'completed once\n', { flag: 'wx' });
    console.log('FIXTURE_SETUP_SUCCEEDED');
  }
}

main().catch(error => {
  console.error(error);
  process.exitCode = 91;
});

/** Real installed Pi process death and lazy binding failures; no inference. */
import assert from 'node:assert/strict';
import { mkdtemp, readFile, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createExternalPiSession } from '../src/agents/external-pi.js';
const directory = await mkdtemp(join(tmpdir(), 'hive-external-lifecycle-'));
const pidFile = join(directory, 'pi.pid');
const wrapper = join(directory, 'pi-wrapper.sh');
const quote = (value: string) => `'${value.replaceAll("'", "'\\''")}'`;
await writeFile(wrapper, `#!/bin/sh\nprintf '%s\\n' "$$" > ${quote(pidFile)}\nexec ${quote(process.env.HIVE_PI_COMMAND ?? '/Users/tsk/Library/pnpm/pi')} "$@"\n`, { mode: 0o700 });
let deaths = 0;
const bindingError = new Error('Binding unavailable before activation');
let notify!: (error: Error) => void;
const died = new Promise<Error>(resolve => { notify = resolve; });
const session = await createExternalPiSession({ cwd: directory, sessionDir: directory, command: wrapper,
  teamBinding() { throw bindingError; }, onEvent() {},
  onUnexpectedExit(error) { deaths++; notify(error); } });
try {
  await assert.rejects(session.prompt('Never reaches the model'), error => error === bindingError);
  assert.equal(session.history().some(message => (message as { role?: string }).role === 'assistant'), false);
  const pid = Number((await readFile(pidFile, 'utf8')).trim());
  assert.ok(Number.isSafeInteger(pid) && pid > 0);
  process.kill(pid, 'SIGKILL');
  let timer: ReturnType<typeof setTimeout> | undefined;
  const error = await Promise.race([died, new Promise<never>((_, reject) => { timer = setTimeout(() => reject(new Error('Missing external Pi death callback')), 5000); })]).finally(() => clearTimeout(timer));
  assert.ok(error instanceof Error);
  assert.equal(deaths, 1);
  await assert.rejects(session.prompt('Never reaches the model'), candidate => candidate === error);
} finally { await session.dispose(); }
assert.equal(deaths, 1, 'Intentional cleanup must not emit another unexpected exit');
console.log(JSON.stringify({ pass: true, directory, inference: false, unexpectedExitCallbacks: deaths }));

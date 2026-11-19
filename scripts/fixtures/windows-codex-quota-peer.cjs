// Synthetic app-server; only accepts the three read-only protocol calls.
const fs = require('node:fs');
const path = require('node:path');
const { spawn } = require('node:child_process');
const readline = require('node:readline');
const root = process.env.HIVE_QUOTA_FIXTURE;
if (!root || process.env.CODEX_HOME !== path.join(root, 'codex-home')) process.exit(71);
if (JSON.stringify(process.argv.slice(2)) !== JSON.stringify(['app-server', '--listen', 'stdio://'])) process.exit(73);
const helper = spawn(process.execPath, ['-e', 'setInterval(()=>{},1000)'], {
  detached: true,
  windowsHide: true,
  stdio: ['ignore', 'inherit', 'inherit'],
});
helper.unref();
fs.writeFileSync(
  path.join(root, 'peer-pids.json'),
  JSON.stringify({ peer: process.pid, helper: helper.pid }),
);
const log = path.join(root, 'requests.jsonl');
const allowed = new Set(['initialize', 'initialized', 'account/read', 'account/rateLimits/read']);
const input = readline.createInterface({ input: process.stdin });
input.on('line', (line) => {
  const message = JSON.parse(line);
  fs.appendFileSync(log, JSON.stringify({ method: message.method, params: message.params }) + '\n');
  if (!allowed.has(message.method)) process.exit(72);
  if (message.method === 'initialize') {
    process.stdout.write(
      JSON.stringify({ id: message.id, result: { userAgent: 'controlled fixture' } }) + '\n',
    );
  } else if (message.method === 'account/read') {
    process.stdout.write(
      JSON.stringify({
        id: message.id,
        result: {
          requiresOpenaiAuth: true,
          account: { type: 'chatgpt', email: 'PRIVATE_FIXTURE_EMAIL' },
        },
      }) + '\n',
    );
  } else if (message.method === 'account/rateLimits/read') {
    fs.writeFileSync(path.join(root, 'quota-ready'), 'ready');
    const poll = setInterval(() => {
      if (!fs.existsSync(path.join(root, 'release'))) return;
      clearInterval(poll);
      process.stdout.write(
        JSON.stringify({
          id: message.id,
          result: {
            ordinaryUsageAllowed: true,
            rateLimits: {
              limitId: 'codex',
              primary: { usedPercent: 22, windowDurationMins: 300, resetsAt: 1800000000 },
            },
            accountId: 'PRIVATE_FIXTURE_ACCOUNT',
          },
        }) + '\n',
      );
    }, 20);
  }
});
input.on('close', () => process.exit(0));

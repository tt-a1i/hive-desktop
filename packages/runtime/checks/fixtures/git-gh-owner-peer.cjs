const fs = require('node:fs');
const { spawn } = require('node:child_process');

const args = process.argv.slice(2);
if (args[0] === '--child') {
  setTimeout(() => fs.writeFileSync(args[1], 'fake PR write'), 1200);
  setInterval(() => {}, 1000);
} else if (args[0] === 'normal') {
  process.stdin.resume();
  process.stdin.on('end', () => {
    console.log(JSON.stringify({ args, ghRepo: process.env.GH_REPO ?? null }));
    console.error('fixture diagnostic');
  });
} else {
  const [record, marker] = args.slice(-2);
  const helper = spawn(process.execPath, [__filename, '--child', marker], {
    stdio: ['ignore', 'inherit', 'inherit'],
  });
  fs.writeFileSync(record, JSON.stringify({ gh: process.pid, helper: helper.pid }));
  setInterval(() => {}, 1000);
}

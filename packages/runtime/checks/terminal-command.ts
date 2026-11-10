/** Execute generated commands against an owned argument recorder, never a login. */
import assert from 'node:assert/strict';
import {execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {mkdtemp, writeFile, access} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {terminalCommand} from '../src/terminal-command.js';
const root=await mkdtemp(join(tmpdir(),'hive-terminal-command-'));
const script=join(root,"CLI 中文 ' $(touch INJECTION).cjs");
await writeFile(script,"#!/usr/bin/env node\nconsole.log(JSON.stringify(process.argv.slice(2)));",{mode:0o700});
const values=['login',"a ' quote",'$(touch INJECTION)','`touch INJECTION`','semi;colon','line\nbreak','路径 with spaces'];
const command=terminalCommand([process.execPath,script,...values]);
const run=promisify(execFile);
const result=process.platform==='win32'
 ? await run('powershell.exe',['-NoProfile','-NonInteractive','-Command',command],{cwd:root})
 : await run('/bin/sh',['-c',command],{cwd:root});
assert.deepEqual(JSON.parse(result.stdout),values);
await assert.rejects(access(join(root,'INJECTION')),(error:NodeJS.ErrnoException)=>error.code==='ENOENT');
assert.equal(terminalCommand(["C:\\Program Files\\Bob's CLI\\codex.cmd",'login'],'win32'),"& 'C:\\Program Files\\Bob''s CLI\\codex.cmd' 'login'");
console.log('TERMINAL_COMMAND_OK: exact executable/Unicode/quotes/metacharacter arguments round-trip, no injection file. Native platform execution; Windows string alone is not Windows execution evidence.');

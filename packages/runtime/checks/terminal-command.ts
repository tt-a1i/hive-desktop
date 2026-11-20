/** Execute generated commands against an owned argument recorder, never a login. */
import assert from 'node:assert/strict';
import {execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {mkdtemp, writeFile, access} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {terminalCommand} from '../src/terminal-command.js';
import {codexLoginCommand} from '../src/codex-account.js';
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
const recorder=join(root,"Recorder's argv and env.cjs");
await writeFile(recorder,"#!/usr/bin/env node\nconsole.log(JSON.stringify({args:process.argv.slice(2),home:process.env.CODEX_HOME}));",{mode:0o700});
let executable = recorder;
if (process.platform==='win32') {
  executable = join(root,"Recorder's CLI.cmd");
  await writeFile(executable,`@echo off\r\n"${process.execPath}" "${recorder}" %*\r\n`);
}
const codexHome = join(root, "Codex home's $(touch INJECTION)");
const login = codexLoginCommand(executable, codexHome);
assert.ok(login);
const options={cwd:root,env:{...process.env,CODEX_HOME:'parent-home'}};
const scoped = process.platform==='win32'
  ? await run('powershell.exe',['-NoProfile','-NonInteractive','-Command',`${login}; [Console]::WriteLine($env:CODEX_HOME)`],options)
  : await run('/bin/sh',['-c',`${login}; printf '%s\\n' "$CODEX_HOME"`],options);
const [recorded, parentHome] = scoped.stdout.trim().split(/\r?\n/);
assert.deepEqual(JSON.parse(recorded!), {args:['login'],home:codexHome});
assert.equal(parentHome, 'parent-home');
assert.equal(codexLoginCommand(executable,'relative-codex-home'), undefined);
assert.equal(codexLoginCommand(executable,join(root,'control\ncharacter')), undefined);
assert.equal(codexLoginCommand('codex',undefined),terminalCommand(['codex','login']));
assert.equal(codexLoginCommand("C:\\Bob's CLI\\codex.exe","D:\\Bob's Codex home",'win32'),
  "& { $previous = $env:CODEX_HOME; try { $env:CODEX_HOME = 'D:\\Bob''s Codex home'; & 'C:\\Bob''s CLI\\codex.exe' 'login' } finally { $env:CODEX_HOME = $previous } }");
assert.equal(codexLoginCommand("C:codex.exe","D:\\Codex home",'win32'), undefined);
await assert.rejects(access(join(root,'INJECTION')),(error:NodeJS.ErrnoException)=>error.code==='ENOENT');
console.log('TERMINAL_COMMAND_OK: exact executable/Unicode/quotes/metacharacter arguments round-trip, no injection file. Native platform execution; Windows string alone is not Windows execution evidence.');

/** Real Hive persistence with a local Codex protocol peer; no vendor inference. */
import assert from 'node:assert/strict';
import {mkdtemp, mkdir, readFile, writeFile, rename, rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {ChatIndex} from '../src/chat-index.js';
const directory = await mkdtemp(join(tmpdir(), 'hive-codex-mode-persistence-'));
const prior = process.env.HIVE_CODEX_COMMAND;
process.env.HIVE_CODEX_COMMAND = fileURLToPath(new URL('./codex-modes-peer.cjs', import.meta.url));
const {createDesktopRuntime} = await import('../src/desktop-runtime.js');
let runtime: Awaited<ReturnType<typeof createDesktopRuntime>> | undefined;
try {
 const project = join(directory, 'project'), data = join(directory, 'data'); await mkdir(project);
 runtime = await createDesktopRuntime(data, () => {});
 const workspace = await runtime.hive.request('POST', '/api/workspaces', {path: project, name: 'Mode persistence', autostart_orchestrator: false}) as {id:string};
 const chat = await runtime.chats.createRecord(workspace.id, 'codex');
 assert.equal((await runtime.chats.mode(chat.id)).selected, null);
 assert.equal((await runtime.chats.mode(chat.id, 'native-plan')).pendingNextTurn, true);
 await runtime.chats.mode(chat.id, 'workspace-write', true);
 const index = () => new ChatIndex(join(data, 'chats'));
 assert.deepEqual(index().get(chat.id).nativeMode, {backend:'codex',mode:'native-plan',permissionMode:'workspace-write'});
 await runtime.close(); runtime = await createDesktopRuntime(data, () => {});
 const collaboration = await runtime.chats.mode(chat.id), permission = await runtime.chats.mode(chat.id, undefined, true);
 assert.equal(collaboration.selected, 'native-plan'); assert.equal(collaboration.pendingNextTurn, true);
 assert.equal(permission.selected, 'workspace-write');
 const wire = async () => (await readFile(join(project, 'modes-wire.jsonl'), 'utf8')).trim().split('\n').map(row => JSON.parse(row));
 const starts = (await wire()).filter(row => row.method === 'thread/start');
 assert.equal(starts.at(-1).params.sandbox, 'workspace-write'); assert.equal(starts.at(-1).params.approvalPolicy, 'on-request');
 const file = join(data, 'chats/index.json');
 await rename(file,file+'.retained'); await mkdir(file);
 try { await assert.rejects(runtime.chats.mode(chat.id, 'native-default')); }
 finally { await rm(file,{recursive:true}); await rename(file+'.retained',file); }
 assert.equal((await runtime.chats.mode(chat.id)).selected,'native-plan');
 assert.equal((await runtime.chats.mode(chat.id, undefined, true)).selected,'workspace-write');
 await runtime.close(); runtime = undefined;
 await writeFile(join(project,'modes-control.json'),JSON.stringify({error:-32601}));
 runtime = await createDesktopRuntime(data, () => {});
 await assert.rejects(runtime.chats.mode(chat.id), /Unsupported Codex collaboration mode/);
 await assert.rejects(runtime.chats.prompt(chat.id,'Do not submit with unavailable saved mode'));
 assert.equal((await wire()).filter(row => row.method==='turn/start').length,0);
 console.log('CODEX_MODE_PERSISTENCE_OK: two axes retained across cold start, disk failure and unsupported saved mode fail closed; no inference.');
} finally { await runtime?.close(); if (prior === undefined) delete process.env.HIVE_CODEX_COMMAND; else process.env.HIVE_CODEX_COMMAND=prior; }

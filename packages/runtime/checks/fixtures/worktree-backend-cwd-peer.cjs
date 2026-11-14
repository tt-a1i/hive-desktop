#!/usr/bin/env node
// Controlled protocol peers only. No vendor CLI, credentials or model API.
const fs = require('node:fs'), path = require('node:path'), crypto = require('node:crypto'), readline = require('node:readline');
const args = process.argv.slice(2), cwd = fs.realpathSync(process.cwd());
const backend = args.includes('app-server') ? 'codex' : args.includes('--mode') ? 'pi-rpc' : 'claude';
const arg = name => args.find(value => value.startsWith(name + '='))?.slice(name.length + 1) ?? (args.includes(name) ? args[args.indexOf(name) + 1] : undefined);
const log = value => fs.appendFileSync(process.env.HIVE_CWD_FIXTURE_WIRE, JSON.stringify({backend, cwd, pid: process.pid, ...value}) + '\n');
const send = value => process.stdout.write(JSON.stringify(value) + '\n');
log({event: 'start', args});
let state, entries = [], servers = [], parentUuid = null;
if (backend === 'codex') {
  const file = path.join(cwd, '.fixture-codex-thread.json');
  try { state = JSON.parse(fs.readFileSync(file, 'utf8')); } catch (error) { if (error.code !== 'ENOENT') throw error; }
  state ??= {id: crypto.randomUUID(), path: file, cwd, historyMode: 'paginated', turns: []};
} else if (backend === 'pi-rpc') {
  const file = arg('--session') ?? path.join(arg('--session-dir'), crypto.randomUUID() + '.jsonl');
  if (fs.existsSync(file)) {
    const [header, ...rows] = fs.readFileSync(file, 'utf8').trim().split('\n').map(line => JSON.parse(line));
    if (header.cwd !== cwd) throw Error('Pi fixture cwd changed');
    state = {sessionId: header.id, sessionFile: file}; entries = rows;
  } else {
    state = {sessionId: crypto.randomUUID(), sessionFile: file}; fs.mkdirSync(path.dirname(file), {recursive: true});
    fs.writeFileSync(file, JSON.stringify({type: 'session', version: 3, id: state.sessionId, cwd, timestamp: new Date().toISOString()}) + '\n');
  }
  Object.assign(state, {isStreaming: false, thinkingLevel: 'medium', model: {id: 'fixture-cwd', provider: 'openai', name: 'Protocol fixture only', input: ['text']}});
} else {
  const sid = arg('--session-id') ?? arg('--resume');
  const file = path.join(process.env.CLAUDE_CONFIG_DIR, 'projects', cwd.replace(/[^a-zA-Z0-9]/g, '-'), sid + '.jsonl');
  state = {sid, file};
  if (fs.existsSync(file)) for (const raw of fs.readFileSync(file, 'utf8').trim().split('\n')) { const row = JSON.parse(raw); if (row.uuid) parentUuid = row.uuid; }
}
const model = {model: 'fixture-cwd', modelProvider: 'openai'};
const saveCodex = () => fs.writeFileSync(state.path, JSON.stringify(state));
const notify = (method, params) => send({method, params: {threadId: state.id, ...params}});
readline.createInterface({input: process.stdin}).on('line', raw => {
  try {
    const q = JSON.parse(raw);
    if (backend === 'codex') {
      if (!q.method) return;
      log({event: 'request', method: q.method, params: q.params});
      const answer = result => send({id: q.id, result});
      if (q.method === 'initialize') answer({});
      else if (q.method === 'model/list') answer({data: [{id: 'fixture-cwd', model: 'fixture-cwd', displayName: 'Protocol fixture only', hidden: false}], nextCursor: null});
      else if (q.method === 'thread/start') { if (q.params.cwd !== cwd) throw Error('Codex thread/start cwd mismatch'); saveCodex(); answer({thread: {...state, turns: []}, ...model}); }
      else if (q.method === 'thread/resume') { if (q.params.threadId !== state.id) throw Error('Codex thread identity changed'); answer({thread: {...state, turns: []}, ...model}); }
      else if (q.method === 'thread/read') answer({thread: {...state, turns: []}, ...model});
      else if (q.method === 'thread/turns/list') answer({data: state.turns.map(turn => ({...turn, itemsView: 'full'})), nextCursor: null});
      else if (q.method === 'turn/start') {
        const user = {id: crypto.randomUUID(), type: 'userMessage', content: q.params.input};
        const assistant = {id: crypto.randomUUID(), type: 'agentMessage', text: 'Controlled cwd fixture completion'};
        const turn = {id: crypto.randomUUID(), status: 'inProgress', items: [user]};
        state.turns.push(turn); answer({turn}); notify('turn/started', {turn}); notify('item/completed', {turnId: turn.id, item: user});
        turn.items.push(assistant); turn.status = 'completed'; saveCodex();
        notify('item/completed', {turnId: turn.id, item: assistant}); notify('turn/completed', {turn});
      } else if (q.method === 'turn/interrupt') answer({});
      else if (q.id !== undefined) throw Error('Unexpected Codex request: ' + q.method);
    } else if (backend === 'pi-rpc') {
      log({event: 'request', type: q.type});
      const answer = data => send({id: q.id, type: 'response', command: q.type, success: true, data});
      if (q.type === 'get_state') answer(state);
      else if (q.type === 'get_commands') answer({commands: [{name: process.env.HIVE_PI_BRIDGE_COMMAND, source: 'extension', sourceInfo: {path: arg('--extension')}}]});
      else if (q.type === 'get_available_models') answer({models: [state.model]});
      else if (q.type === 'get_entries') {
        const index = q.since === undefined ? -1 : entries.findIndex(entry => entry.id === q.since);
        answer({entries: entries.slice(index + 1), leafId: entries.at(-1)?.id ?? null});
      } else if (q.type === 'prompt') {
        const payload = JSON.parse(q.message.slice(q.message.indexOf(' ') + 1));
        log({event: 'bridge', action: payload.action});
        if (payload.action === 'context') {
          const entry = {id: crypto.randomUUID(), parentId: entries.at(-1)?.id ?? null, type: 'custom_message', customType: payload.customType,
            content: payload.text, display: false, timestamp: new Date().toISOString()};
          entries.push(entry); fs.appendFileSync(state.sessionFile, JSON.stringify(entry) + '\n');
        } else if (payload.action !== 'initialize' && payload.action !== 'bind') throw Error('Unexpected Pi query action: ' + payload.action);
        answer({disposition: 'handled'});
      } else if (q.type === 'abort') answer({});
      else throw Error('Unexpected Pi request: ' + q.type);
    } else {
      log({event: 'incoming', type: q.type, subtype: q.request?.subtype, shouldQuery: q.shouldQuery});
      if (q.type === 'control_request') {
        const sub = q.request.subtype; let response = {};
        if (sub === 'initialize') { servers = q.request.sdkMcpServers ?? []; response = {commands: [], models: [{value: 'fixture-cwd', displayName: 'Protocol fixture only', description: 'No inference'}], account: {}}; }
        else if (sub === 'mcp_status') response = {mcpServers: servers.map(name => ({name, status: 'connected', serverInfo: {name, version: '1'}, config: {type: 'sdk', name}}))};
        else if (sub === 'get_settings') response = {applied: {model: 'fixture-cwd', effort: 'medium'}};
        send({type: 'control_response', response: {subtype: 'success', request_id: q.request_id, response}});
      } else if (q.type === 'user') {
        if (q.shouldQuery !== false) throw Error('Claude model query is forbidden in this fixture');
        fs.mkdirSync(path.dirname(state.file), {recursive: true});
        fs.appendFileSync(state.file, JSON.stringify({...q, parentUuid, isSidechain: false, timestamp: new Date().toISOString(), cwd, sessionId: state.sid}) + '\n'); parentUuid = q.uuid;
      }
    }
  } catch (error) { log({event: 'fixture-error', message: error.message}); console.error(error); process.exitCode = 1; process.stdin.destroy(); }
}).on('close', () => process.exit(process.exitCode ?? 0));

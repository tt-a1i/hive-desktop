// Controlled app-server for real Hive attachment checks. No model calls.
const {createInterface} = require('node:readline');
const {existsSync, appendFileSync} = require('node:fs');
const {basename} = require('node:path');

const thread = {id: basename(process.cwd()), path: null, historyMode: 'legacy', turns: []};
const model = {thread, model: 'gpt-6.1-terra', modelProvider: 'openai'};
const send = value => process.stdout.write(JSON.stringify(value) + '\n');
const notify = (method, params) => send({method, params: {threadId: thread.id, ...params}});
let turn;
let answered = false;
function end() {
  turn.status = 'interrupted';
  notify('turn/completed', {turn});
}
function resolve() {
  notify('serverRequest/resolved', {requestId: 7});
  end();
}
const timer = setInterval(() => {
  if (turn?.status !== 'inProgress') return;
  if (existsSync('exit')) process.exit(7);
  if (existsSync('turn-end')) end();
  else if (existsSync('native-resolve') || (answered && existsSync('finish'))) resolve();
}, 10);

async function run() {
  for await (const line of createInterface({input: process.stdin})) {
    const request = JSON.parse(line);
    if (!request.method) {
      appendFileSync('responses.jsonl', JSON.stringify(request) + '\n');
      answered = true;
      continue;
    }
    const reply = result => send({id: request.id, result});
    switch (request.method) {
      case 'initialize':
      case 'thread/settings/update':
        reply({});
        break;
      case 'model/list':
        reply({data: [{id: 'terra', model: 'gpt-6.1-terra', displayName: 'Terra', hidden: false}], nextCursor: null});
        break;
      case 'configRequirements/read':
        reply({requirements: null});
        break;
      case 'collaborationMode/list':
        reply({data: [{mode: 'default'}, {mode: 'plan'}]});
        break;
      case 'thread/start':
      case 'thread/resume':
        reply(model);
        break;
      case 'thread/read':
        reply({thread});
        break;
      case 'turn/interrupt':
        reply({});
        resolve();
        break;
      case 'turn/start': {
        turn = {id: 'turn-' + thread.turns.length, status: 'inProgress', items: []};
        thread.turns.push(turn);
        reply({turn});
        notify('turn/started', {turn});
        const identity = {threadId: thread.id, turnId: turn.id, itemId: 'item'};
        if (request.params.input[0].text === 'approval') {
          send({id: 7, method: 'item/commandExecution/requestApproval',
            params: {...identity, startedAtMs: 1, availableDecisions: ['decline']}});
          break;
        }
        send({id: 7, method: 'item/tool/requestUserInput', params: {...identity, isBlocking: true,
          questions: [
            {id: 'public', header: 'Public', question: 'Choose', isSecret: false, isOther: true, options: null},
            {id: 'secret', header: 'Secret', question: 'Secret', isSecret: true, isOther: true, options: null},
            {id: 'extra', header: 'Extra', question: 'Extra', isSecret: false, isOther: true, options: null},
          ],
        }});
        break;
      }
    }
  }
}
run().catch(error => {
  console.error(error);
  process.exitCode = 1;
}).finally(() => clearInterval(timer));

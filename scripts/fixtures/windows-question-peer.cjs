// Controlled local Codex app-server. No model, account or network access.
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const readline = require('node:readline');
const root = process.env.HIVE_QUESTION_FIXTURE;
if (!root || process.env.CODEX_HOME !== path.join(root, 'codex-home')) process.exit(71);
if (JSON.stringify(process.argv.slice(2)) !== JSON.stringify(['app-server', '--listen', 'stdio://'])) process.exit(72);
const threadFile = path.join(root, 'question-thread.json');
const responses = path.join(root, 'question-responses.jsonl');
const thread = fs.existsSync(threadFile) ? JSON.parse(fs.readFileSync(threadFile, 'utf8')) :
  { id: 'controlled-question-thread', path: null, historyMode: 'legacy', turns: [] };
const save = () => fs.writeFileSync(threadFile, JSON.stringify(thread));
const send = value => process.stdout.write(JSON.stringify(value) + '\n');
const notify = (method, params) => send({ method, params: { threadId: thread.id, ...params } });
const answer = (id, result) => send({ id, result });
const requests = path.join(root, 'question-peer-requests.jsonl');
const input = readline.createInterface({ input: process.stdin });
input.on('line', line => {
  try {
    const q = JSON.parse(line);
    fs.appendFileSync(requests, JSON.stringify({ method: q.method ?? 'response', id: q.id }) + '\n');
    if (!q.method) {
      if (q.id !== 7) throw Error('Unexpected response identity');
      const text = q.result.answers.public.answers[0];
      const refs = text.split('\n').filter(line => line.startsWith('{')).map(line => JSON.parse(line));
      if (refs.length !== 1) throw Error('Expected one attached file');
      const bytes = fs.readFileSync(refs[0].path);
      fs.appendFileSync(responses, JSON.stringify({ id: q.id, result: q.result, references: refs,
        bytesBase64: bytes.toString('base64'), sha256: crypto.createHash('sha256').update(bytes).digest('hex') }) + '\n');
      const turn = thread.turns[thread.turns.length - 1];
      turn.status = 'completed'; save();
      notify('serverRequest/resolved', { requestId: 7 });
      notify('turn/completed', { turn });
      return;
    }
    if (q.method === 'initialize') answer(q.id, {});
    else if (q.method === 'initialized') return;
    else if (q.method === 'model/list') answer(q.id, { data: [{ id: 'terra', model: 'gpt-6.1-terra', displayName: 'Terra', hidden: false }], nextCursor: null });
    else if (q.method === 'configRequirements/read') answer(q.id, { requirements: null });
    else if (q.method === 'collaborationMode/list') answer(q.id, { data: [{ mode: 'default' }, { mode: 'plan' }] });
    else if (q.method === 'thread/start' || q.method === 'thread/resume') answer(q.id, { thread, model: 'gpt-6.1-terra', modelProvider: 'openai' });
    else if (q.method === 'thread/read') answer(q.id, { thread });
    else if (q.method === 'thread/settings/update') answer(q.id, {});
    else if (q.method === 'turn/start') {
      if (thread.turns.length) throw Error('Only one turn is permitted');
      const turn = { id: 'turn-0', status: 'inProgress', items: [] }; thread.turns.push(turn); save();
      answer(q.id, { turn }); notify('turn/started', { turn });
      send({ id: 7, method: 'item/tool/requestUserInput', params: { threadId: thread.id, turnId: turn.id, itemId: 'question-item',
        isBlocking: true, questions: [{ id: 'public', header: 'File proof', question: 'Attach the fixture file',
          isSecret: false, isOther: false, options: [{ label: 'Verify fixture', description: 'Use the saved file' }] }] } });
    } else throw Error(`Unexpected method: ${q.method}`);
  } catch (error) { console.error(error); process.exit(73); }
});
input.on('close', () => process.exit(0));

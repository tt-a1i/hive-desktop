const { appendFileSync, existsSync, readFileSync, writeFileSync } = require('node:fs');
const { randomUUID } = require('node:crypto');
const { join } = require('node:path');
const { createInterface } = require('node:readline');
const { setTimeout: delay } = require('node:timers/promises');

const directory = process.env.HIVE_VIEW_FIXTURE_DIRECTORY;
const version = readFileSync(join(directory, 'version'), 'utf8');
const log = fields => appendFileSync(join(directory, 'wire.jsonl'), JSON.stringify({ pid: process.pid, ...fields }) + '\n');
const send = frame => process.stdout.write(JSON.stringify(frame) + '\n');
const gate = async name => {
  while (existsSync(join(directory, name))) await delay(10);
};
let thread;
let model = 'gpt-6.1-terra';
let effort = 'low';
const image = 'aGl2ZSB2aWV3IGZpeHR1cmU=';
const snapshotPath = id => join(directory, `${encodeURIComponent(id)}.json`);
const setup = (id, saved) => {
  const file = snapshotPath(id);
  if (existsSync(file)) return JSON.parse(readFileSync(file, 'utf8'));
  const turns = saved ? [{ id: `${id}-saved`, status: 'completed', items: [
    { id: `${id}-image`, type: 'userMessage', content: [{ type: 'image', url: `data:image/png;base64,${image}` }] },
    { id: `${id}-marker`, type: 'agentMessage', text: `Saved reply for ${id}` },
  ] }] : [];
  return { id, path: null, historyMode: 'paginated', turns };
};
const notify = (method, params) => send({ method, params: { threadId: thread.id, ...params } });
const save = () => writeFileSync(snapshotPath(thread.id), JSON.stringify(thread));
const modelInfo = id => ({ id, model: id, displayName: 'Controlled metadata', hidden: false,
  supportedReasoningEfforts: [{ reasoningEffort: 'low' }, { reasoningEffort: 'medium' }] });

log({ method: 'process/start', version });
createInterface({ input: process.stdin }).on('line', async line => {
  const request = JSON.parse(line);
  if (request.id === undefined) return;
  const { id, method, params } = request;
  log({ method, threadId: params?.threadId ?? thread?.id, cwd: params?.cwd,
    ...(method === 'thread/resume' && { deferGoalContinuation: params.deferGoalContinuation }),
    ...(method === 'turn/start' && { input: params.input, additionalContext: params.additionalContext }) });
  const reply = result => send({ id, result });
  try {
    switch (method) {
      case 'initialize': return reply({ userAgent: `hive_desktop/${version} (controlled test)` });
      case 'model/list': return reply({ data: [modelInfo('gpt-6.1-terra'), modelInfo('fixture-alternate')], nextCursor: null });
      case 'thread/start':
      case 'thread/resume': {
        thread = setup(params.threadId ?? randomUUID(), method === 'thread/resume');
        model = params.model ?? model;
        await gate(`${thread.id}.hold-resume`);
        return reply({ thread, model, modelProvider: 'openai' });
      }
      case 'thread/read':
        await gate(`${thread.id}.hold-read`);
        return reply({ thread: { ...thread, model, reasoningEffort: effort }, model, modelProvider: 'openai' });
      case 'thread/turns/list': {
        const offset = Number(params.cursor ?? 0);
        const turn = thread.turns[offset];
        return reply({ data: turn ? [{ ...turn, itemsView: 'full' }] : [],
          nextCursor: offset + 1 < thread.turns.length ? String(offset + 1) : null });
      }
      case 'collaborationMode/list': return reply({ data: [{ mode: 'plan' }, { mode: 'default' }] });
      case 'configRequirements/read': return reply({ requirements: null });
      case 'thread/settings/update':
        await gate(`${thread.id}.hold-settings`);
        model = params.model ?? model;
        effort = params.effort ?? effort;
        reply({});
        return notify('thread/settings/updated', { threadSettings: { model, effort } });
      case 'turn/start': {
        const user = { id: randomUUID(), type: 'userMessage', content: params.input };
        const turn = { id: randomUUID(), status: 'inProgress', items: [user] };
        thread.turns.push(turn);
        save();
        reply({ turn });
        notify('turn/started', { turn });
        notify('item/completed', { turnId: turn.id, item: user });
        await gate(`${thread.id}.hold-turn`);
        if (turn.status !== 'inProgress') return;
        const assistant = { id: randomUUID(), type: 'agentMessage', text: 'Controlled turn finished' };
        turn.items.push(assistant);
        turn.status = 'completed';
        save();
        notify('item/completed', { turnId: turn.id, item: assistant });
        return notify('turn/completed', { turn });
      }
      case 'turn/interrupt': {
        const turn = thread.turns.find(candidate => candidate.id === params.turnId);
        if (!turn) throw new Error('Missing controlled turn to interrupt');
        turn.status = 'interrupted';
        save();
        reply({});
        return notify('turn/completed', { turn });
      }
      default: throw new Error(`Unexpected fixture request: ${method}`);
    }
  } catch (error) {
    send({ id, error: { code: -32601, message: error.message } });
  }
});

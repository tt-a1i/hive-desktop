// Controlled local Pi RPC for native import tests. Reads and appends to the
// exact session passed to --session and never contacts a provider.
const { appendFileSync, readFileSync } = require('node:fs');
const { randomUUID } = require('node:crypto');
const { createInterface } = require('node:readline');

const argument = name => process.argv[process.argv.indexOf(name) + 1];
const sessionFile = argument('--session');
const extensionPath = argument('--extension');
const command = process.env.HIVE_PI_BRIDGE_COMMAND;
const [header, ...entries] = readFileSync(sessionFile, 'utf8').trim().split('\n').map(line => JSON.parse(line));
const model = {provider:'fixture', id:'fixture-local', name:'Controlled Pi'};
const leaf = () => entries.at(-1)?.id ?? null;
const send = message => process.stdout.write(JSON.stringify(message) + '\n');
const log = message => appendFileSync(process.env.HIVE_PI_IMPORT_WIRE, JSON.stringify(message) + '\n');
const persist = fields => {
  const entry = {...fields, id:randomUUID(), parentId:leaf(), timestamp:new Date().toISOString()};
  entries.push(entry);
  appendFileSync(sessionFile, JSON.stringify(entry) + '\n');
};
const custom = (customType, data) => persist({type:'custom', customType, data});
log({event:'start', sessionId:header.id, sessionFile, cwd:process.cwd()});
createInterface({input:process.stdin}).on('line', line => {
  const request = JSON.parse(line);
  const reply = data => send({type:'response', id:request.id, command:request.type, success:true, data});
  switch (request.type) {
    case 'get_state': return reply({sessionId:header.id, sessionFile, model, isStreaming:false});
    case 'get_commands': return reply({commands:[{name:command, source:'extension', sourceInfo:{path:extensionPath}}]});
    case 'get_available_models': return reply({models:[model]});
    case 'get_entries': return reply({entries:entries.slice(request.since ? entries.findIndex(item => item.id === request.since) + 1 : 0), leafId:leaf()});
    case 'prompt': {
      const prefix = `/${command} `;
      if (request.message.startsWith(prefix)) {
        const bridge = JSON.parse(request.message.slice(prefix.length));
        if (bridge.action === 'context') persist({type:'custom_message', customType:bridge.customType, content:bridge.text, display:false});
        if (bridge.action === 'begin-user-input') custom('hive-user-input', bridge.input);
        if (bridge.action === 'end-user-input') custom('hive-user-input-result', {deliveryId:bridge.deliveryId, disposition:bridge.disposition});
        if (bridge.action === 'complete') custom('hive-delivery-completed', {id:bridge.deliveryId});
        return reply({disposition:'handled'});
      }
      log({event:'query', text:request.message});
      reply({disposition:'started'});
      send({type:'agent_start'});
      const user = {role:'user', content:request.message, timestamp:Date.now()};
      const assistant = {role:'assistant', content:[{type:'text', text:'Controlled Pi continued'}], stopReason:'stop', timestamp:Date.now()};
      for (const message of [user, assistant]) {
        persist({type:'message', message});
        send({type:'message_end', message});
      }
      return send({type:'agent_settled', aborted:false});
    }
    default: return send({type:'response', id:request.id, command:request.type, success:false, error:`Unexpected request ${request.type}`});
  }
});

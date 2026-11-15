// Synthetic ACP agent: forwards explicit test prompts to the supplied MCP server.
const { spawn } = require('node:child_process');
const readline = require('node:readline');
let bridge, lines, descriptor, next = 0;
const send = value => process.stdout.write(JSON.stringify({ jsonrpc: '2.0', ...value }) + '\n');
async function rpc(method, params) {
  const id = ++next;
  bridge.stdin.write(JSON.stringify({ jsonrpc: '2.0', id, method, params }) + '\n');
  const response = JSON.parse((await lines.next()).value);
  if (response.error) throw Error(JSON.stringify(response.error));
  return response.result;
}
readline.createInterface({ input: process.stdin }).on('line', async line => {
  const request = JSON.parse(line);
  try {
    if (request.method === 'initialize') {
      if ('HIVE_ACP_TEAM_BINDING' in process.env || 'HIVE_ACP_TEAM_PROOF' in process.env) throw Error('ACP parent must not inherit private bridge environment');
      return send({ id: request.id, result: { protocolVersion: 1, agentCapabilities: {}, agentInfo: { name: 'fixture', version: '1' } } });
    }
    if (request.method === 'session/new') {
      const server = request.params.mcpServers[0];
      descriptor=server;
      const token=JSON.parse(server.env.find(entry=>entry.name==='HIVE_ACP_TEAM_BINDING').value).token;
      if(process.argv.some(arg=>arg.includes(token)))throw Error('token was placed in ACP argv');
      // Mirrors Codex's MCP inherited allowlist, then applies the explicit ACP descriptor env.
      const env = Object.fromEntries(['HOME','LOGNAME','PATH','SHELL','USER','__CF_USER_TEXT_ENCODING','LANG','LC_ALL','TERM','TMPDIR','TZ'].flatMap(name => process.env[name] === undefined ? [] : [[name,process.env[name]]]));
      for (const {name,value} of server.env) env[name]=value;
      bridge = spawn(server.command, server.args, { env });
      lines = readline.createInterface({ input: bridge.stdout })[Symbol.asyncIterator]();
      await rpc('initialize', { protocolVersion: '2025-11-25', capabilities: {}, clientInfo: { name: 'fixture', version: '1' } });
      bridge.stdin.write('{"jsonrpc":"2.0","method":"notifications/initialized"}\n');
      await rpc('tools/list', {});
      return send({ id: request.id, result: { sessionId: 'fixture' } });
    }
    if (request.method === 'session/prompt') {
      const args = JSON.parse(request.params.prompt[0].text);
      const result = args?.[0] === '__fixture_reflect_private_descriptor' ? (process.stderr.write(JSON.stringify(descriptor)), {content:[{text:JSON.stringify(descriptor)}]}) : args === null ? { content: [{ text: 'turn ended' }] } : await rpc('tools/call', { name: 'team', arguments: { args } });
      send({ method: 'session/update', params: { sessionId: 'fixture', update: { sessionUpdate: 'agent_message_chunk', content: { type: 'text', text: JSON.stringify(result) } } } });
      return send({ id: request.id, result: { stopReason: 'end_turn' } });
    }
  } catch (error) { send({ id: request.id, error: { code: -32603, message: String(error) } }); }
});
process.stdin.on('end', () => { if (bridge) bridge.stdin.end(); });

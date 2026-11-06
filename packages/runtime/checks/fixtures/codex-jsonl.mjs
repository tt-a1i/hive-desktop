#!/usr/bin/env node
// Synthetic protocol peer only. Ignores the transport's fixed app-server argv.
import { createInterface } from 'node:readline';
const lines = createInterface({ input: process.stdin });
const send = (value) => process.stdout.write(`${JSON.stringify(value)}\n`);
const overlap = [];
let held;
let serverRequests;
lines.on('line', (line) => {
  const message = JSON.parse(line);
  if (!message.method) {
    serverRequests.responses.push(message);
    if (serverRequests.responses.length === 2) {
      send({ id: serverRequests.id, result: serverRequests.responses });
      serverRequests = undefined;
    }
    return;
  }
  switch (message.method) {
    case 'initialize': send({ id: message.id, result: {} }); break;
    case 'initialized': send({ method: 'fixture/ready', params: { ready: true } }); break;
    case 'fixture/overlap':
      overlap.push(message);
      if (overlap.length === 2) {
        for (const request of overlap.reverse()) send({ id: request.id, result: request.params });
      }
      break;
    case 'fixture/serverRequests':
      serverRequests = { id: message.id, responses: [] };
      send({ id: 41, method: 'fixture/ask', params: { kind: 'numeric' } });
      send({ id: '41', method: 'fixture/ask', params: { kind: 'string' } });
      break;
    case 'fixture/hold': held = message.id; break;
    case 'fixture/release':
      send({ id: held, result: 'late response' });
      send({ id: message.id, result: 'released' });
      break;
    default: throw new Error(`Unexpected fixture method: ${message.method}`);
  }
});

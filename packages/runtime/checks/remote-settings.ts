/** Real Hive HTTP/SQLite and original remote CLI; owned loopback gateway only. */
import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { once } from 'node:events';
import { createServer } from 'node:http';
import { mkdtemp, mkdir, rm, access } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';
import WebSocket, { WebSocketServer } from 'ws';
import { createHiveHost, HiveHttpError } from '../src/hive-host.js';
// Test the exact commands returned by the compiled desktop sidecar.
const { remoteCommands } = await import('../dist/remote-commands.js');
const run = promisify(execFile);
const root = await mkdtemp(join(tmpdir(), 'hive-desktop-remote-'));
const state = join(root, "桌面 ' $(touch SHOULD_NOT_EXIST)");
const foreign = join(root, 'foreign-hive');
const token = crypto.randomUUID();
const requests: string[] = [];
const confirmations: Record<string, unknown>[] = [];
let gatewayRevokeAvailable = false;
const gateway = createServer(async (request, response) => {
  let text = '';
  for await (const chunk of request) text += chunk;
  const body = JSON.parse(text || '{}');
  requests.push(request.url!);
  response.setHeader('content-type', 'application/json');
  if (request.url === '/daemon/code') {
    response.end(JSON.stringify({ code: 'owned-code', expiresAt: Date.now() + 30_000, pollIntervalMs: 1 }));
  } else if (request.url === '/daemon/token' && body.code === 'owned-code') {
    response.end(JSON.stringify({ daemonId: 'owned-daemon', daemonToken: token }));
  } else if (request.url === '/pair/confirm' || request.url === '/pair/revoke') {
    assert.equal(request.headers.authorization, `Bearer ${token}`);
    confirmations.push(body);
    if (request.url === '/pair/revoke' && !gatewayRevokeAvailable) {
      response.writeHead(503).end('{}');
      return;
    }
    response.end('{}');
  } else {
    response.writeHead(404).end('{}');
  }
});
gateway.listen(0, '127.0.0.1');
await once(gateway, 'listening');
const address = gateway.address();
assert.ok(address && typeof address !== 'string');
const gatewayUrl = `http://127.0.0.1:${address.port}`;
const sockets = new WebSocketServer({ server: gateway });
let daemon: WebSocket | undefined;
const frames: Record<string, unknown>[] = [];
const binaryFrames: Uint8Array[] = [];
sockets.on('connection', socket => {
  daemon = socket;
  socket.on('message', (data, binary) => {
    if (binary) { binaryFrames.push(new Uint8Array(data as Buffer)); return; }
    if (data.toString() === 'hb:ping') socket.send('hb:pong');
    else frames.push(JSON.parse(data.toString()));
  });
});
async function waitFor(predicate: () => Promise<boolean> | boolean) {
  const deadline = Date.now() + 5_000;
  while (!await predicate()) {
    assert.ok(Date.now() < deadline, 'Timed out waiting for real remote state');
    await new Promise(resolve => setTimeout(resolve, 10));
  }
}
const helper = fileURLToPath(new URL('../dist/remote-cli.js', import.meta.url));
const env = { ...process.env, HIVE_DATA_DIR: foreign };
let host: Awaited<ReturnType<typeof createHiveHost>> | undefined;
try {
  await mkdir(state);
  host = await createHiveHost({ dataDir: join(state, 'hive') });
  const status = () => host!.request('GET', '/api/remote/status') as Promise<Record<string, unknown>>;
  assert.equal((await status()).enabled, false);
  assert.equal((await status()).logged_in, false);
  await assert.rejects(host.request('POST', '/api/remote/pairings', {}), (error: unknown) => error instanceof HiveHttpError && error.status === 503);
  for (const route of ['devices', 'pairings/pending', 'audit']) {
    assert.deepEqual(await host.request('GET', `/api/remote/${route}`), []);
  }
  await host.request('PUT', '/api/remote/enabled', { enabled: true });
  assert.equal((await status()).enabled, true);
  await host.request('PUT', '/api/remote/enabled', { enabled: false });
  await assert.rejects(host.request('PUT', '/api/settings/app-state/remote_enabled', { value: 'true' }), (error: unknown) => error instanceof HiveHttpError && error.status === 403);
  const login = await run(process.execPath, [helper, state, 'login', '--gateway', gatewayUrl], { env, timeout: 20_000 });
  assert.ok(login.stdout.includes('Remote access is now enabled'));
  assert.equal(login.stdout.includes(token), false);
  assert.deepEqual(requests, ['/daemon/code', '/daemon/token']);
  assert.equal((await status()).logged_in, true);
  assert.equal((await status()).gateway_url, gatewayUrl);
  assert.equal((await status()).enabled, true);
  await host.request('PUT', '/api/remote/enabled', { enabled: true });
  await waitFor(() => daemon?.readyState === WebSocket.OPEN);
  const ticket = await host.request('POST', '/api/remote/pairings', {}) as Record<string, unknown>;
  assert.equal(typeof ticket.pairing_id, 'string');
  assert.equal(typeof ticket.code, 'string');
  assert.ok(Number(ticket.expires_at) > Date.now());
  await assert.rejects(host.request('POST', '/api/remote/pairings/missing/confirm', {}), (error: unknown) => error instanceof HiveHttpError && error.status === 404);
  await host.request('POST', `/api/remote/pairings/${ticket.pairing_id}/reject`, {});
  // Phone-side X25519 derives its own SAS from the actual daemon websocket ACK.
  // No engine/store/crypto/socket implementation is mocked or injected.
  const require = createRequire(import.meta.url);
  const cryptoPeer = await import(require.resolve('@tt-a1i/hive/dist/src/shared/remote-crypto.js'));
  const pairedTicket = await host.request('POST', '/api/remote/pairings', {}) as { pairing_id: string; qr: string };
  const payload = cryptoPeer.decodePairingPayload(pairedTicket.qr);
  const keypair = cryptoPeer.generateDeviceKeyPair();
  const salt = cryptoPeer.generateSessionSalt();
  daemon!.send('\x00gw:' + JSON.stringify({ t: 'peer-online', role: 'pair', jti: 'owned-pair-session' }));
  daemon!.send(JSON.stringify({ t: 'hello', devicePublicKey: cryptoPeer.toBase64Url(keypair.publicKey), sessionSalt: cryptoPeer.toBase64Url(salt), proposedName: 'Owned phone' }));
  await waitFor(() => frames.some(frame => frame.t === 'pair-ack'));
  const ack = frames.find(frame => frame.t === 'pair-ack')!;
  const phone = cryptoPeer.deriveDeviceSession({
    deviceSecretKey: keypair.secretKey, devicePublicKey: keypair.publicKey,
    daemonPublicKey: cryptoPeer.fromBase64Url(ack.daemonPublicKey),
    pairingSecret: cryptoPeer.fromBase64Url(payload.pairingSecret), sessionSalt: salt,
    ids: { daemonId: ack.daemonId, deviceId: ack.deviceId, protocolVersion: ack.protocolVersion },
  });
  const pending = await host.request('GET', '/api/remote/pairings/pending') as { pairing_id: string; sas: string; device_name: string }[];
  assert.equal(pending.length, 1);
  assert.equal(pending[0]?.sas, phone.sas);
  assert.equal(pending[0]?.device_name, 'Owned phone');
  assert.deepEqual(await host.request('GET', '/api/remote/devices'), []);
  await host.request('POST', `/api/remote/pairings/${pairedTicket.pairing_id}/confirm`, {});
  await waitFor(() => frames.some(frame => frame.t === 'confirmed' && frame.deviceId === ack.deviceId));
  assert.equal(confirmations[0]?.boundJti, 'owned-pair-session');
  let devices = await host.request('GET', '/api/remote/devices') as { id: string; name: string }[];
  assert.equal(devices[0]?.id, ack.deviceId);
  assert.equal(devices[0]?.name, 'Owned phone');
  // Exercise the real encrypted data tunnel, including the original HTTP
  // allowlist and the transplanted settings trust boundary, before revocation.
  const protocol = await import(require.resolve('@tt-a1i/hive/dist/src/shared/remote-protocol.js'));
  const ids = { daemonId: ack.daemonId, deviceId: ack.deviceId, protocolVersion: ack.protocolVersion };
  const phoneSalt = cryptoPeer.generateConnSalt();
  const saltHeader = protocol.encodeHeader({ version: ack.protocolVersion, kind: protocol.FrameKind.Data, flags: 0, streamId: protocol.CONN_SALT_STREAM_ID, seq: 0 });
  daemon!.send(Buffer.concat([saltHeader, protocol.encodeConnSalt({ role: 'device', salt: phoneSalt })]));
  await waitFor(() => binaryFrames.some(frame => protocol.decodeHeader(frame).streamId === protocol.CONN_SALT_STREAM_ID));
  const daemonSaltFrame = binaryFrames.splice(binaryFrames.findIndex(frame => protocol.decodeHeader(frame).streamId === protocol.CONN_SALT_STREAM_ID), 1)[0]!;
  const daemonSalt = protocol.decodeConnSalt(daemonSaltFrame.subarray(protocol.HEADER_BYTES)).salt;
  const keys = cryptoPeer.deriveConnectionKeys({ rootD2p: phone.d2p, rootP2d: phone.p2d, phoneConnSalt: phoneSalt, daemonConnSalt: daemonSalt, ids });
  const sealer = cryptoPeer.createSealer('p2d');
  const opener = cryptoPeer.createOpener('d2p');
  function sendEncrypted(kind: number, streamId: number, payload: Uint8Array) {
    const header = protocol.encodeHeader({ version: ack.protocolVersion, kind, flags: 0, streamId, seq: sealer.nextSeq });
    const sealed = cryptoPeer.sealNext(sealer, { key: keys.p2d, streamId, headerBytes: header, payload });
    daemon!.send(Buffer.concat([header, sealed.ciphertext]));
  }
  sendEncrypted(protocol.FrameKind.Data, 0, protocol.encodeHello({ ...ids, role: 'device' }));
  let nextStream = 1;
  async function encryptedHttp(method: string, path: string, body?: unknown, malformed = false) {
    const stream = nextStream;
    nextStream += 2;
    sendEncrypted(protocol.FrameKind.Open, stream, protocol.encodeOpenPayload({ transport: protocol.StreamTransport.Http, http: { method, path, headers: [['content-type', 'application/json']], hasBody: body !== undefined } }));
    if (body !== undefined) {
      const bytes = new TextEncoder().encode(JSON.stringify(body));
      sendEncrypted(protocol.FrameKind.Data, stream, malformed ? bytes : protocol.encodeHttpBodyChunk(bytes));
    }
    if (!malformed) sendEncrypted(protocol.FrameKind.End, stream, new Uint8Array());
    let statusCode: number | undefined;
    let resetCode: number | undefined;
    const parts: Uint8Array[] = [];
    let ended = false;
    await waitFor(() => {
      for (const frame of binaryFrames.splice(0)) {
        const headerBytes = frame.subarray(0, protocol.HEADER_BYTES);
        const header = protocol.decodeHeader(headerBytes);
        if (header.streamId === protocol.CONN_SALT_STREAM_ID) continue;
        const plaintext = cryptoPeer.openNext(opener, { key: keys.d2p, streamId: header.streamId, headerBytes, ciphertext: frame.subarray(protocol.HEADER_BYTES), seq: header.seq });
        if (header.streamId !== stream) continue;
        if (header.kind === protocol.FrameKind.Reset) {
          assert.ok(malformed, 'Real tunnel rejected a valid test stream');
          resetCode = protocol.decodeResetPayload(plaintext);
          ended = true;
          continue;
        }
        if (header.kind === protocol.FrameKind.Data) {
          const part = protocol.decodeHttpData(plaintext);
          if (part.kind === 'head') statusCode = part.head.status;
          else parts.push(part.data);
        }
        if (header.kind === protocol.FrameKind.End) ended = true;
      }
      return ended;
    });
    return { status: statusCode, body: Buffer.concat(parts).toString(), reset: resetCode };
  }
  const reachable = await encryptedHttp('GET', '/api/remote/devices');
  assert.equal(reachable.status, 200);
  assert.equal(JSON.parse(reachable.body)[0].id, ack.deviceId);
  const malformed = await encryptedHttp('PUT', '/api/settings/app-state/owned-malformed-check', { value: 'must-not-save' }, true);
  assert.equal(malformed.reset, protocol.ResetCode.ProtocolError);
  assert.equal((await encryptedHttp('GET', '/api/remote/devices')).status, 200);
  assert.equal((await host.request('GET', '/api/settings/app-state/owned-malformed-check') as { value: string | null }).value, null);
  for (const key of ['remote_daemon_token', 'remote_gateway_url', 'remote_pending_revokes', 'remote_enabled']) {
    const deniedRead = await encryptedHttp('GET', `/api/settings/app-state/${key}`);
    assert.equal(deniedRead.status, 403);
    assert.equal(deniedRead.body.includes(token), false);
    const deniedWrite = await encryptedHttp('PUT', `/api/settings/app-state/${key}`, { value: 'forbidden' });
    assert.equal(deniedWrite.status, 403);
  }
  await host.request('POST', `/api/remote/devices/${ack.deviceId}/revoke`, {});
  devices = await host.request('GET', '/api/remote/devices') as typeof devices;
  assert.deepEqual(devices, []);
  await waitFor(() => confirmations.some(body => body.deviceId === ack.deviceId && !body.boundJti));
  const queue = await host.request('GET', '/api/settings/app-state/remote_pending_revokes') as { value: string };
  assert.deepEqual(JSON.parse(queue.value), [{ deviceId: ack.deviceId, gatewayUrl }]);
  const audit = await host.request('GET', '/api/remote/audit') as { action: string; device_id: string; result: string }[];
  assert.ok(audit.some(entry => entry.action === 'revoke' && entry.device_id === ack.deviceId && entry.result === 'ok'));
  assert.ok(audit.some(entry => entry.action === 'reject' && entry.device_id === ack.deviceId && entry.result === 'rejected'));
  // A gateway outage must not lose the revocation obligation across restart.
  await host.close();
  gatewayRevokeAvailable = true;
  host = await createHiveHost({ dataDir: join(state, 'hive') });
  await waitFor(async () => ((await host!.request('GET', '/api/settings/app-state/remote_pending_revokes')) as { value: string | null }).value === null);
  assert.ok(confirmations.filter(body => body.deviceId === ack.deviceId && !body.boundJti).length >= 2);
  assert.deepEqual(await host.request('GET', '/api/remote/devices'), []);
  const commands = remoteCommands(state);
  if (process.platform !== 'win32') {
    const result = await run('/bin/sh', ['-c', commands.status], { env, cwd: root, timeout: 20_000 });
    assert.equal(result.stdout.includes(token), false);
    assert.ok(result.stdout.includes(gatewayUrl));
    await assert.rejects(access(join(root, 'SHOULD_NOT_EXIST')), { code: 'ENOENT' });
  }
  await run(process.execPath, [helper, state, 'logout'], { env, timeout: 20_000 });
  assert.equal((await status()).logged_in, false);
  assert.equal((await status()).enabled, false);
  await host.close();
  host = await createHiveHost({ dataDir: join(state, 'hive') });
  assert.equal((await status()).logged_in, false);
  assert.equal((await status()).enabled, false);
  await assert.rejects(access(foreign), { code: 'ENOENT' });
  const invalid = await run(process.execPath, [helper, 'relative', 'status'], { env }).then(() => 0, error => error.code);
  assert.notEqual(invalid, 0);
  console.error('Remote data isolation, native CLI login/logout, real websocket X25519 pairing/SAS, encrypted HTTP/config-key rejection, revocation outage/restart retry, audit and persistence passed. Production gateway/account and Windows UI not covered.');
} finally {
  await host?.close();
  for (const client of sockets.clients) client.terminate();
  await new Promise<void>(resolve => sockets.close(() => resolve()));
  await new Promise<void>(resolve => gateway.close(() => resolve()));
  await rm(root, { recursive: true, force: true });
}

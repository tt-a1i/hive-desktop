import assert from 'node:assert/strict';
import { mkdtemp, readFile, rm, mkdir } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { createAcpSession } from '../src/agents/acp.js';
import type { PiEvent } from '../src/agents/types.js';
const root = await mkdtemp(join(tmpdir(), 'hive-acp-'));
const handles: Awaited<ReturnType<typeof createAcpSession>>[] = [];
let events: PiEvent[] = [];
const start = async (mode = 'normal', sessionFile?: string, id = mode) => { const h = await createAcpSession({ command: process.execPath, args: [resolve('checks/fixtures/acp-peer.cjs'), mode, id], cwd: root, sessionDir: root, ...(sessionFile && { sessionFile }), onEvent: e => events.push(e), requestTimeoutMs: 300, cancelTimeoutMs: 40 }); handles.push(h); return h; };
try {
    await assert.rejects(start('version'), /version/);
    await assert.rejects(start('auth'), /auth_required/);
    const a = await start('normal', undefined, 'a');
    const b = await start('normal', undefined, 'b');
    await Promise.all([a.prompt('A', { deliveryId: 'da' }), b.prompt('B', { deliveryId: 'db' })]);
    assert.deepEqual(a.completedDeliveryIds(), ['da']);
    assert.deepEqual(b.completedDeliveryIds(), ['db']);
    assert(events.some(e => e.type === 'assistant_delta' && e.kind === 'thinking'));
    assert(events.some(e => e.type === 'tool_end' && !e.isError));
    await a.addContext('context', 'fixture');
    assert(a.hasContext('fixture'));
    const file = a.sessionFile!;
    await a.dispose();
    const restored = await start('normal', file, 'a');
    assert.equal(restored.history().length, 2);
    assert.match(JSON.stringify(restored.history()), /restored once/);
    await restored.dispose();
    const again = await start('normal', file, 'a');
    assert.equal(again.history().length, 2);
    assert.deepEqual(again.completedDeliveryIds(), ['da']);
    await again.dispose();
    const offline = await start('no-load', file, 'a');
    assert(offline.readOnly);
    await assert.rejects(offline.prompt('must not create new'), /read-only/);
    const unknown = await start('unknown');
    await unknown.prompt('unsupported request answered');
    const p = await start('permission');
    events = [];
    const turn = p.prompt('permission');
    for (let i = 0; i < 100 && !p.interactions!().length; i++)
        await new Promise(r => setTimeout(r, 5));
    const interaction = p.interactions!()[0]!;
    assert(interaction);
    assert.throws(() => p.validateInteractionResponse!(interaction.id, { optionId: 'made-up' }), /optionId/);
    await p.respondInteraction!(interaction.id, { optionId: 'deny-1' });
    await turn;
    assert.match(JSON.stringify(p.history()), /deny-1/);
    for (const mode of ['wrong-session', 'eof', 'timeout']) {
        const h = await start(mode);
        await assert.rejects(h.prompt('fail', { deliveryId: mode }));
        assert.deepEqual(h.completedDeliveryIds(), []);
        await h.dispose();
        const raw = JSON.parse(await readFile(h.sessionFile!, 'utf8'));
        assert.equal(raw.deliveries[mode], 'sent');
    }
    for (const mode of ['cancel', 'ignore-cancel']) {
        const h = await start(mode);
        const t = h.prompt('wait', { deliveryId: mode });
        await new Promise(r => setTimeout(r, 10));
        await h.abort();
        await assert.rejects(t);
        assert.deepEqual(h.completedDeliveryIds(), []);
    }
    const fault = await start();
    await rm(fault.sessionFile!);
    await mkdir(fault.sessionFile!);
    await assert.rejects(fault.prompt('cannot write', { deliveryId: 'fault' }));
    assert.deepEqual(fault.completedDeliveryIds(), []);
    await assert.rejects(fault.prompt('must pause'));
    console.log('ACP session: isolated peers, version/auth, load/read-only, replay, streaming/patches, permission, unknown methods, EOF/timeout/session mismatch, cancel, durable failure passed');
}
finally {
    await Promise.allSettled(handles.map(h => h.dispose()));
    await rm(root, { recursive: true, force: true });
}

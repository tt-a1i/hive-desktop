import assert from 'node:assert/strict';
import { mkdtemp, readFile, rm, mkdir } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createAcpSession } from '../src/agents/acp.js';
import type { PiEvent } from '../src/agents/types.js';
const root = await mkdtemp(join(tmpdir(), 'hive-acp-'));
const handles: Awaited<ReturnType<typeof createAcpSession>>[] = [];
let events: PiEvent[] = [];
const start = async (mode = 'normal', sessionFile?: string, id = mode, onEvent?: (event: PiEvent) => void) => { const h = await createAcpSession({ command: process.execPath, args: [fileURLToPath(new URL('./fixtures/acp-peer.cjs', import.meta.url)), mode, id, join(root,'native-echo')], cwd: root, sessionDir: root, ...(sessionFile && { sessionFile }), onEvent: e => { events.push(e); onEvent?.(e); }, requestTimeoutMs: mode === 'cancel' || mode === 'ignore-cancel' ? 5000 : 300, cancelTimeoutMs: 40 }); handles.push(h); return h; };
try {
    await assert.rejects(start('normal', join(root, 'missing.json')), /missing/);
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
    await a.prompt('consume durable context once');
    assert.equal(JSON.parse(await readFile(a.sessionFile!, 'utf8')).contexts.length, 0);
    const file = a.sessionFile!;
    await a.dispose();
    await b.prompt('B continues after A closes', {deliveryId:'db2'});
    assert.deepEqual(b.completedDeliveryIds(), ['db','db2']);
    const restored = await start('normal', file, 'a');
    assert.equal(restored.history().length, 2);
    assert.match(JSON.stringify(restored.history()), /restored once/);
    assert(restored.hasContext('fixture'), 'native history replacement preserves local context receipts');
    await restored.dispose();
    const again = await start('normal', file, 'a');
    assert.equal(again.history().length, 2);
    assert.deepEqual(again.completedDeliveryIds(), ['da']);
    await again.dispose();
    const grouped = await start('message-ids', file, 'a');
    assert.equal(grouped.history().length, 3);
    assert.deepEqual(grouped.history().map(m=>(m as {content:{text:string}[]}).content[0]!.text), ['hello','answer1','answer2']);
    await grouped.dispose();
    const offline = await start('no-load', file, 'a');
    assert(offline.readOnly);
    await assert.rejects(offline.prompt('must not create new'), /read-only/);
    const echo = await start('echo');
    await echo.addContext('INTERNAL_STARTUP_INSTRUCTIONS','startup');
    await echo.prompt('actual task',{deliveryId:'echo-delivery',presentation:{version:1,origin:'dispatch',displayText:'visible task',dispatchId:'dispatch-proof',workerId:'worker-proof',messageSequence:7}});
    await echo.dispose();
    const echoCold=await start('echo',echo.sessionFile!);
    const echoHistory=echoCold.history() as {content:{text:string}[];presentation:{dispatchId:string;workerId:string}}[];
    assert.equal(echoHistory[0]!.content[0]!.text,'actual task');
    assert.equal(echoHistory[0]!.presentation.dispatchId,'dispatch-proof');
    assert.equal(echoHistory[0]!.presentation.workerId,'worker-proof');
    assert(!JSON.stringify(echoHistory).includes('INTERNAL_STARTUP_INSTRUCTIONS'));
    const duplicate = await start('echo');
    for(const deliveryId of ['dupe1','dupe2']) {
        await duplicate.addContext('SAME_INTERNAL_STARTUP','duplicate-startup');
        await duplicate.prompt('same task',{deliveryId,presentation:{version:1,origin:'dispatch',displayText:'same task',dispatchId:deliveryId}});
    }
    await duplicate.dispose();
    const ambiguous=await start('echo',duplicate.sessionFile!);
    const ambiguousMessage=ambiguous.history()[0] as {acpReplaySource:string;presentation?:unknown;content:{text:string}[]};
    assert.equal(ambiguousMessage.acpReplaySource,'unknown');
    assert.equal(ambiguousMessage.presentation,undefined);
    assert.equal(ambiguousMessage.content[0]!.text,'same task');
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
        const cold = await start('normal', h.sessionFile!, mode);
        assert.deepEqual(cold.uncertainDeliveryIds(), [mode]);
        await assert.rejects(cold.prompt('must not replay', { deliveryId: mode }), /uncertain/);
    }
    for (const mode of ['cancel', 'ignore-cancel']) {
        let received!: () => void;
        const promptReceived = new Promise<void>(resolve => { received = resolve; });
        const h = await start(mode, undefined, mode, event => {
            if (event.type === 'assistant_delta' && event.delta === 'fixture prompt received') received();
        });
        // Observe rejection immediately, including if persistence fails before the peer receives the prompt.
        const outcome = h.prompt('wait', { deliveryId: mode }).then(
            () => ({ error: undefined }), error => ({ error }));
        await Promise.race([promptReceived, outcome.then(({ error }) => {
            throw new Error(`Prompt ended before the peer acknowledged it: ${String(error)}`);
        })]);
        assert.deepEqual(h.uncertainDeliveryIds(), [mode]);
        await h.abort();
        const { error } = await outcome;
        assert.ok(error instanceof Error);
        assert.match(error.message, mode === 'cancel' ? /cancelled; delivery outcome is uncertain/ : /cancellation timed out/);
        assert.deepEqual(h.completedDeliveryIds(), []);
        assert.deepEqual(h.uncertainDeliveryIds(), [mode]);
        if(mode==='cancel'){await h.prompt('new delivery after confirmed stop',{deliveryId:'after-cancel'});assert.deepEqual(h.completedDeliveryIds(),['after-cancel']);}
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

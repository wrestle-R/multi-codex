import test from 'node:test';
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { copyFile, mkdtemp, mkdir, writeFile, readFile, rm } from 'node:fs/promises';
import { PassThrough } from 'node:stream';
import { randomUUID, createHash } from 'node:crypto';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { localService, syntheticAuth } from './fixtures/local-service.mjs';
const { CodexBridge, JsonLines, RpcPeer, prepareSessionHome, controlEndpoint, AccountClient, BridgeClient } = createRequire(import.meta.url)('../dist/core.cjs');
const engine = process.env.MULTI_CODEX_TEST_ENGINE;
const helper = process.env.MULTI_CODEX_TEST_SHARED_HELPER;
const digest = value => createHash('sha256').update(value).digest('hex');

test('native broker lists foreign full history, fences writers, and continues with B after A handoff', { skip: !engine || !helper }, async () => {
  const root = await mkdtemp(join(tmpdir(), 'mc-shared-native-'));
  const dataRoot = join(root, 'data'); const globalHome = join(root, 'global');
  const service = await localService(); const profiles = [];
  const open = []; let storage;
  try {
    await mkdir(globalHome); await mkdir(dataRoot);
    for (const suffix of ['a', 'b']) {
      const id = randomUUID(); const home = join(dataRoot, 'profiles', id, 'codex-home'); await mkdir(home, { recursive: true });
      await writeFile(join(home, 'auth.json'), JSON.stringify(syntheticAuth(`synthetic-account-${suffix}`)), { mode: 0o600 });
      await writeFile(join(home, 'config.toml'), service.config, { mode: 0o600 });
      profiles.push({ id, name: `Fixture ${suffix}`, authMode: 'ChatGPT', createdAt: new Date().toISOString(), updatedAt: new Date().toISOString() });
    }
    await writeFile(join(dataRoot, 'profiles.json'), JSON.stringify(profiles), { mode: 0o600 });
    const originals = await Promise.all(profiles.map(async p => digest(await readFile(join(dataRoot, 'profiles', p.id, 'codex-home/auth.json')))));
    async function window(profile) {
      const sessionHome = join(dataRoot, 'windows', randomUUID());
      await prepareSessionHome(join(dataRoot, 'profiles', profile.id, 'codex-home'), sessionHome, { sharedChats: true });
      const input = new PassThrough(); const output = new PassThrough(); const notifications = [];
      const bridge = new CodexBridge({ engine, helper: resolve('tests/fixtures/native-chat-helper-proxy.mjs'), dataRoot, globalHome, sessionHome, profileId: profile.id, sharedChats: true, socket: controlEndpoint(sessionHome), token: randomUUID(), workspace: root }, new JsonLines(input, output), ['app-server']);
      const lines = new JsonLines(output, input); lines.on('message', message => { if (message.method) notifications.push(message); });
      const rpc = new RpcPeer(lines, 'native-frontend:');
      const handle = { bridge, rpc }; open.push(handle);
      await bridge.listen(); await rpc.request('initialize', { clientInfo: { name: 'multi_codex_native_shared_test', version: '1' }, capabilities: { experimentalApi: true } }, 60000);
      handle.control = await BridgeClient.connect(bridge.config.socket, bridge.config.token);
      return { ...handle, notifications };
    }
    const a = await window(profiles[0]); const b = await window(profiles[1]);
    const started = await a.rpc.request('thread/start', { cwd: root, model: 'gpt-5.4' }, 60000);
    const id = started.thread.id;
    await a.rpc.request('turn/start', { threadId: id, input: [{ type: 'text', text: 'preserve the complete first-account context' }] }, 60000);
    async function completed(window, count) {
      const deadline = Date.now() + 45000;
      while (window.notifications.filter(message => message.method === 'turn/completed').length < count || !window.bridge.state().canSwitch) {
        if (Date.now() >= deadline) throw new Error('Native fixture turn/checkpoint did not finish');
        await new Promise(resolve => setTimeout(resolve, 25));
      }
    }
    await completed(a, 1);
    const original = (await a.rpc.request('thread/read', { threadId: id, includeTurns: true })).thread;
    const publishedDeadline = Date.now() + 10000;
    while (!b.notifications.some(message => message.method === 'thread/started' && message.params?.thread?.id === id)) {
      if (Date.now() >= publishedDeadline) {
        throw new Error(`Foreign chat was not published: ${a.bridge.state().reason ?? 'no checkpoint'}`);
      }
      await new Promise(resolve => setTimeout(resolve, 25));
    }
    const listing = await b.rpc.request('thread/list', { limit: 100 });
    const listed = listing.data.find(thread => thread.id === id);
    for (const key of ['cliVersion', 'createdAt', 'cwd', 'ephemeral', 'id', 'modelProvider', 'preview', 'projectId', 'sessionId', 'source', 'status', 'turns', 'updatedAt']) assert.ok(Object.hasOwn(listed, key), `Native sidebar field missing: ${key}`);
    assert.deepEqual(listed.turns, []);
    const before = await b.rpc.request('thread/read', { threadId: id, includeTurns: true }, 60000);
    assert.ok(JSON.stringify(before.thread.turns).includes('preserve the complete first-account context'));
    assert.deepEqual(before.thread.turns, original.turns, 'observer retains every native turn item');
    const pagedTurns = await b.rpc.request('thread/turns/list', { threadId: id, limit: 100 });
    assert.deepEqual(pagedTurns.data.map(turn => turn.id).sort(), original.turns.map(turn => turn.id).sort(), 'observer paging retains native turn identities');
    const pagedItems = await b.rpc.request('thread/items/list', { threadId: id, turnId: original.turns[0].id, limit: 100 });
    assert.deepEqual(pagedItems.data.map(entry => entry.item.id).sort(), original.turns[0].items.map(item => item.id).sort(), 'observer paging retains native item identities');
    const observing = await b.rpc.request('thread/resume', { threadId: id });
    assert.deepEqual(observing.thread.turns, original.turns, 'a busy chat opens as a complete read-only snapshot in the sidebar');
    await assert.rejects(b.rpc.request('turn/start', { threadId: id, input: [{ type: 'text', text: 'must remain blocked while A owns the chat' }] }), /another|previous|used by/i);
    const oldA = a.bridge.backend;
    await a.bridge.switchAccount(profiles[1].id);
    assert.ok(oldA.exitCode !== null || oldA.signalCode !== null, 'old native writer must be confirmed stopped');
    const resumed = await b.rpc.request('thread/resume', { threadId: id }, 60000);
    assert.ok(JSON.stringify(resumed.thread.turns).includes('preserve the complete first-account context'));
    assert.deepEqual(resumed.thread.turns, original.turns, 'writer hydration retains every native turn item');
    const requestStart = service.requests.length;
    await b.rpc.request('turn/start', { threadId: id, input: [{ type: 'text', text: 'continue with the second saved account' }] }, 60000);
    await completed(b, 1);
    assert.ok(service.requests.slice(requestStart).some(request => /responses/.test(request.path) && request.account === 'synthetic-account-b'));
    await b.bridge.switchAccount(profiles[0].id);
    storage = new AccountClient(helper, engine, dataRoot, globalHome);
    const saved = await storage.request('chats/read', { threadId: id, includeTurns: true });
    assert.ok(JSON.stringify(saved.thread.turns).includes('continue with the second saved account'));
    assert.deepEqual(saved.thread.turns.slice(0, -1), original.turns);
    const empty = await a.rpc.request('thread/start', { cwd: root, model: 'gpt-5.4' });
    assert.equal(empty.thread.turns.length, 0);
    const emptyWriter = a.bridge.backend;
    await a.control.request('releaseSharedChats');
    assert.ok(emptyWriter.exitCode !== null || emptyWriter.signalCode !== null);
    assert.equal(a.bridge.state().selectedId, profiles[1].id);
    assert.deepEqual(await Promise.all(profiles.map(async p => digest(await readFile(join(dataRoot, 'profiles', p.id, 'codex-home/auth.json'))))), originals);
    if (process.env.MULTI_CODEX_TEST_ROLLOUT_EXPORT) {
      const exported = resolve(process.env.MULTI_CODEX_TEST_ROLLOUT_EXPORT);
      await mkdir(dirname(exported), { recursive: true });
      const checkpoint = await storage.request('chats/get', { threadId: id });
      await copyFile(join(dataRoot, 'shared-chats', 'checkpoints', checkpoint.checkpoint, 'rollout.jsonl'), exported);
    }
  } finally {
    await storage?.dispose();
    for (const window of open) { window.control?.dispose(); window.rpc.close(); await window.bridge.dispose(); }
    await service.close();
    if (process.env.MULTI_CODEX_TEST_KEEP_TEMP === '1') process.stderr.write(`Inert native fixture retained at ${root}\n`);
    else await rm(root, { recursive: true, force: true, maxRetries: 20, retryDelay: 100 });
  }
});

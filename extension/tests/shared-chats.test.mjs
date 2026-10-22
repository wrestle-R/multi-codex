import test from 'node:test';
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { mkdtemp, mkdir, writeFile, readFile, rm, realpath } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
const { SharedChats } = createRequire(import.meta.url)('../dist/core.cjs');

async function fixture(t) {
  const home = await mkdtemp(join(tmpdir(), 'mc-shared-routing-'));
  await mkdir(join(home, 'sessions'));
  const path = join(home, 'sessions', 'full.jsonl');
  const raw = JSON.stringify({ type: 'session_meta', payload: { id: 'native-one' } }) + '\n'
    + JSON.stringify({ type: 'response_item', payload: { type: 'message', content: [{ type: 'input_text', text: 'Full original context' }] } }) + '\n';
  await writeFile(path, raw);
  const calls = []; let busy = false; let verifies = 0; let status = 'idle'; let failure; let stateRead;
  const saved = { id: 'logical-one', nativeId: 'native-one', sourceProfileId: 'account-a', revision: 1, checkpoint: 'fixture', metadata: { id: 'native-one', snippet: 'Foreign account chat', cwd: home, source: 'vscode', modelProvider: 'openai' } };
  const shared = new SharedChats({
    home: () => home, owner: () => ({ instanceId: 'instance', backendPid: 123, sessionHome: home }), profile: () => 'account-b', verifyAccount: async () => { verifies++; },
    helper: async (method, params) => {
      calls.push({ side: 'helper', method, params });
      if (failure === method) { failure = undefined; throw new Error('Fixture catalog commit failure'); }
      if (method === 'chats/list') return params.offset === 0 ? [saved] : [];
      if (method === 'chats/get') return saved;
      if (method === 'chats/read') return { thread: { ...saved.metadata, id: saved.id, turns: [{ items: [{ text: 'Full original context' }] }] } };
      if (method === 'chats/acquire' && busy) throw new Error('Another writer owns this chat');
      if (method === 'chats/hydrate') return { path, thread: saved };
      if (method === 'chats/checkpoint' || method === 'chats/checkpointStopped') return saved;
      return {};
    },
    backend: async (method, params) => {
      calls.push({ side: 'backend', method, params });
      if (method === 'thread/read' && stateRead) status = stateRead();
      if (method === 'thread/loaded/list') return { data: ['native-one'] };
      if (method === 'thread/resume' || method === 'thread/read' || method === 'thread/metadata/update' || method === 'thread/unarchive') return { thread: { ...saved.metadata, path, status: { type: status }, turns: [{ original: true }] } };
      if (method === 'turn/start') return { turn: { id: 'next-turn', status: 'inProgress' } };
      return {};
    },
  });
  t.after(() => rm(home, { recursive: true, force: true }));
  return { shared, home, path, raw, calls, busy: value => { busy = value; }, status: value => { status = value; }, verified: () => verifies, failHelper: method => { failure = method; }, onRead: callback => { stateRead = callback; } };
}
test('foreign chats are listed and fully readable before any resume or writer acquisition', async t => {
  const { shared, calls } = await fixture(t);
  const listing = await shared.request('thread/list', { limit: 20 });
  assert.equal(listing.data[0].id, 'logical-one');
  assert.equal(listing.data[0].status.type, 'notLoaded');
  const read = await shared.request('thread/read', { threadId: 'logical-one', includeTurns: true });
  assert.equal(read.thread.turns[0].items[0].text, 'Full original context');
  assert.equal(calls.some(call => call.side === 'backend'), false);
  assert.equal(calls.some(call => call.method === 'chats/acquire'), false);
});
test('writer acquisition precedes hydration/resume and uses the native identity internally', async t => {
  const { shared, calls, path, verified } = await fixture(t);
  const result = await shared.request('thread/resume', { threadId: 'logical-one' });
  assert.equal(result.thread.id, 'logical-one');
  assert.deepEqual(calls.slice(0, 4).map(call => call.method), ['chats/get', 'chats/acquire', 'chats/hydrate', 'chats/verify']);
  assert.equal(calls.find(call => call.method === 'thread/resume').params.path, path);
  await shared.request('turn/start', { threadId: 'logical-one', input: [] });
  assert.equal(calls.find(call => call.method === 'turn/start').params.threadId, 'native-one');
  assert.ok(verified() >= 3);
  assert.equal(shared.notification({ method: 'turn/completed', params: { threadId: 'native-one' } }).params.threadId, 'logical-one');
});
test('a competing writer blocks backend work while checkpoint observation remains available', async t => {
  const { shared, calls, busy } = await fixture(t); busy(true);
  await assert.rejects(shared.request('turn/start', { threadId: 'logical-one' }), /Another writer/);
  assert.equal(calls.some(call => call.side === 'backend'), false);
  assert.equal((await shared.request('thread/read', { threadId: 'logical-one' })).thread.id, 'logical-one');
});
test('checkpoint includes full rollout assets and is committed before release', async t => {
  const { shared, home, path, raw, calls } = await fixture(t);
  await shared.request('thread/resume', { threadId: 'logical-one' });
  await mkdir(join(home, 'attachments')); const asset = join(home, 'attachments', 'original image.png');
  await writeFile(asset, 'exact asset bytes');
  await writeFile(path, raw + JSON.stringify({ type: 'response_item', payload: { type: 'localImage', path: asset } }) + '\n');
  await shared.drain();
  const checkpoint = calls.find(call => call.method === 'chats/checkpoint');
  assert.deepEqual(checkpoint.params.assets, [[asset, await realpath(asset)]]);
  assert.equal(await readFile(checkpoint.params.path, 'utf8'), await readFile(path, 'utf8'));
  await shared.captureForStop(); await shared.publishAfterStop(); await shared.releaseAfterStop();
  assert.ok(calls.findIndex(call => call.method === 'chats/checkpointStopped') < calls.findIndex(call => call.method === 'chats/release'));
});
test('unverified mutations and direct panel login fail closed', async t => {
  const { shared, calls } = await fixture(t);
  await assert.rejects(shared.request('thread/futureMutation', { threadId: 'logical-one' }), /not been verified/);
  await assert.rejects(shared.request('account/login/start', {}), /Use Multi Codex/);
  assert.equal(calls.length, 0);
});

test('model, metadata and queue edits use the owned native identity and preserve native responses', async t => {
  const { shared, calls } = await fixture(t);
  for (const method of ['thread/settings/update', 'turn/settings/update', 'thread/metadata/update', 'thread/queue/delete', 'thread/queue/update', 'thread/queue/reorder']) {
    const result = await shared.request(method, { threadId: 'logical-one', model: 'fixture-model' });
    assert.equal(calls.findLast(call => call.side === 'backend' && call.method === method).params.threadId, 'native-one');
    if (result.thread) assert.equal(result.thread.id, 'logical-one');
  }
});

test('model/name edits during a running turn succeed without publishing partial history', async t => {
  const { shared, calls, status } = await fixture(t);
  await shared.request('thread/resume', { threadId: 'logical-one' });
  status('active');
  for (const method of ['thread/settings/update', 'thread/name/set']) await shared.request(method, { threadId: 'logical-one' });
  assert.equal(calls.some(call => call.method === 'chats/checkpoint'), false);
  status('idle');
  await shared.checkpoint('logical-one');
  assert.ok(calls.some(call => call.method === 'chats/checkpoint'));
});

test('archive/unarchive mutate native files and return the restored thread, not a fabricated empty response', async t => {
  const { shared, calls } = await fixture(t);
  await shared.request('thread/archive', { threadId: 'logical-one' });
  assert.ok(calls.findIndex(call => call.method === 'thread/archive') < calls.findIndex(call => call.method === 'chats/archive'));
  const restored = await shared.request('thread/unarchive', { threadId: 'logical-one' });
  assert.equal(restored.thread.id, 'logical-one');
  await shared.request('thread/delete', { threadId: 'logical-one' });
  assert.ok(calls.some(call => call.side === 'backend' && call.method === 'thread/delete'));
  const count = calls.length;
  await shared.drain();
  assert.equal(calls.slice(count).some(call => call.method === 'chats/checkpoint'), false);
});

test('observer queues are non-executable and queue edits remain blocked by a competing writer', async t => {
  const { shared, calls, busy } = await fixture(t);
  assert.deepEqual(await shared.request('thread/queue/list', { threadId: 'logical-one' }), { data: [], nextCursor: null });
  busy(true);
  await assert.rejects(shared.request('thread/queue/delete', { threadId: 'logical-one' }), /Another writer/);
  assert.equal(calls.some(call => call.side === 'backend'), false);
});

test('a native-applied archive retries its catalog commit without executing the native mutation twice', async t => {
  const { shared, calls, failHelper, home } = await fixture(t);
  failHelper('chats/archive');
  await assert.rejects(shared.request('thread/archive', { threadId: 'logical-one' }), /catalog commit failure/);
  const intent = JSON.parse(await readFile(join(home, 'chat-operations/logical-one.json'), 'utf8'));
  assert.equal(intent.phase, 'nativeApplied');
  await assert.rejects(shared.request('thread/delete', { threadId: 'logical-one' }), /unfinished library operation/);
  await shared.request('thread/archive', { threadId: 'logical-one' });
  assert.equal(calls.filter(call => call.side === 'backend' && call.method === 'thread/archive').length, 1);
  await assert.rejects(readFile(join(home, 'chat-operations/logical-one.json')), { code: 'ENOENT' });
});

test('work resuming between metadata reads defers a completed edit and keeps strict handoff fenced', async t => {
  const { shared, calls, onRead } = await fixture(t);
  await shared.request('thread/resume', { threadId: 'logical-one' });
  let reads = 0; onRead(() => ++reads === 1 ? 'idle' : 'active');
  await shared.request('thread/name/set', { threadId: 'logical-one', name: 'Native edit succeeds' });
  assert.equal(calls.some(call => call.method === 'chats/checkpoint'), false);
  await assert.rejects(shared.drain(), /finish before publishing/);
});

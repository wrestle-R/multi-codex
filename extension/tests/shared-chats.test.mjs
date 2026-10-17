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
  const calls = []; let busy = false; let verifies = 0;
  const saved = { id: 'logical-one', nativeId: 'native-one', sourceProfileId: 'account-a', revision: 1, checkpoint: 'fixture', metadata: { id: 'native-one', snippet: 'Foreign account chat', cwd: home, source: 'vscode', modelProvider: 'openai' } };
  const shared = new SharedChats({
    home: () => home, owner: () => ({ instanceId: 'instance', backendPid: 123, sessionHome: home }), profile: () => 'account-b', verifyAccount: async () => { verifies++; },
    helper: async (method, params) => {
      calls.push({ side: 'helper', method, params });
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
      if (method === 'thread/loaded/list') return { data: ['native-one'] };
      if (method === 'thread/resume' || method === 'thread/read') return { thread: { ...saved.metadata, path, status: { type: 'idle' }, turns: [{ original: true }] } };
      if (method === 'turn/start') return { turn: { id: 'next-turn', status: 'inProgress' } };
      return {};
    },
  });
  t.after(() => rm(home, { recursive: true, force: true }));
  return { shared, home, path, raw, calls, busy: value => { busy = value; }, verified: () => verifies };
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

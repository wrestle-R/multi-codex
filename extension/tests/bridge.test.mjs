import test from 'node:test';
import assert from 'node:assert/strict';
import { PassThrough } from 'node:stream';
import { mkdtemp, chmod, mkdir, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { createRequire } from 'node:module';
const { controlEndpoint, spawnExecutable, CodexBridge, JsonLines, RpcPeer, BridgeClient } = createRequire(import.meta.url)('../dist/core.cjs');
async function setup(t) {
  const root = await mkdtemp(join(tmpdir(), 'mc-bridge-test-'));
  const engine = resolve('tests/fixtures/fake-engine.mjs'); const helper = resolve('tests/fixtures/fake-accounts.mjs');
  await chmod(engine, 0o755); await chmod(helper, 0o755); await mkdir(join(root, 'home'));
  const input = new PassThrough(); const output = new PassThrough();
  const bridge = new CodexBridge({ engine, helper, dataRoot: root, globalHome: root, sessionHome: join(root, 'home'), socket: controlEndpoint(root.replace(/[^a-zA-Z0-9]/g, '')), token: 'private-test-token', workspace: root }, new JsonLines(input, output), ['app-server']);
  const peer = new RpcPeer(new JsonLines(output, input), 'frontend:');
  await bridge.listen(); await peer.request('initialize'); input.write(JSON.stringify({ method: 'initialized' }) + '\n');
  const client = await BridgeClient.connect(bridge.config.socket, bridge.config.token);
  t.after(async () => { client.dispose(); peer.close(); await bridge.dispose(); await rm(root, { recursive: true, force: true }); });
  return { bridge, client, peer };
}
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
async function until(read, check) { for (let i = 0; i < 100; i++) { if (check(await read())) return; await delay(10); } throw new Error('Condition timed out'); }

test('authentication failure restores the previous verified account without restarting', async t => {
  const { bridge, client, peer } = await setup(t); const pid = bridge.backend.pid;
  await client.request('switch', { id: 'a' }); await peer.request('fixture/control', { mode: 'reject' });
  await assert.rejects(() => client.request('switch', { id: 'b' }), /authentication failure/);
  assert.equal((await client.request('state')).selectedId, 'a'); assert.equal(bridge.backend.pid, pid);
  await peer.request('fixture/control', { mode: '' }); await client.request('switch', { id: 'b' });
  assert.equal((await client.request('state')).selectedId, 'b');
});
test('wrong backend identity rolls back before allowing further work', async t => {
  const { client, peer } = await setup(t); await client.request('switch', { id: 'a' });
  await peer.request('fixture/control', { mode: 'wrong-identity' });
  await assert.rejects(() => client.request('switch', { id: 'b' }), /did not confirm/);
  const state = await client.request('state'); assert.equal(state.selectedId, 'a'); assert.equal(state.canSwitch, true);
});
test('a hidden active thread blocks account switching even without activity notifications', async t => {
  const { client, peer } = await setup(t); await client.request('switch', { id: 'a' });
  await peer.request('fixture/control', { threadStatus: 'active' });
  await assert.rejects(() => client.request('switch', { id: 'b' }), /still running/);
  assert.equal((await client.request('state')).selectedId, 'a');
});
test('simultaneous switches are serialized and new turns cannot enter during authentication', async t => {
  const { client, peer } = await setup(t); await client.request('switch', { id: 'a' });
  await peer.request('fixture/control', { mode: 'hold' });
  const pending = client.request('switch', { id: 'b' });
  await until(() => peer.request('fixture/control'), result => result.held);
  await assert.rejects(() => client.request('switch', { id: 'a' }), /already in progress/);
  await assert.rejects(() => peer.request('turn/start', { threadId: 'hidden-thread' }), /changing accounts/);
  await peer.request('fixture/control', { mode: '', release: true }); await pending;
  assert.equal((await client.request('state')).selectedId, 'b');
});
test('opaque queued work and unresolved approvals fail closed', async t => {
  const { client, peer } = await setup(t); await client.request('switch', { id: 'a' });
  await peer.request('fixture/control', { notification: { method: 'item/commandExecution/requestApproval', id: 'approval-1', params: {} } });
  await assert.rejects(() => client.request('switch', { id: 'b' }), /approval/);
  await peer.request('fixture/control', { notification: { method: 'serverRequest/resolved', params: { requestId: 'approval-1' } } });
  await peer.request('fixture/control', { notification: { method: 'thread/queue/changed', params: { threadId: 'hidden-thread' } } });
  await assert.rejects(() => client.request('switch', { id: 'b' }), /could not be verified/);
});
test('private socket rejects an incorrect token', async t => {
  const { bridge } = await setup(t);
  await assert.rejects(() => BridgeClient.connect(bridge.config.socket, 'incorrect-test-token'), /closed/);
});
test('backend termination closes requests and disables account changes', async t => {
  const { bridge, client, peer } = await setup(t); await client.request('switch', { id: 'a' });
  await assert.rejects(() => peer.request('fixture/crash', {}, 100), /closed|timed out/);
  await until(() => client.request('state'), state => state.connectionFailed);
  await assert.rejects(() => client.request('switch', { id: 'b' }), /connecting/);
  assert.equal(bridge.activity.ready, false);
});
test('ChatGPT and API-key modes can switch both ways in the same backend', async t => {
  const { bridge, client, peer } = await setup(t); const pid = bridge.backend.pid;
  for (const id of ['a', 'k', 'b', 'k']) {
    await client.request('switch', { id }); assert.equal((await client.request('state')).selectedId, id);
    assert.equal((await peer.request('getAuthStatus')).authMethod, id === 'k' ? 'apikey' : 'chatgpt'); assert.equal(bridge.backend.pid, pid);
  }
});
test('work submitted during the activity scan cancels the switch before authentication', async t => {
  const { client, peer } = await setup(t); await client.request('switch', { id: 'a' });
  await peer.request('fixture/control', { mode: 'hold-scan' });
  const switching = client.request('switch', { id: 'b' }); const rejection = assert.rejects(switching, /became busy/);
  await until(() => peer.request('fixture/control'), result => result.held);
  await assert.rejects(() => peer.request('turn/start'), /changing accounts/);
  await peer.request('fixture/control', { mode: '', release: true }); await rejection;
  assert.equal((await client.request('state')).selectedId, 'a');
});
test('unverifiable rollback blocks new work instead of using an uncertain account', async t => {
  const { client, peer } = await setup(t); await client.request('switch', { id: 'a' });
  await peer.request('fixture/control', { mode: 'reject-all' });
  await assert.rejects(() => client.request('switch', { id: 'b' }), /authentication failure/);
  const state = await client.request('state'); assert.equal(state.canSwitch, false); assert.equal(state.authenticationUnverified, true);
  assert.equal(state.selectedId, null); await assert.rejects(() => peer.request('turn/start'), /changing accounts/);
});

import test from 'node:test';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdtemp, mkdir, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { createRequire } from 'node:module';
import { syntheticAuth } from './fixtures/local-service.mjs';
const { controlEndpoint, spawnExecutable, CodexBridge, JsonLines, RpcPeer, findCodexBackend, tapBackend, sameExecutablePath } = createRequire(import.meta.url)('../dist/core.cjs');

test('Windows backend discovery accepts native case and separator variants while keeping executable identity exact', () => {
  assert.equal(sameExecutablePath('D:\\Extensions\\Codex\\codex.exe', 'd:/extensions/codex/codex.exe', 'win32'), true);
  assert.equal(sameExecutablePath('\\\\?\\D:\\Extensions\\Codex\\codex.exe', 'd:/extensions/codex/codex.exe', 'win32'), true);
  assert.equal(sameExecutablePath('D:\\Extensions\\Codex\\codex.exe', 'd:/other/codex.exe', 'win32'), false);
  assert.equal(sameExecutablePath('/extensions/Codex/codex', '/extensions/codex/codex', 'linux'), false);
});

async function setup(t, before = async () => {}) {
  const root = await mkdtemp(join(tmpdir(), 'mc-attachment-test-'));
  const engine = resolve('tests/fixtures/fake-engine.mjs');
  const child = spawnExecutable(engine, ['app-server']);
  const peer = new RpcPeer(new JsonLines(child.stdout, child.stdin), 'original:');
  await peer.request('initialize');
  await peer.request('account/login/start', { type: 'chatgptAuthTokens', accessToken: syntheticAuth('synthetic-account-a').tokens.access_token });
  await before(peer);
  const write = child.stdin.write; const emit = child.stdout.emit;
  const tap = tapBackend(child);
  const home = join(root, 'home'); await mkdir(home);
  const bridge = new CodexBridge({ engine, helper: resolve('tests/fixtures/fake-accounts.mjs'), dataRoot: root, globalHome: root,
    sessionHome: home, socket: controlEndpoint(root.replace(/[^a-zA-Z0-9]/g, '')), token: 'attachment-fixture', workspace: root, attached: true }, tap.frontend, [], tap);
  t.after(async () => { await bridge.dispose(); peer.close(); child.kill(); await rm(root, { recursive: true, force: true }); });
  await bridge.attachReady();
  return { bridge, peer, child, tap, write, emit };
}

test('live attachment reuses the exact backend and keeps the original protocol working after detach', async t => {
  const { bridge, peer, child, write, emit } = await setup(t);
  const pid = child.pid;
  await bridge.switchAccount('b');
  const status = await peer.request('getAuthStatus');
  assert.equal(JSON.parse(Buffer.from(status.authToken.split('.')[1], 'base64url'))['https://api.openai.com/auth'].chatgpt_account_id, 'synthetic-account-b');
  assert.equal(bridge.backend.pid, pid);
  await bridge.dispose();
  assert.equal(child.stdin.write, write); assert.equal(child.stdout.emit, emit);
  assert.equal(child.killed, false);
  assert.equal((await peer.request('fixture/control')).held, false);
});
test('a turn already running before attachment is discovered before changing authentication', async t => {
  const { bridge, peer } = await setup(t, peer => peer.request('fixture/control', { threadStatus: 'active' }));
  await assert.rejects(() => bridge.switchAccount('b'), /still running/);
  const status = await peer.request('getAuthStatus');
  assert.equal(JSON.parse(Buffer.from(status.authToken.split('.')[1], 'base64url'))['https://api.openai.com/auth'].chatgpt_account_id, 'synthetic-account-a');
});
test('live attachment refuses API keys before any authentication mutation', async t => {
  const { bridge, peer } = await setup(t);
  await assert.rejects(() => bridge.switchAccount('k'), /API-key switching requires/);
  assert.equal((await peer.request('getAuthStatus')).authMethod, 'chatgpt');
});
test('failure on the first live switch restores the original unmanaged identity', async t => {
  const { bridge, peer } = await setup(t);
  await peer.request('fixture/control', { mode: 'wrong-identity' });
  await assert.rejects(() => bridge.switchAccount('b'), /did not confirm/);
  const status = await peer.request('getAuthStatus');
  assert.equal(JSON.parse(Buffer.from(status.authToken.split('.')[1], 'base64url'))['https://api.openai.com/auth'].chatgpt_account_id, 'synthetic-account-a');
  assert.equal(bridge.state().authenticationUnverified, false);
});
test('fragmented UTF-8 frames and batched frames reach the original reader exactly once', async t => {
  const { bridge, peer, child } = await setup(t);
  const seen = [];
  peer.lines.on('message', message => { if (message.method === 'fixture/unicode') seen.push(message.params.text); });
  const frame = Buffer.from(JSON.stringify({ method: 'fixture/unicode', params: { text: 'A ✓ 🙂 B' } }) + '\n');
  const split = frame.indexOf(Buffer.from('🙂')) + 2;
  child.stdout.emit('data', frame.subarray(0, split)); child.stdout.emit('data', frame.subarray(split));
  child.stdout.emit('data', Buffer.concat([frame, frame]));
  assert.deepEqual(seen, ['A ✓ 🙂 B', 'A ✓ 🙂 B', 'A ✓ 🙂 B']);
  assert.equal(bridge.state().connectionFailed, false);
});
test('live attachment preserves a long chat response above 8 MiB without losing the connection', async t => {
  const { bridge, peer, child } = await setup(t);
  const seen = [];
  peer.lines.on('message', message => { if (message.method === 'fixture/history') seen.push(message.params.history.length); });
  const history = 'x'.repeat(9 * 1024 * 1024);
  const frame = Buffer.from(JSON.stringify({ method: 'fixture/history', params: { history } }) + '\n');
  for (let offset = 0; offset < frame.length; offset += 32767) child.stdout.emit('data', frame.subarray(offset, offset + 32767));
  assert.deepEqual(seen, [history.length]); assert.equal(bridge.state().connectionFailed, false);
  assert.equal((await peer.request('fixture/control')).held, false);
});
test('an incomplete pre-attachment frame is preserved and disables switching', async t => {
  const { bridge, child } = await setup(t);
  child.stdout.emit('data', Buffer.from('tail-of-an-existing-frame\n'));
  assert.equal(bridge.state().connectionFailed, true);
  await assert.rejects(() => bridge.switchAccount('b'), /connecting/);
});
test('backend discovery refuses ambiguous children and never selects a different executable', async t => {
  const engine = resolve('tests/fixtures/fake-engine.mjs');
  const first = spawnExecutable(engine, ['app-server']);
  const second = spawnExecutable(engine, ['app-server']);
  t.after(() => { first.kill(); second.kill(); });
  assert.throws(() => findCodexBackend(engine), /Multiple Codex backends/);
  assert.equal(findCodexBackend('/not-the-official-codex'), undefined);
});

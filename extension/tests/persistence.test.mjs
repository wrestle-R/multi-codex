import test from 'node:test';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdtemp, mkdir, writeFile, readFile, readlink, stat, rm } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { createHash } from 'node:crypto';
import { createRequire } from 'node:module';
import { syntheticAuth } from './fixtures/local-service.mjs';
const { JsonLines, RpcPeer, BridgeClient, startupHomes } = createRequire(import.meta.url)('../dist/core.cjs');

test('startup uses the current launch home and ignores stale configured history', () => {
  const base = { globalHome: '/global', startupHome: '/configured', historyHome: '/stale-history' };
  assert.deepEqual(startupHomes(base, { CODEX_HOME: '/current' }), { sourceHome: '/current', historyHome: '/current' });
  assert.deepEqual(startupHomes(base, { CODEX_HOME: '/current', CODEX_SQLITE_HOME: '/history' }), { sourceHome: '/current', historyHome: '/history' });
  assert.equal(startupHomes(base, {}).sourceHome, '/configured');
  assert.equal(startupHomes({ globalHome: '/global' }, {}).sourceHome, '/global');
  assert.throws(() => startupHomes(base, { CODEX_HOME: 'relative' }), /absolute/);
});

test('each open preserves the current login and resources, ignoring old project selections', async () => {
  const root = await mkdtemp(join(tmpdir(), 'mc-startup-'));
  const globalHome = join(root, 'global'); const currentHome = join(root, 'account-home'); const nextHome = join(root, 'next-home'); const runtimeRoot = join(root, 'runtime');
  const originalConfig = '[analytics]\nenabled = false\n';
  const originalAuth = JSON.stringify(syntheticAuth('synthetic-account-a'));
  const nextAuth = JSON.stringify(syntheticAuth('synthetic-account-b'));
  for (const [home, auth] of [[globalHome, nextAuth], [currentHome, originalAuth], [nextHome, nextAuth]]) {
    await mkdir(home); await writeFile(join(home, 'config.toml'), originalConfig); await writeFile(join(home, 'auth.json'), auth);
  }
  await mkdir(join(runtimeRoot, 'selections'), { recursive: true });
  await writeFile(join(runtimeRoot, 'selections', createHash('sha256').update(root).digest('hex') + '.json'), JSON.stringify({ id: 'b' }));
  const configPath = join(root, 'config.json');
  await writeFile(configPath, JSON.stringify({ engine: resolve('tests/fixtures/fake-engine.mjs'), helper: resolve('tests/fixtures/fake-accounts.mjs'), dataRoot: root, globalHome, historyHome: globalHome, runtimeRoot }));
  async function start(home) {
    const env = Object.fromEntries(Object.entries(process.env).filter(([key]) => !key.startsWith('CODEX_') && !key.startsWith('OPENAI_') && key !== 'ELECTRON_RUN_AS_NODE'));
    const child = spawn(process.execPath, [resolve('dist/bridge-main.cjs'), '--bridge-config', configPath, 'app-server'], { cwd: root, env: { ...env, CODEX_HOME: home }, stdio: ['pipe', 'pipe', 'pipe'] });
    child.stderr.resume(); const peer = new RpcPeer(new JsonLines(child.stdout, child.stdin), 'test-host:');
    await peer.request('initialize'); child.stdin.write(JSON.stringify({ method: 'initialized' }) + '\n');
    let registration;
    for (let i = 0; i < 100; i++) { registration = await readFile(join(runtimeRoot, 'connections', `${process.pid}.json`), 'utf8').then(JSON.parse).catch(() => null); if (registration) break; await new Promise(resolve => setTimeout(resolve, 10)); }
    const client = await BridgeClient.connect(registration.socket, registration.token);
    return { client, peer, async stop() { client.dispose(); peer.close(); const done = new Promise(resolve => child.once('exit', resolve)); child.stdin.end(); await done; await rm(join(runtimeRoot, 'connections', `${process.pid}.json`), { force: true }); } };
  }
  let running;
  try {
    running = await start(currentHome);
    const homes = await running.peer.request('fixture/homes');
    assert.equal(homes.history, currentHome);
    assert.equal((await stat(join(homes.home, 'auth.json'))).mode & 0o777, 0o600);
    assert.equal(await readlink(join(homes.home, 'skills')), join(currentHome, 'skills'));
    assert.equal(await readlink(join(homes.home, 'sessions')), join(currentHome, 'sessions'));
    assert.equal((await running.peer.request('getAuthStatus')).authToken, JSON.parse(originalAuth).tokens.access_token);
    assert.equal((await running.client.request('state')).selectedId, null);
    assert.equal((await running.client.request('state')).sourceHome, currentHome);
    await running.peer.request('fixture/control', { mode: 'wrong-identity', withholdFileToken: true });
    await assert.rejects(() => running.client.request('switch', { id: 'b' }), /did not confirm/);
    assert.equal((await running.peer.request('getAuthStatus')).authToken, JSON.parse(originalAuth).tokens.access_token);
    await running.peer.request('fixture/control', { mode: '' });
    await running.client.request('switch', { id: 'b' }); await running.stop(); running = undefined;
    running = await start(currentHome);
    assert.equal((await running.client.request('state')).selectedId, null);
    assert.equal((await running.peer.request('getAuthStatus')).authToken, JSON.parse(originalAuth).tokens.access_token);
    await running.stop(); running = undefined;
    running = await start(nextHome);
    assert.equal((await running.peer.request('fixture/homes')).history, nextHome);
    assert.equal((await running.peer.request('getAuthStatus')).authToken, JSON.parse(nextAuth).tokens.access_token);
    assert.equal(await readFile(join(currentHome, 'config.toml'), 'utf8'), originalConfig);
    assert.equal(await readFile(join(currentHome, 'auth.json'), 'utf8'), originalAuth);
    assert.equal(await readFile(join(nextHome, 'auth.json'), 'utf8'), nextAuth);
    assert.equal(await readFile(join(globalHome, 'auth.json'), 'utf8'), nextAuth);
  } finally { await running?.stop(); await rm(root, { recursive: true, force: true }); }
});

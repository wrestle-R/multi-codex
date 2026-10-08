import test from 'node:test';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdtemp, mkdir, writeFile, readFile, rm } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { createRequire } from 'node:module';
const { JsonLines, RpcPeer, BridgeClient } = createRequire(import.meta.url)('../dist/core.cjs');
test('project selection restores on startup while original configuration and auth remain unchanged', async () => {
  const root = await mkdtemp(join(tmpdir(), 'mc-persistence-')); const globalHome = join(root, 'default-home'); const runtimeRoot = join(root, 'runtime');
  await mkdir(globalHome); const originalConfig = '[analytics]\nenabled = false\n'; const originalAuth = '{"test":"source-must-remain-unchanged"}';
  await writeFile(join(globalHome, 'config.toml'), originalConfig); await writeFile(join(globalHome, 'auth.json'), originalAuth);
  const configPath = join(root, 'config.json');
  await writeFile(configPath, JSON.stringify({ engine: resolve('tests/fixtures/fake-engine.mjs'), helper: resolve('tests/fixtures/fake-accounts.mjs'), dataRoot: root, globalHome, runtimeRoot }));
  async function start() {
    const child = spawn(process.execPath, [resolve('dist/bridge-main.cjs'), '--bridge-config', configPath, 'app-server'], { cwd: root, stdio: ['pipe', 'pipe', 'pipe'] });
    child.stderr.resume(); const peer = new RpcPeer(new JsonLines(child.stdout, child.stdin), 'test-host:');
    await peer.request('initialize'); child.stdin.write(JSON.stringify({ method: 'initialized' }) + '\n');
    let registration;
    for (let i = 0; i < 100; i++) { registration = await readFile(join(runtimeRoot, 'connections', `${process.pid}.json`), 'utf8').then(JSON.parse).catch(() => null); if (registration) break; await new Promise(resolve => setTimeout(resolve, 10)); }
    const client = await BridgeClient.connect(registration.socket, registration.token);
    return { child, client, peer, async stop() { client.dispose(); peer.close(); const done = new Promise(resolve => child.once('exit', resolve)); child.stdin.end(); await done; await rm(join(runtimeRoot, 'connections', `${process.pid}.json`), { force: true }); } };
  }
  let running;
  try {
    running = await start(); await running.client.request('switch', { id: 'b' }); await running.stop(); running = undefined;
    running = await start(); let state;
    for (let i = 0; i < 100; i++) { state = await running.client.request('state'); if (state.selectedId === 'b' && state.canSwitch) break; await new Promise(resolve => setTimeout(resolve, 10)); }
    assert.equal(state.selectedId, 'b'); assert.equal(state.canSwitch, true);
    assert.equal(await readFile(join(globalHome, 'config.toml'), 'utf8'), originalConfig);
    assert.equal(await readFile(join(globalHome, 'auth.json'), 'utf8'), originalAuth);
  } finally { await running?.stop(); await rm(root, { recursive: true, force: true }); }
});

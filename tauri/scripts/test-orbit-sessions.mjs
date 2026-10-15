// Real Codex engines against a loopback-only provider and disposable homes.
import assert from 'node:assert/strict';
import { spawn, execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { createRequire } from 'node:module';
import { mkdtemp, mkdir, writeFile, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { randomUUID } from 'node:crypto';
import { once } from 'node:events';
import { localService, syntheticAuth } from '../../extension/tests/fixtures/local-service.mjs';
const exec = promisify(execFile);
const { JsonLines, RpcPeer } = createRequire(import.meta.url)('../../extension/dist/core.cjs');
const engine = process.env.MULTI_CODEX_TEST_ENGINE || '/usr/lib/chatgpt/resources/codex';
const diagnostic = resolve(process.env.MULTI_CODEX_TEST_ORBIT_DIAGNOSTIC || '../cli/target/debug/examples/orbit-sessions');
const root = await mkdtemp(join(tmpdir(), 'mc-orbit-session-'));
const data = join(root, 'data');
const service = await localService(); service.hold();
const children = []; let rpc; let accountRpc;
async function until(read, check) {
  const deadline = Date.now() + 20000; let last;
  while (Date.now() < deadline) { const value = await read(); last = value; if (check(value)) return value; await new Promise(r => setTimeout(r, 150)); }
  throw new Error(`Live Orbit session condition timed out: ${JSON.stringify(last)}; children ${JSON.stringify(children.map(c => ({pid:c.pid,exit:c.exitCode})))}; local request count ${service.requests.length}`);
}
try {
  const homes = []; const logins = [];
  for (let i = 0; i < 2; i++) {
    const home = join(data, 'profiles', randomUUID(), i ? 'bridge-runtime/session' : 'codex-home');
    await mkdir(home, { recursive: true });
    await writeFile(join(home, 'config.toml'), 'cli_auth_credentials_store = "file"\n' + service.config);
    const auth = JSON.stringify(syntheticAuth(`orbit-fixture-${i}`)); logins.push(auth);
    await writeFile(join(home, 'auth.json'), auth, { mode: 0o600 }); homes.push(home);
  }
  function launch(home, args) {
    const env = Object.fromEntries(Object.entries(process.env).filter(([k]) => !/^(CODEX_|OPENAI_|MULTI_CODEX_|NODE_OPTIONS|ELECTRON_RUN_AS_NODE)/.test(k)));
    Object.assign(env, { CODEX_HOME: home, CODEX_SQLITE_HOME: home, HOME: root });
    const child = spawn(engine, args, { cwd: root, env, stdio: ['pipe', 'pipe', 'pipe'] });
    children.push(child); child.stderr.resume(); return child;
  }
  const cli = launch(homes[0], ['exec', '--skip-git-repo-check', '--sandbox', 'read-only', '--json', 'Return the fixture reply.']); cli.stdout.resume(); cli.stdin.end();
  const ide = launch(homes[1], ['app-server', '--stdio']);
  const lines = new JsonLines(ide.stdout, ide.stdin); rpc = new RpcPeer(lines, 'orbit:');
  await rpc.request('initialize', { clientInfo: { name: 'orbit_native_fixture', version: '1' }, capabilities: { experimentalApi: true } });
  lines.send({ method: 'initialized', params: {} });
  const started = await rpc.request('thread/start', { cwd: root, model: 'gpt-5.4' });
  await rpc.request('turn/start', { threadId: started.thread.id, input: [{ type: 'text', text: 'Return the fixture reply.' }] });
  const helper = process.env.MULTI_CODEX_TEST_SHARED_HELPER || resolve('src-tauri/target/debug/multi-codex-account-helper');
  const global = join(root, 'global'); await mkdir(global);
  const accounts = spawn(helper, [engine, data, global], { stdio: ['pipe', 'pipe', 'ignore'] }); children.push(accounts);
  accountRpc = new RpcPeer(new JsonLines(accounts.stdout, accounts.stdin), 'orbit-accounts:');
  const profileId = homes[1].split('/profiles/')[1].split('/')[0];
  const publishSelected = async () => {
    const { thread } = await rpc.request('thread/read', { threadId: started.thread.id, includeTurns: false });
    await accountRpc.request('orbit/publish', { pid: ide.pid, profileId, threads: [thread] });
    return thread;
  };
  const inspect = async () => JSON.parse((await exec(diagnostic, [data])).stdout).combined;
  const native = JSON.parse((await exec(diagnostic, [data])).stdout).native;
  assert.equal(native.some(s => s.pid === ide.pid), false, 'app-server descriptors do not prove a visible IDE chat');
  await publishSelected();
  console.log('fixture: discover CLI and publish authoritative IDE selection');
  const live = await until(inspect, all => all.some(s => s.pid === cli.pid && s.client === 'cli') && all.some(s => s.pid === ide.pid && s.client === 'vscode'));
  assert.equal(live.find(s => s.pid === cli.pid).sessionCount, 1);
  assert.equal(live.find(s => s.pid === ide.pid).sessionCount, 1);
  service.release();
  console.log('fixture: release turns');
  if (cli.exitCode === null) await Promise.race([once(cli, 'exit'), new Promise((_, reject) => setTimeout(() => reject(new Error('CLI exit timed out')), 10000))]);
  await until(publishSelected, thread => thread.status?.type === 'idle');
  const idle = await until(inspect, all => !all.some(s => s.pid === cli.pid) && all.some(s => s.pid === ide.pid && s.states.includes('idle')));
  assert.equal(idle.some(s => s.pid === cli.pid), false, 'saved CLI history must disappear when the process exits');
  console.log('fixture: unsubscribe IDE');
  await rpc.request('thread/unsubscribe', { threadId: started.thread.id });
  // Some engines retain rollout file descriptors after unsubscribe. The
  // existing bridge's authoritative empty UI snapshot must beat that fallback.
  await accountRpc.request('orbit/publish', { pid: ide.pid, profileId, threads: [] });
  const combined = async () => JSON.parse((await exec(diagnostic, [data])).stdout).combined;
  await until(combined, all => !all.some(s => s.pid === ide.pid && s.sessionCount));
  for (let i = 0; i < homes.length; i++) assert.equal(await readFile(join(homes[i], 'auth.json'), 'utf8'), logins[i]);
  console.log('PASS: real CLI/IDE engines discovered, idle retained, exited CLI excluded, authoritative bridge unload overrides retained descriptors, and inert logins preserved.');
} finally {
  service.release(); rpc?.close(); accountRpc?.close();
  for (const child of children) if (child.exitCode === null) { const exited = once(child, 'exit'); child.stdin.end(); child.kill('SIGTERM'); const timer = setTimeout(() => child.kill('SIGKILL'), 5000); await exited; clearTimeout(timer); }
  await service.close(); await rm(root, { recursive: true, force: true });
}

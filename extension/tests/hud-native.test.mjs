import test from 'node:test';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createRequire } from 'node:module';
import { mkdtemp, mkdir, writeFile, readFile, readdir, rm } from 'node:fs/promises';
import { once } from 'node:events';
import { randomUUID } from 'node:crypto';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { localService, syntheticAuth } from './fixtures/local-service.mjs';
const { JsonLines, RpcPeer } = createRequire(import.meta.url)('../dist/core.cjs');
const engine = process.env.MULTI_CODEX_TEST_ENGINE;
const helper = process.env.MULTI_CODEX_TEST_SHARED_HELPER;
function environment(home) {
  const env = { ...process.env, CODEX_HOME: home, CODEX_SQLITE_HOME: home };
  for (const key of Object.keys(env)) if (key.startsWith('OPENAI_') || (key.startsWith('CODEX_') && !['CODEX_HOME', 'CODEX_SQLITE_HOME'].includes(key))) delete env[key];
  return env;
}
async function eventually(read, predicate) {
  const deadline = Date.now() + 15000;
  while (Date.now() < deadline) { const value = await read(); if (predicate(value)) return value; await new Promise(resolve => setTimeout(resolve, 20)); }
  throw new Error('Desktop HUD native fixture did not reach the expected state');
}
test('passive Desktop adapter preserves native auth, observes live work/usage and excludes history/secrets', { skip: !engine || !helper }, async () => {
  const root = await mkdtemp(join(tmpdir(), 'mc-hud-native-')); const home = join(root, 'home');
  await mkdir(home); const profile = randomUUID(); const service = await localService();
  const auth = JSON.stringify(syntheticAuth('synthetic-account-a'));
  await writeFile(join(home, 'auth.json'), auth, { mode: 0o600 }); await writeFile(join(home, 'config.toml'), service.config);
  const child = spawn(helper, ['--hud-proxy', engine, root, profile, 'app-server'], { env: environment(home), stdio: ['pipe', 'pipe', 'ignore'] });
  const rpc = new RpcPeer(new JsonLines(child.stdout, child.stdin), 'hud-native:');
  const snapshot = async () => {
    const files = (await readdir(join(root, 'hud/instances')).catch(() => [])).filter(name => name.endsWith('.json'));
    if (!files.length) return null;
    return JSON.parse(await readFile(join(root, 'hud/instances', files[0]), 'utf8').catch(() => 'null'));
  };
  try {
    await rpc.request('initialize', { clientInfo: { name: 'multi_codex_hud_test', version: '1' }, capabilities: { experimentalApi: true } });
    await rpc.request('account/read', { refreshToken: false });
    await rpc.request('thread/list', {}); assert.deepEqual((await eventually(snapshot, Boolean)).sessions, {});
    const started = await rpc.request('thread/start', { cwd: root, model: 'gpt-5.4' }); const id = started.thread.id;
    await eventually(snapshot, value => value?.sessions[id]); service.hold();
    await rpc.request('turn/start', { threadId: id, input: [{ type: 'text', text: 'NEVER_PERSIST_THIS_PROMPT_OR_CREDENTIAL' }] });
    await eventually(snapshot, value => value?.sessions[id]?.status === 'active'); service.release();
    const saved = await eventually(snapshot, value => value?.sessions[id]?.status === 'idle' && value.sessions[id].tokens?.total != null);
    assert.equal(saved.profileId, profile); assert.ok(saved.sessions[id].tokens.total > 0);
    const encoded = JSON.stringify(saved); assert.equal(encoded.includes('NEVER_PERSIST'), false); assert.equal(encoded.includes('synthetic-refresh'), false); assert.equal(encoded.includes('access_token'), false);
    assert.equal(await readFile(join(home, 'auth.json'), 'utf8'), auth);
    await rpc.request('thread/unsubscribe', { threadId: id }); await eventually(snapshot, value => value && !value.sessions[id]);
  } finally {
    rpc.close(); child.stdin.end(); if (child.exitCode === null) { const exited = once(child, 'exit'); child.kill('SIGTERM'); await exited; }
    await service.close(); await rm(root, { recursive: true, force: true, maxRetries: 20, retryDelay: 100 });
  }
});
test('observer forwards fragmented Unicode and arbitrary RPC bytes exactly, including sensitive messages', { skip: !helper }, async () => {
  const root = await mkdtemp(join(tmpdir(), 'mc-hud-bytes-')); const echo = join(root, 'echo.sh');
  await writeFile(echo, '#!/bin/sh\nexec cat\n', { mode: 0o700 });
  const child = spawn(helper, ['--hud-proxy', echo, root, randomUUID(), 'app-server'], { env: environment(root), stdio: ['pipe', 'pipe', 'ignore'] });
  const output = []; child.stdout.on('data', bytes => output.push(bytes));
  const bytes = Buffer.from(JSON.stringify({ id: 1, result: { authToken: 'NEVER_PERSIST_TOKEN', text: 'हैलो 🌿 ' + 'x'.repeat(2 * 1024 * 1024) } }) + '\n');
  try {
    child.stdin.write(bytes.subarray(0, 35)); child.stdin.end(bytes.subarray(35)); await once(child, 'exit');
    assert.deepEqual(Buffer.concat(output), bytes);
  } finally { if (child.exitCode === null) child.kill(); await rm(root, { recursive: true, force: true }); }
});

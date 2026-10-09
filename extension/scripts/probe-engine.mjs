import { spawn } from 'node:child_process';
import { mkdtemp, writeFile, rm, readdir, readFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createInterface } from 'node:readline';
import { localService, syntheticAuth } from '../tests/fixtures/local-service.mjs';
const binary = process.env.MULTI_CODEX_TEST_ENGINE;
if (!binary) throw new Error('Set MULTI_CODEX_TEST_ENGINE to the official extension bundled engine.');
const home = await mkdtemp(join(tmpdir(), 'multi-codex-engine-probe-'));
const fixture = await localService();
const live = process.env.MULTI_CODEX_TEST_PROFILES;
await writeFile(join(home, 'config.toml'), live ? '[analytics]\nenabled = false\n' : fixture.config, { mode: 0o600 });
const environment = Object.fromEntries(Object.entries(process.env).filter(([key]) => !key.startsWith('CODEX_') && !key.startsWith('OPENAI_')));
const child = spawn(binary, ['app-server'], { env: { ...environment, CODEX_HOME: home, CODEX_SQLITE_HOME: home,
  }, stdio: ['pipe', 'pipe', 'pipe'] });
child.stderr.resume();
const pending = new Map(); const events = []; let id = 0;
const reader = createInterface({ input: child.stdout });
reader.on('line', line => {
  let value; try { value = JSON.parse(line); } catch { return; }
  if (value.id !== undefined && pending.has(value.id)) {
    const request = pending.get(value.id); clearTimeout(request.timer); pending.delete(value.id);
    value.error ? request.reject(new Error(value.error.message)) : request.resolve(value.result);
  } else if (value.method) events.push(value.method);
});
function request(method, params = {}) {
  const requestId = ++id;
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => { pending.delete(requestId); reject(new Error(`${method} timed out`)); }, 20000);
    pending.set(requestId, { resolve, reject, timer }); child.stdin.write(JSON.stringify({ id: requestId, method, params }) + '\n');
  });
}
try {
  await request('initialize', { clientInfo: { name: 'multi_codex_probe', version: JSON.parse(await readFile(new URL('../package.json', import.meta.url), 'utf8')).version }, capabilities: { experimentalApi: true } });
  child.stdin.write(JSON.stringify({ method: 'initialized' }) + '\n');
  let profiles = [syntheticAuth('synthetic-account-a'), syntheticAuth('synthetic-account-b')];
  if (live) {
    profiles = [];
    for (const id of await readdir(live)) {
      try { const auth = JSON.parse(await readFile(join(live, id, 'codex-home', 'auth.json'), 'utf8')); if (auth.tokens?.access_token && auth.tokens?.account_id) profiles.push(auth); } catch {}
      if (profiles.length === 2) break;
    }
    if (profiles.length !== 2) throw new Error('Two saved accounts are required for the live probe.');
  }
  for (const index of [0, 1, 0]) {
    const auth = profiles[index];
    await request('account/login/start', { type: 'chatgptAuthTokens', accessToken: auth.tokens.access_token, chatgptAccountId: auth.tokens.account_id, chatgptPlanType: 'plus' });
    await request('account/read', { refreshToken: false });
    const status = await request('getAuthStatus', { includeToken: true, refreshToken: false });
    const claims = JSON.parse(Buffer.from(status.authToken.split('.')[1], 'base64url').toString());
    if (claims['https://api.openai.com/auth']?.chatgpt_account_id !== auth.tokens.account_id) throw new Error('Backend identity did not match the selected profile.');
    console.log(JSON.stringify({ profile: index === 0 ? 'A' : 'B', identityVerified: true, pid: child.pid }));
  }
  if (!live) {
    const apiKey = 'synthetic-api-key-for-local-test';
    await request('account/logout');
    await request('account/login/start', { type: 'apiKey', apiKey });
    const status = await request('getAuthStatus', { includeToken: true, refreshToken: false });
    if (status.authMethod !== 'apikey' || status.authToken !== apiKey) throw new Error('API-key identity could not be verified');
    console.log(JSON.stringify({ profile: 'API-key fixture', identityVerified: true, pid: child.pid }));
    await request('account/login/start', { type: 'chatgptAuthTokens', accessToken: profiles[0].tokens.access_token, chatgptAccountId: profiles[0].tokens.account_id });
    const restored = await request('getAuthStatus', { includeToken: true, refreshToken: false });
    if (restored.authToken !== profiles[0].tokens.access_token) throw new Error('ChatGPT could not be restored after API-key mode');
  }
  console.log(JSON.stringify({ passed: true, sameBackendPid: child.pid, fixtureRequests: fixture.requests.map(r => r.path), events }));
} finally {
  child.kill(); reader.close(); for (const request of pending.values()) clearTimeout(request.timer);
  await rm(home, { recursive: true, force: true }); await fixture.close();
}

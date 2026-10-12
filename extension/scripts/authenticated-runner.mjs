// Fresh runner credentials remain private. Only encrypted instructions and boolean
// results leave the runner; never upload this script's private working directory.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { publicEncrypt, constants, createHash, randomUUID } from 'node:crypto';
import { mkdir, mkdtemp, readFile, writeFile, rm } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { PassThrough } from 'node:stream';
const { spawnExecutable, terminateExecutable, JsonLines, RpcPeer, AccountClient,
  CodexBridge, BridgeClient, controlEndpoint, helperName } = createRequire(import.meta.url)('../dist/core.cjs');
const output = resolve(process.env.AUTH_OUTPUT);
const engine = resolve(process.env.AUTH_ENGINE);
const key = Buffer.from(process.env.AUTH_PUBLIC_KEY, 'base64').toString();
const root = await mkdtemp(join(tmpdir(), 'multi-codex-real-auth-'));
const dataRoot = join(root, 'data'); const globalHome = join(root, 'global');
const helper = resolve('bin', helperName);
const report = { platform: process.platform, architecture: process.arch, passed: false,
  browserFlow: 'not tested', deviceLogins: [], switching: [] };
const digest = value => createHash('sha256').update(value).digest('hex');
await mkdir(output, { recursive: true });
await mkdir(globalHome, { mode: 0o700 });
let phase = 'setup'; let accounts; let bridge; let client; let frontend;
const saved = [];
async function server(home) {
  await mkdir(home, { recursive: true, mode: 0o700 });
  await writeFile(join(home, 'config.toml'), 'cli_auth_credentials_store = "file"\n[analytics]\nenabled = false\n', { mode: 0o600 });
  const env = Object.fromEntries(Object.entries(process.env).filter(([key]) => !/^(CODEX_|OPENAI_)/.test(key)));
  const child = spawnExecutable(engine, ['app-server'], { env: { ...env, CODEX_HOME: home, CODEX_SQLITE_HOME: home }, cwd: root });
  child.stderr.resume();
  const lines = new JsonLines(child.stdout, child.stdin); const rpc = new RpcPeer(lines);
  child.on('error', () => rpc.close()); child.on('exit', () => rpc.close());
  await rpc.request('initialize', { clientInfo: { name: 'multi_codex_native_acceptance', version: '1' }, capabilities: { experimentalApi: true } });
  lines.send({ method: 'initialized' });
  return { child, lines, rpc, close: async () => { rpc.close(); await terminateExecutable(child); } };
}
async function login(home, name) {
  const backend = await server(home);
  try {
    const completion = new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error('Device sign-in timed out')), 15 * 60_000);
      const handler = message => {
        if (message.method !== 'account/login/completed') return;
        clearTimeout(timer); backend.lines.off('message', handler);
        message.params.success ? resolve() : reject(new Error('Device sign-in failed'));
      };
      backend.lines.on('message', handler);
    });
    // Keep a rejection handler attached while the instructions are prepared.
    completion.catch(() => {});
    const instructions = await backend.rpc.request('account/login/start', { type: 'chatgptDeviceCode' });
    assert.equal(instructions.type, 'chatgptDeviceCode');
    assert.equal(new URL(instructions.verificationUrl).origin, 'https://auth.openai.com');
    const encrypted = publicEncrypt({ key, padding: constants.RSA_PKCS1_OAEP_PADDING, oaepHash: 'sha256' },
      Buffer.from(JSON.stringify({ platform: process.platform, account: name, url: instructions.verificationUrl, code: instructions.userCode })));
    await writeFile(join(output, `signin-${name}.encrypted`), encrypted);
    await completion;
    const result = await backend.rpc.request('account/read', { refreshToken: false });
    assert.equal(result.account?.type, 'chatgpt');
    assert.ok(result.account.email);
    const auth = await readFile(join(home, 'auth.json'), 'utf8');
    const parsed = JSON.parse(auth);
    assert.ok(parsed.tokens?.access_token && parsed.tokens?.account_id);
    return { auth, identity: digest(result.account.email.toLowerCase()), accountId: parsed.tokens.account_id };
  } finally { await backend.close(); }
}
try {
  // Browser OAuth uses the runner's localhost callback. Exercise initiation and
  // cancellation here; a device login below is the real remote browser sign-in.
  phase = 'browser-initiation';
  const browser = await server(join(root, 'browser'));
  try {
    const instructions = await browser.rpc.request('account/login/start', { type: 'chatgpt' });
    assert.equal(instructions.type, 'chatgpt');
    const url = new URL(instructions.authUrl);
    assert.ok(['https://auth.openai.com', 'https://chatgpt.com'].includes(url.origin));
    const redirect = new URL(url.searchParams.get('redirect_uri'));
    assert.equal(redirect.hostname, 'localhost');
    await browser.rpc.request('account/login/cancel', { loginId: instructions.loginId });
    assert.equal((await browser.rpc.request('account/read', { refreshToken: false })).account, null);
    report.browserFlow = 'initiation and cancellation passed; callback completion requires a local tunnel';
  } finally { await browser.close(); }
  accounts = new AccountClient(helper, engine, dataRoot, globalHome);
  const identities = [];
  for (const name of ['a', 'b']) {
    phase = `device-${name}`;
    const result = await login(join(root, name), name);
    if (identities.length) assert.notEqual(result.identity, identities[0].identity, 'Two different accounts are required');
    const added = await accounts.request('accounts/add', { name: `Runner ${name.toUpperCase()}`, authJson: result.auth });
    saved.push(added.metadata.id); identities.push(result);
    report.deviceLogins.push({ account: name, authenticated: true, differentAccount: name === 'b' ? true : null });
    await writeFile(join(output, 'progress.json'), JSON.stringify(report));
  }
  phase = 'production-bridge-switching';
  const input = new PassThrough(); const resultStream = new PassThrough();
  const sessionHome = join(root, 'bridge'); await mkdir(sessionHome, { mode: 0o700 });
  await writeFile(join(sessionHome, 'config.toml'), 'cli_auth_credentials_store = "file"\n[analytics]\nenabled = false\n');
  bridge = new CodexBridge({ engine, helper, dataRoot, globalHome, sessionHome,
    socket: controlEndpoint(randomUUID()), token: randomUUID(), workspace: root }, new JsonLines(input, resultStream), ['app-server']);
  frontend = new RpcPeer(new JsonLines(resultStream, input), 'acceptance:');
  await bridge.listen();
  await frontend.request('initialize', { clientInfo: { name: 'multi_codex_native_acceptance', version: '1' } });
  input.write(JSON.stringify({ method: 'initialized' }) + '\n');
  client = await BridgeClient.connect(bridge.config.socket, bridge.config.token);
  const pid = bridge.backend.pid;
  const credentialHashes = await Promise.all(saved.map(id => readFile(join(dataRoot, 'profiles', id, 'codex-home', 'auth.json')).then(digest)));
  for (const index of [0, 1, 0]) {
    await client.request('switch', { id: saved[index] });
    assert.equal((await client.request('state')).selectedId, saved[index]);
    const status = await frontend.request('getAuthStatus', { includeToken: true, refreshToken: false });
    assert.equal(status.authToken, JSON.parse(identities[index].auth).tokens.access_token);
    const actual = await frontend.request('account/read', { refreshToken: false });
    assert.equal(digest(actual.account.email.toLowerCase()), identities[index].identity);
    // A real authenticated read verifies that this isn't only a cosmetic switch.
    await client.request('usage');
    assert.equal(bridge.backend.pid, pid);
    for (let i = 0; i < saved.length; i++) assert.equal(digest(await readFile(join(dataRoot, 'profiles', saved[i], 'codex-home', 'auth.json'))), credentialHashes[i]);
    report.switching.push({ account: index === 0 ? 'a' : 'b', identityVerified: true, authenticatedUsage: true, sameBackend: true, sourceCredentialsUnchanged: true });
  }
  report.passed = true;
} catch {
  // Backend failures can include auth URLs or tokens. Publish only the stage.
  report.failedStage = phase;
  process.exitCode = 1;
} finally {
  client?.dispose(); frontend?.close(); await bridge?.dispose();
  if (accounts) {
    for (const id of saved) await accounts.request('accounts/delete', { id }).catch(() => { report.cleanupFailed = true; process.exitCode = 1; });
    await accounts.dispose();
  }
  await rm(root, { recursive: true, force: true, maxRetries: 10, retryDelay: 300 }).catch(() => { report.cleanupFailed = true; process.exitCode = 1; });
  if (report.cleanupFailed) report.passed = false;
  await writeFile(join(output, 'result.json'), JSON.stringify(report, null, 2));
  await writeFile(join(output, 'exit-status'), String(process.exitCode || 0));
}

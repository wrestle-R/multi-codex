// Fresh runner credentials remain private. Only encrypted instructions and boolean
// results leave the runner; never upload this script's private working directory.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createRequire } from 'node:module';
import { publicEncrypt, constants, createHash, randomUUID } from 'node:crypto';
import { mkdir, mkdtemp, readFile, writeFile, rm, realpath } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { PassThrough } from 'node:stream';
const { spawnExecutable, terminateExecutable, JsonLines, RpcPeer, AccountClient,
  CodexBridge, BridgeClient, controlEndpoint, helperName, prepareSessionHome, loginParams } = createRequire(import.meta.url)('../dist/core.cjs');
const output = resolve(process.env.AUTH_OUTPUT);
const engine = resolve(process.env.AUTH_ENGINE);
const key = Buffer.from(process.env.AUTH_PUBLIC_KEY, 'base64').toString();
const root = await realpath(await mkdtemp(join(tmpdir(), 'multi-codex-real-auth-')));
const dataRoot = join(root, 'data'); const globalHome = join(root, 'global');
const helper = resolve('bin', helperName);
const report = { platform: process.platform, architecture: process.arch, passed: false,
  browserFlow: 'not tested', deviceLogins: [], switching: [], sharedContinuation: 'not tested' };
report.source = process.env.GITHUB_SHA ?? 'local';
report.engineVersion = spawnSync(engine, ['--version'], { encoding: 'utf8', windowsHide: true }).stdout?.trim();
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
  phase = `device-${name}-initialize`;
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
    phase = `device-${name}-request-code`;
    const instructions = await backend.rpc.request('account/login/start', { type: 'chatgptDeviceCode' });
    assert.equal(instructions.type, 'chatgptDeviceCode');
    assert.equal(new URL(instructions.verificationUrl).origin, 'https://auth.openai.com');
    const encrypted = publicEncrypt({ key, padding: constants.RSA_PKCS1_OAEP_PADDING, oaepHash: 'sha256' },
      Buffer.from(JSON.stringify({ platform: process.platform, account: name, url: instructions.verificationUrl, code: instructions.userCode })));
    await writeFile(join(output, `signin-${name}.encrypted`), encrypted);
    phase = `device-${name}-await-completion`;
    await completion;
    phase = `device-${name}-verify-account`;
    const result = await backend.rpc.request('account/read', { refreshToken: false });
    assert.equal(result.account?.type, 'chatgpt');
    assert.ok(result.account.email);
    const evidence = { account: name, authenticated: true, savedInLauncher: false };
    report.deviceLogins.push(evidence);
    phase = `device-${name}-read-private-credential`;
    const auth = await readFile(join(home, 'auth.json'), 'utf8');
    const parsed = JSON.parse(auth);
    evidence.credentialFormat = { explicitAuthMode: typeof parsed.auth_mode === 'string', accessToken: Boolean(parsed.tokens?.access_token), explicitAccountId: Boolean(parsed.tokens?.account_id) };
    assert.equal(loginParams(parsed).type, 'chatgptAuthTokens');
    return { auth, identity: digest(result.account.email.toLowerCase()) };
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
    assert.ok(['localhost', '127.0.0.1', '[::1]'].includes(redirect.hostname));
    await browser.rpc.request('account/login/cancel', { loginId: instructions.loginId });
    assert.equal((await browser.rpc.request('account/read', { refreshToken: false })).account, null);
    report.browserFlow = 'initiation and cancellation passed; callback completion requires a local tunnel';
  } finally { await browser.close(); }
  accounts = new AccountClient(helper, engine, dataRoot, globalHome);
  const identities = [];
  for (const name of ['a', 'b']) {
    phase = `device-${name}`;
    const result = await login(join(root, name), name);
    phase = `device-${name}-verify-distinct-account`;
    if (identities.length) assert.notEqual(result.identity, identities[0].identity, 'Two different accounts are required');
    phase = `device-${name}-save-account`;
    const added = await accounts.request('accounts/add', { name: `Runner ${name.toUpperCase()}`, authJson: result.auth });
    assert.equal(typeof added.id, 'string');
    saved.push(added.id); identities.push(result);
    Object.assign(report.deviceLogins.at(-1), { savedInLauncher: true, differentAccount: name === 'b' ? true : null });
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
  phase = 'shared-account-continuation';
  client.dispose(); frontend.close(); await bridge.dispose();
  client = undefined; frontend = undefined; bridge = undefined;
  const sharedInput = new PassThrough(); const sharedOutput = new PassThrough();
  const sharedHome = join(dataRoot, 'windows', randomUUID());
  const workspace = join(root, 'empty-workspace'); await mkdir(workspace);
  await prepareSessionHome(join(dataRoot, 'profiles', saved[0], 'codex-home'), sharedHome, { sharedChats: true });
  bridge = new CodexBridge({ engine, helper, dataRoot, globalHome, sessionHome: sharedHome,
    sharedChats: true, profileId: saved[0], socket: controlEndpoint(randomUUID()), token: randomUUID(), workspace }, new JsonLines(sharedInput, sharedOutput), ['app-server']);
  const sharedLines = new JsonLines(sharedOutput, sharedInput);
  frontend = new RpcPeer(sharedLines, 'shared-acceptance:');
  await bridge.listen();
  await frontend.request('initialize', { clientInfo: { name: 'multi_codex_authenticated_sharing_acceptance', version: '1' }, capabilities: { experimentalApi: true } });
  sharedInput.write(JSON.stringify({ method: 'initialized' }) + '\n');
  client = await BridgeClient.connect(bridge.config.socket, bridge.config.token);
  async function minimalTurn(threadId, text) {
    let handler;
    let timer;
    const completion = new Promise((resolve, reject) => {
      timer = setTimeout(() => reject(new Error('Authenticated shared turn timed out')), 120000);
      handler = message => {
        if (message.method === 'turn/completed' && message.params.threadId === threadId) {
          message.params.turn.status === 'completed' ? resolve() : reject(new Error('Authenticated shared turn failed'));
        }
      };
      sharedLines.on('message', handler);
    });
    completion.catch(() => {});
    try {
      await frontend.request('turn/start', { threadId, input: [{ type: 'text', text }] });
      await completion;
    } finally { clearTimeout(timer); sharedLines.off('message', handler); }
  }
  phase = 'shared-account-a-start-thread';
  // Cross-account continuation must not inherit an account-specific preferred
  // model. Use the same explicit baseline as the native sharing acceptance.
  const sharedModel = 'gpt-5.4';
  const started = await frontend.request('thread/start', { cwd: workspace, model: sharedModel, sandbox: 'read-only', approvalPolicy: 'never' });
  const threadId = started.thread.id;
  phase = 'shared-account-a-first-turn';
  await minimalTurn(threadId, 'Reply with just the word ready.');
  phase = 'shared-account-a-read-thread';
  const original = (await frontend.request('thread/read', { threadId, includeTurns: true })).thread;
  const writerA = bridge.backend;
  phase = 'shared-switch-to-account-b';
  await client.request('switch', { id: saved[1] });
  assert.ok(writerA.exitCode !== null || writerA.signalCode !== null);
  assert.equal(digest((await frontend.request('account/read', { refreshToken: false })).account.email.toLowerCase()), identities[1].identity);
  const observed = await frontend.request('thread/read', { threadId, includeTurns: true });
  assert.deepEqual(observed.thread.turns, original.turns);
  const resumed = await frontend.request('thread/resume', { threadId });
  assert.deepEqual(resumed.thread.turns, original.turns);
  phase = 'shared-account-b-continuation';
  await minimalTurn(threadId, 'Reply with just the word continued.');
  const continued = (await frontend.request('thread/read', { threadId, includeTurns: true })).thread;
  assert.equal(continued.turns.length, original.turns.length + 1);
  assert.deepEqual(continued.turns.slice(0, -1), original.turns);
  phase = 'shared-return-to-account-a';
  await client.request('switch', { id: saved[0] });
  assert.equal(digest((await frontend.request('account/read', { refreshToken: false })).account.email.toLowerCase()), identities[0].identity);
  for (let i = 0; i < saved.length; i++) assert.equal(digest(await readFile(join(dataRoot, 'profiles', saved[i], 'codex-home', 'auth.json'))), credentialHashes[i]);
  report.sharedContinuation = { fullNativeContext: true, writerStoppedBeforeHandoff: true, authenticatedAccountBContinuation: true, returnToAccountA: true, sourceCredentialsUnchanged: true, realModelTurns: 2, model: sharedModel };
  report.passed = true;
} catch (error) {
  // Backend failures can include auth URLs or tokens. Publish only the stage.
  report.failedStage = phase;
  if (['ENOENT', 'ETIMEDOUT', 'ERR_ASSERTION'].includes(error.code)) report.failureCode = error.code;
  if (error.message === 'Device sign-in failed') report.failureCode = 'device-login-rejected';
  if (error.message === 'Device sign-in timed out') report.failureCode = 'device-login-timeout';
  if (/keyring|keychain|credential store/i.test(error.message) && !/eyJ|sk-/.test(error.message)) report.failureCode = 'credential-store-error';
  if (error.message === 'Auth JSON is missing a recognized auth_mode') report.failureCode = 'auth-mode-missing';
  if (error.message === 'Codex did not recognize this credential. Sign in again.') report.failureCode = 'native-credential-validation-failed';
  process.exitCode = 1;
} finally {
  client?.dispose(); frontend?.close(); await bridge?.dispose();
  if (accounts) {
    // Recover IDs even if a check failed immediately after creating an account.
    // This helper only owns the fresh disposable dataRoot above.
    const remaining = await accounts.list().catch(() => { report.cleanupFailed = true; process.exitCode = 1; return []; });
    for (const id of new Set([...saved, ...remaining.map(account => account.id)])) await accounts.request('accounts/delete', { id }).catch(() => { report.cleanupFailed = true; process.exitCode = 1; });
    await accounts.dispose();
  }
  await rm(root, { recursive: true, force: true, maxRetries: 10, retryDelay: 300 }).catch(() => { report.cleanupFailed = true; process.exitCode = 1; });
  if (report.cleanupFailed) report.passed = false;
  await writeFile(join(output, 'result.json'), JSON.stringify(report, null, 2));
  await writeFile(join(output, 'exit-status'), String(process.exitCode || 0));
}

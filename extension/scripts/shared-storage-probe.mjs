// Disposable native feasibility gate. Never reads or changes production profiles.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { spawn } from 'node:child_process';
import { mkdtemp, mkdir, writeFile, readFile, readdir, cp, symlink, rm, realpath } from 'node:fs/promises';
import { pathToFileURL } from 'node:url';
import { join, resolve, relative, dirname } from 'node:path';
import { tmpdir } from 'node:os';
import { createHash } from 'node:crypto';
import { localService, syntheticAuth } from '../tests/fixtures/local-service.mjs';
const { spawnExecutable, terminateExecutable, JsonLines, RpcPeer, bundledEnginePath } = createRequire(import.meta.url)('../dist/core.cjs');
const root = await realpath(await mkdtemp(join(tmpdir(), 'mc-sharing-gate-')));
const source = resolve(process.env.MULTI_CODEX_TEST_EXTENSIONS);
const code = process.env.MULTI_CODEX_TEST_VSCODE;
const cli = process.env.MULTI_CODEX_TEST_CODE_CLI;
let cliScript = process.env.MULTI_CODEX_TEST_CODE_NODE_CLI;
if (process.platform === 'win32') {
  const launcher = await readFile(join(dirname(code), 'bin/code.cmd'), 'utf8');
  const active = launcher.match(/"%~dp0\.\.\\([^"\r\n]+\\cli\.js)"/i);
  if (!active || active[1].split('\\').some(part => part === '..')) throw new Error('Unsupported installed editor CLI layout');
  cliScript = join(dirname(code), ...active[1].split('\\'));
}
const report = { platform: process.platform, architecture: process.arch, passed: false, checks: {} };
const output = resolve(process.env.SHARED_GATE_OUTPUT);
await mkdir(output, { recursive: true });
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
async function hashTree(path) {
  const hash = createHash('sha256');
  async function walk(directory) {
    for (const entry of (await readdir(directory, { withFileTypes: true })).sort((a,b) => a.name.localeCompare(b.name))) {
      const child = join(directory, entry.name);
      hash.update(relative(path, child));
      if (entry.isDirectory()) await walk(child);
      else if (entry.isFile()) hash.update(await readFile(child));
      else throw new Error('Unexpected link inside staged extension payload');
    }
  }
  await walk(path); return hash.digest('hex');
}
async function command(args) {
  return new Promise(resolve => {
    const env = { ...process.env };
    delete env.VSCODE_IPC_HOOK_CLI; delete env.VSCODE_PID; delete env.VSCODE_DEV;
    if (cliScript) env.ELECTRON_RUN_AS_NODE = '1';
    const child = spawn(cliScript ? code : cli, cliScript ? [cliScript, ...args] : args,
      { env, windowsHide: true, stdio: ['ignore', 'ignore', 'pipe'] });
    let diagnostic = '';
    child.stderr.on('data', bytes => { if (diagnostic.length < 2000) diagnostic += bytes.toString(); });
    const timeout = setTimeout(() => child.kill(), 180_000);
    child.once('error', () => { clearTimeout(timeout); resolve(false); });
    child.once('exit', (status, signal) => {
      clearTimeout(timeout);
      if (status !== 0) report.lastEditorCommandFailure = { status, signal, diagnostic: diagnostic.replace(/(?:sk-[A-Za-z0-9_-]{20,}|eyJ[A-Za-z0-9_.-]{30,})/g, '[redacted]') };
      resolve(status === 0);
    });
  });
}
let gui; let guiDiagnostic = ''; let a; let b; let fixture; let phase = 'extension-catalogs';
try {
  const payload = join(root, 'payload'); await cp(source, payload, { recursive: true, dereference: true });
  const original = await hashTree(payload);
  const registry = JSON.parse(await readFile(join(payload, 'extensions.json'), 'utf8'));
  const packages = registry.filter(entry => entry.identifier.id === 'openai.chatgpt' || entry.identifier.id === 'openai.codex-audio');
  assert.ok(packages.some(entry => entry.identifier.id === 'openai.chatgpt'));
  async function catalog(name) {
    const extensions = join(root, name, 'extensions'); const user = join(root, name, 'user');
    await mkdir(extensions, { recursive: true }); await mkdir(join(user, 'User'), { recursive: true });
    await writeFile(join(user, 'User', 'settings.json'), JSON.stringify({ 'extensions.autoUpdate': false,
      'extensions.autoCheckUpdates': false, 'telemetry.telemetryLevel': 'off', 'workbench.startupEditor': 'none',
      'security.workspace.trust.enabled': false, 'chatgpt.openOnStartup': false }));
    const entries = [];
    for (const row of packages) {
      const name = row.relativeLocation;
      assert.equal(name, name.split(/[\\/]/).at(-1));
      await symlink(join(payload, name), join(extensions, name), process.platform === 'win32' ? 'junction' : 'dir');
      const location = pathToFileURL(join(extensions, name));
      entries.push({ ...row, location: { ...row.location, authority: location.host, path: decodeURIComponent(location.pathname) } });
    }
    await writeFile(join(extensions, 'extensions.json'), JSON.stringify(entries));
    return { extensions, user, args: ['--user-data-dir', user, '--extensions-dir', extensions] };
  }
  const ca = await catalog('editor-a'); const cb = await catalog('editor-b');
  assert.ok(await command([...ca.args, '--list-extensions']));
  assert.ok(await command([...cb.args, '--list-extensions']));
  const guiEnv = { ...process.env };
  for (const key of Object.keys(guiEnv)) if (/^(CODEX_|OPENAI_)/.test(key)) delete guiEnv[key];
  const guiHome = join(root, 'gui-home');
  await mkdir(guiHome, { mode: 0o700 });
  await writeFile(join(guiHome, 'config.toml'), 'cli_auth_credentials_store = "file"\n[analytics]\nenabled = false\n');
  guiEnv.CODEX_HOME = guiHome; guiEnv.CODEX_SQLITE_HOME = guiHome;
  delete guiEnv.ELECTRON_RUN_AS_NODE; delete guiEnv.VSCODE_IPC_HOOK_CLI; delete guiEnv.VSCODE_PID;
  gui = spawn(code, [...ca.args, '--new-window', '--skip-welcome', '--skip-release-notes', '--disable-gpu',
    ...(process.platform === 'linux' ? ['--no-sandbox', '--ozone-platform=x11'] : []), root],
    { windowsHide: true, stdio: ['ignore', 'ignore', 'pipe'], env: guiEnv, cwd: root });
  gui.stderr.on('data', bytes => { if (guiDiagnostic.length < 3000) guiDiagnostic += bytes.toString(); });
  await delay(6000); assert.equal(gui.exitCode, null, 'Native editor must remain running during maintenance');
  const replacementSucceeded = await command([...cb.args, '--install-extension', 'openai.chatgpt@26.1002.51308', '--force']);
  assert.equal(await hashTree(payload), original, 'Native replacement followed a private package link into shared payload');
  assert.equal(gui.exitCode, null, 'Native replacement interrupted the peer editor');
  report.checks.privateCatalogReplacementLeavesPayloadAndPeerIntact = true;
  report.checks.nativeReplacementAccepted = replacementSucceeded;
  assert.ok(await command([...cb.args, '--uninstall-extension', 'openai.chatgpt']));
  assert.equal(await hashTree(payload), original, 'Uninstall followed a package link into shared payload');
  assert.ok(await command([...ca.args, '--list-extensions']));
  const aRegistry = JSON.parse(await readFile(join(ca.extensions, 'extensions.json'), 'utf8'));
  assert.ok(aRegistry.some(row => row.identifier.id === 'openai.chatgpt'));
  assert.equal(gui.exitCode, null, 'Peer editor was terminated by catalog maintenance');
  report.checks.privateCatalogUninstallLeavesSharedPayloadAndPeerIntact = true;
  // Test the direct writable-root proposal too: the other user's native catalog
  // changes without any app lock. This is why private catalogs are necessary.
  const common = join(root, 'unsafe-common'); await cp(source, common, { recursive: true, dereference: true });
  const before = await hashTree(common);
  assert.ok(await command(['--user-data-dir', join(root, 'unsafe-user'), '--extensions-dir', common, '--uninstall-extension', 'openai.chatgpt']));
  report.checks.writableCommonRootCanBeMutatedByNativeEditor = (await hashTree(common)) !== before;
  assert.equal(report.checks.writableCommonRootCanBeMutatedByNativeEditor, true);
  await terminateExecutable(gui); gui = undefined;
  phase = 'cross-home-full-history';
  const official = packages.find(row => row.identifier.id === 'openai.chatgpt');
  const engine = await bundledEnginePath(join(payload, official.relativeLocation));
  fixture = await localService();
  async function server(name) {
    const home = join(root, name); await mkdir(home);
    await writeFile(join(home, 'auth.json'), JSON.stringify(syntheticAuth(`synthetic-account-${name}`)));
    await writeFile(join(home, 'config.toml'), fixture.config);
    const env = Object.fromEntries(Object.entries(process.env).filter(([key]) => !/^(CODEX_|OPENAI_)/.test(key)));
    const child = spawnExecutable(engine, ['app-server'], { env: { ...env, CODEX_HOME: home, CODEX_SQLITE_HOME: home } });
    child.stderr.resume(); const lines = new JsonLines(child.stdout, child.stdin); const rpc = new RpcPeer(lines);
    await rpc.request('initialize', { clientInfo: { name: 'multi_codex_shared_storage_probe', version: '1' }, capabilities: { experimentalApi: true } });
    lines.send({ method: 'initialized' }); return { home, child, lines, rpc };
  }
  async function turn(backend, threadId, text) {
    const done = new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error('Fixture turn timed out')), 30_000);
      backend.lines.on('message', message => { if (message.method === 'turn/completed') { clearTimeout(timer); resolve(message.params.turn); } });
    });
    await backend.rpc.request('turn/start', { threadId, input: [{ type: 'text', text }] });
    assert.equal((await done).status, 'completed');
  }
  a = await server('a'); const started = await a.rpc.request('thread/start', { cwd: root, approvalPolicy: 'never', sandbox: 'read-only' });
  const id = started.thread.id; await turn(a, id, 'Remember complete fixture context and tool history.');
  const originalThread = (await a.rpc.request('thread/read', { threadId: id, includeTurns: true })).thread;
  assert.ok(originalThread.turns.length);
  a.rpc.close(); await terminateExecutable(a.child); a = undefined;
  b = await server('b');
  const path = join(b.home, relative(join(root, 'a'), originalThread.path));
  assert.ok(path.startsWith(b.home)); await mkdir(dirname(path), { recursive: true }); await cp(originalThread.path, path);
  const resumed = (await b.rpc.request('thread/resume', { threadId: id, path })).thread;
  assert.equal(resumed.id, id); assert.deepEqual(resumed.turns, originalThread.turns);
  assert.ok((await b.rpc.request('thread/list')).data.some(thread => thread.id === id));
  await turn(b, id, 'Continue the exact same chat under account B.');
  const continued = (await b.rpc.request('thread/read', { threadId: id, includeTurns: true })).thread;
  assert.equal(continued.turns.length, originalThread.turns.length + 1);
  assert.deepEqual(continued.turns.slice(0, -1), originalThread.turns);
  assert.deepEqual(fixture.requests.filter(request => /responses(?:\?|$)/.test(request.path)).map(request => request.account), ['synthetic-account-a', 'synthetic-account-b']);
  report.checks.fullHistoryCrossHomeResumeAndNativeList = true;
  report.checks.nextRequestUsesSelectedAccount = true;
  report.passed = true;
} catch (error) {
  // This probe only uses inert credentials. Emit diagnostic text for reproducibility.
  report.failedStage = phase; report.failure = error.message; process.exitCode = 1;
  if (guiDiagnostic) report.editorDiagnostic = guiDiagnostic.replace(/(?:sk-[A-Za-z0-9_-]{20,}|eyJ[A-Za-z0-9_.-]{30,})/g, '[redacted]');
} finally {
  for (const backend of [a,b]) if (backend) { backend.rpc.close(); await terminateExecutable(backend.child); }
  if (gui) await terminateExecutable(gui);
  await fixture?.close();
  await rm(root, { recursive: true, force: true, maxRetries: 10, retryDelay: 300 });
  await writeFile(join(output, 'result.json'), JSON.stringify(report, null, 2));
  console.log(JSON.stringify(report, null, 2));
}

// Run after `cargo build --bin multi-codex-desktop`, with the Vite dev server running.
// This needs a live Hyprland session; all app data and Code profiles are disposable.
import assert from 'node:assert/strict';
import { execFile, spawn } from 'node:child_process';
import { promisify } from 'node:util';
import { mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
const exec = promisify(execFile);
if (!process.env.HYPRLAND_INSTANCE_SIGNATURE) throw new Error('A live Hyprland session is required');
const root = await mkdtemp(join(tmpdir(), 'multi-codex-launcher-'));
const env = { ...process.env, XDG_DATA_HOME: join(root, 'data'), XDG_CONFIG_HOME: join(root, 'config'), CODEX_HOME: join(root, 'codex-home') };
delete env.ELECTRON_RUN_AS_NODE; delete env.APPIMAGE; delete env.VSCODE_IPC_HOOK_CLI;
await mkdir(env.CODEX_HOME, { recursive: true }); await mkdir(join(root, 'workspace'));
const launcher = process.env.MULTI_CODEX_TEST_LAUNCHER || resolve('src-tauri/target/debug/multi-codex-desktop');
const app = spawn(launcher, [], { env, stdio: ['ignore', 'pipe', 'pipe'] });
let diagnostics = '';
for (const stream of [app.stdout, app.stderr]) stream.on('data', chunk => { diagnostics = (diagnostics + chunk.toString()).slice(-6000); });
let code;
async function clients() { return JSON.parse((await exec('hyprctl', ['clients', '-j'])).stdout); }
async function until(read, check) {
  const end = Date.now() + 20000;
  while (Date.now() < end) { const value = await read(); if (check(value)) return value; await new Promise(resolve => setTimeout(resolve, 100)); }
  throw new Error('Live launcher window check timed out');
}
async function stop(child) {
  if (!child || child.exitCode !== null || child.signalCode !== null) return;
  const exited = new Promise(resolve => child.once('exit', resolve));
  child.kill('SIGTERM');
  const timeout = setTimeout(() => child.kill('SIGKILL'), 3000);
  try { await exited; } finally { clearTimeout(timeout); }
}
try {
  const before = await until(async () => (await clients()).find(c => c.pid === app.pid), c => c && c.fullscreen === 1 && !c.floating);
  const addresses = new Set((await clients()).map(c => c.address));
  code = spawn('/usr/share/code/code', ['--user-data-dir', join(root, 'code-user'), '--extensions-dir', join(root, 'code-extensions'),
    '--disable-extensions', '--skip-welcome', '--skip-release-notes', '--disable-gpu', '--no-sandbox', '--ozone-platform=x11', join(root, 'workspace')], { env, stdio: 'ignore' });
  await until(clients, list => list.some(c => !addresses.has(c.address) && /code/i.test(c.class)));
  // Let focus and maximize transitions settle after the new window maps.
  await new Promise(resolve => setTimeout(resolve, 1000));
  const afterLaunch = (await clients()).find(c => c.pid === app.pid);
  assert.ok(afterLaunch, 'The launcher disappeared'); assert.equal(afterLaunch.floating, false, 'Launching Code turned Multi Codex into a floating popup');
  code.kill('SIGTERM');
  await until(clients, list => !list.some(c => !addresses.has(c.address) && /code/i.test(c.class)));
  const afterClose = (await clients()).find(c => c.pid === app.pid);
  assert.equal(afterClose.floating, false, 'The launcher retained a floating restore state');
  const details = value => ({ floating: value.floating, fullscreen: value.fullscreen, size: value.size });
  const report = { passed: true, before: details(before), afterLaunch: details(afterLaunch), afterClose: details(afterClose), temporaryData: true };
  await mkdir('test-results', { recursive: true }); await writeFile('test-results/launcher-window.json', JSON.stringify(report, null, 2));
  console.log(JSON.stringify(report, null, 2));
} catch (error) {
  console.error({ appPid: app.pid, exitCode: app.exitCode, diagnostics,
    windows: (await clients()).filter(c => c.pid === app.pid).map(c => ({ pid: c.pid, floating: c.floating, fullscreen: c.fullscreen, size: c.size })) });
  throw error;
} finally {
  await stop(code); await stop(app);
  await rm(root, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 });
}

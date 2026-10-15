// Explicit live-Hyprland test, isolated from every saved account and setting.
// Build the debug launcher and run Vite on 1420 before invoking this script.
import assert from 'node:assert/strict';
import { spawn, execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { once } from 'node:events';
import { mkdtemp, mkdir, writeFile, readFile, rm } from 'node:fs/promises';
import { randomUUID } from 'node:crypto';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
const exec = promisify(execFile);
if (!process.env.HYPRLAND_INSTANCE_SIGNATURE) throw new Error('Requires an isolated live Hyprland test session');
if (process.env.MULTI_CODEX_TEST_HUD_ISOLATED_BUS !== '1') throw new Error('Use dbus-run-session with MULTI_CODEX_TEST_HUD_ISOLATED_BUS=1; never toggle the installed launcher during a fixture');
const clients = async () => JSON.parse((await exec('hyprctl', ['-j', 'clients'])).stdout);
async function until(read, check) {
  const deadline = Date.now() + 15000;
  while (Date.now() < deadline) { const value = await read(); if (check(value)) return value; await new Promise(r => setTimeout(r, 80)); }
  throw new Error('HUD native window condition timed out');
}
const root = await mkdtemp(join(tmpdir(), 'mc-hud-window-'));
const before = JSON.parse((await exec('hyprctl', ['-j', 'activewindow'])).stdout);
const data = join(root, 'data'); const globalHome = join(root, 'global'); const home = join(root, 'user');
await Promise.all([mkdir(data), mkdir(globalHome), mkdir(home)]);
const id = randomUUID(); await mkdir(join(data, 'profiles', id, 'codex-home'), { recursive: true });
await writeFile(join(data, 'profiles.json'), JSON.stringify([{ id, name: 'Disposable HUD fixture', authMode: 'API Key', createdAt: new Date().toISOString(), updatedAt: new Date().toISOString() }]));
await writeFile(join(data, 'profiles', id, 'codex-home', 'auth.json'), JSON.stringify({ OPENAI_API_KEY: 'inert-hud-fixture' }), { mode: 0o600 });
const env = { ...process.env, HOME: home, MULTI_CODEX_DATA_DIR: data, CODEX_HOME: globalHome, XDG_CONFIG_HOME: join(root, 'config'), XDG_DATA_HOME: join(root, 'xdg-data'), XDG_CACHE_HOME: join(root, 'cache') };
for (const key of Object.keys(env)) if (key.startsWith('OPENAI_') || (key.startsWith('CODEX_') && key !== 'CODEX_HOME') || ['NODE_OPTIONS', 'ELECTRON_RUN_AS_NODE', 'APPIMAGE'].includes(key)) delete env[key];
const app = spawn(process.env.MULTI_CODEX_TEST_LAUNCHER || resolve('src-tauri/target/debug/multi-codex-desktop'), [], { env, stdio: ['ignore', 'pipe', 'pipe'] });
let diagnostic = ''; app.stderr.on('data', chunk => { diagnostic = (diagnostic + chunk).slice(-3000); });
try {
  const main = await until(async () => (await clients()).find(w => w.pid === app.pid && w.title === 'Multi Codex'), w => w && w.fullscreen === 1 && !w.floating);
  const hud = await until(async () => (await clients()).find(w => w.pid === app.pid && w.title === 'Multi Codex Orbit'), w => w?.mapped && w.floating && w.pinned && w.size[1] < 150);
  assert.equal(hud.fullscreen, 0); assert.ok(hud.size[0] < 400); assert.ok(hud.size[1] < 150);
  const orbit = async () => (await clients()).find(w => w.pid === app.pid && w.title === 'Multi Codex Orbit');
  const lua = script => exec('hyprctl', ['eval', script]).then(({stdout}) => assert.equal(stdout.trim(), 'ok'));
  const pointer = (x, y) => lua(`hl.dispatch(hl.dsp.cursor.move({x=${x},y=${y}}))`);
  const compact = [...hud.size];
  console.log('fixture: fixed bounds');
  await new Promise(r => setTimeout(r, 1200));
  const resizeStarted = Date.now();
  await lua(`local w=hl.get_window('address:${hud.address}'); assert(w); hl.dispatch(hl.dsp.window.resize({window=w,x=60,y=30,relative=false}))`);
  await until(orbit, w => w?.size[0] === compact[0] && w.size[1] === compact[1]);
  assert.ok(Date.now() - resizeStarted < 2000, 'a forced compositor resize must restore fixed bounds promptly');
  for (const edge of ['top', 'bottom']) {
    console.log(`fixture: ${edge} hover`);
    await pointer(0, 0);
    await until(orbit, w => w?.size[1] === compact[1]);
    const current = await orbit();
    if (edge === 'bottom') {
      const monitor = JSON.parse((await exec('hyprctl', ['-j', 'monitors'])).stdout).find(m => m.id === current.monitor);
      const bottom = monitor.y + monitor.height / monitor.scale - compact[1] - 4;
      await lua(`local w=hl.get_window('address:${current.address}'); assert(w); hl.dispatch(hl.dsp.window.move({window=w,x=${current.at[0]},y=${bottom},relative=false}))`);
      await until(async () => JSON.parse(await readFile(join(data, 'hud/settings.json'), 'utf8').catch(() => '{}')), s => s.y === bottom);
    }
    const anchored = await orbit();
    await pointer(anchored.at[0] + anchored.size[0] - 76, anchored.at[1] + compact[1] / 2);
    const expanded = await until(orbit, w => w?.size[1] > compact[1] + 100);
    for (let i = 0; i < 18; i++) {
      await new Promise(r => setTimeout(r, 90));
      const stable = await orbit();
      assert.deepEqual(stable.size, expanded.size, `${edge} hover must not flicker between sizes`);
      assert.deepEqual(stable.at, expanded.at, `${edge} hover must not jump`);
      assert.equal(stable.size[0], compact[0]);
    }
    if (edge === 'bottom') assert.equal(expanded.at[1] + expanded.size[1], anchored.at[1] + compact[1]);
    if (process.env.MULTI_CODEX_TEST_HUD_SCREENSHOT) await exec('grim', ['-g', `${expanded.at[0]},${expanded.at[1]} ${expanded.size[0]}x${expanded.size[1]}`, `${process.env.MULTI_CODEX_TEST_HUD_SCREENSHOT}-${edge}.png`]);
  }
  await pointer(0, 0);
  await until(orbit, w => w?.size[1] === compact[1]);
  await exec('busctl', ['--user', 'call', 'com.multicodex.Hud', '/com/multicodex/Hud', 'com.multicodex.Hud1', 'Toggle']);
  await until(clients, windows => !windows.some(w => w.pid === app.pid && w.title === 'Multi Codex Orbit' && w.mapped));
  await exec('busctl', ['--user', 'call', 'com.multicodex.Hud', '/com/multicodex/Hud', 'com.multicodex.Hud1', 'Toggle']);
  await until(clients, windows => windows.some(w => w.pid === app.pid && w.title === 'Multi Codex Orbit' && w.mapped));
  await exec('hyprctl', ['eval', `local w=hl.get_window('address:${main.address}'); assert(w); hl.dispatch(hl.dsp.window.fullscreen_state({internal=2,client=2,window=w}))`]);
  await until(clients, windows => !windows.some(w => w.pid === app.pid && w.title === 'Multi Codex Orbit' && w.mapped));
  await exec('hyprctl', ['eval', `local w=hl.get_window('address:${main.address}'); assert(w); hl.dispatch(hl.dsp.window.fullscreen_state({internal=0,client=0,window=w}))`]);
  await until(clients, windows => windows.some(w => w.pid === app.pid && w.title === 'Multi Codex Orbit' && w.mapped));
  await exec('hyprctl', ['eval', `local w=hl.get_window('address:${main.address}'); assert(w); hl.dispatch(hl.dsp.window.close({window=w}))`]);
  await until(clients, windows => !windows.some(w => w.pid === app.pid && w.title === 'Multi Codex' && w.mapped));
  assert.equal(app.exitCode, null);
  assert.ok((await clients()).some(w => w.pid === app.pid && w.title === 'Multi Codex Orbit' && w.mapped));
  console.log('Verified fixed dimensions, repeated stable top/bottom hover, upwards expansion, size/stacking, hide toggle, fullscreen hide/restore, and close-to-tray; saved accounts untouched.');
} catch (error) {
  const windows = (await clients()).filter(w => w.pid === app.pid);
  const hud = windows.find(w => w.title === 'Multi Codex Orbit');
  if (hud && process.env.MULTI_CODEX_TEST_HUD_SCREENSHOT) await exec('grim', ['-g', `${hud.at[0]},${hud.at[1]} ${hud.size[0]}x${hud.size[1]}`, process.env.MULTI_CODEX_TEST_HUD_SCREENSHOT]).catch(() => {});
  throw new Error(`${error.message}\n${diagnostic}\n${JSON.stringify(windows.map(w => ({title:w.title,at:w.at,size:w.size,pinned:w.pinned,floating:w.floating,fullscreen:w.fullscreen})))}`);
}
finally {
  if (app.exitCode === null) { const exited = once(app, 'exit'); app.kill('SIGTERM'); await exited; }
  // The installed launcher may own its own shortcut on this compositor.
  // Fixture cleanup must never remove another process's runtime binding.
  if (before.address && (await clients()).some(w => w.address === before.address)) await exec('hyprctl', ['eval', `local w=hl.get_window('address:${before.address}'); if w then hl.dispatch(hl.dsp.focus({window=w})) end`]).catch(() => {});
  await rm(root, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 });
}

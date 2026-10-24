// Native layer-shell acceptance. Disposable accounts and a separate session bus.
import assert from 'node:assert/strict';
import { spawn, execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { mkdtemp, mkdir, writeFile, readFile, rm, access } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { randomUUID, createHash } from 'node:crypto';
import { once } from 'node:events';
import { createReadStream } from 'node:fs';
const exec = promisify(execFile);
assert.equal(process.env.MULTI_CODEX_TEST_HUD_ISOLATED_BUS, '1', 'Run inside dbus-run-session');
const json = async name => JSON.parse((await exec('hyprctl', ['-j', name])).stdout);
const lua = async code => assert.equal((await exec('hyprctl', ['eval', code])).stdout.trim(), 'ok');
const pointer = (x, y) => lua(`hl.dispatch(hl.dsp.cursor.move({x=${x},y=${y}}))`);
async function until(read, check, label) {
  const deadline = Date.now() + 15000; let last;
  while (Date.now() < deadline) { last = await read(); if (check(last)) return last; await new Promise(r => setTimeout(r, 100)); }
  throw new Error(`${label} timed out: ${JSON.stringify(last)}`);
}
const root = await mkdtemp(join(tmpdir(), 'mc-orbit-layer-'));
const original = await json('activewindow');
const monitor = (await json('monitors')).find(m => m.focused);
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const launcher = process.env.MULTI_CODEX_TEST_LAUNCHER || resolve('src-tauri/target/debug/multi-codex-desktop');
async function artifactDigest() { const hash = createHash('sha256'); for await (const bytes of createReadStream(launcher)) hash.update(bytes); return hash.digest('hex'); }
const artifactSha256 = await artifactDigest();
let app; let input;
const inputSocket = join(root, 'pointer.sock');
input = spawn('ydotoold', ['--socket-path', inputSocket], { stdio: 'ignore' });
await until(async () => access(inputSocket).then(() => true).catch(() => false), Boolean, 'disposable input device');
const button = state => exec('ydotool', ['click', state ? '0x40' : '0x80'], { env: { ...process.env, YDOTOOL_SOCKET: inputSocket } });
try {
  for (const edge of ['top', 'bottom']) {
    const home = join(root, edge); const data = join(home, 'data'); const global = join(home, 'global');
    const id = randomUUID(); const account = join(data, 'profiles', id, 'codex-home');
    await mkdir(account, { recursive: true }); await mkdir(global, { recursive: true }); await mkdir(join(data, 'hud'));
    const auth = Buffer.from(JSON.stringify({ OPENAI_API_KEY: 'inert-layer-fixture' }));
    const catalog = Buffer.from(JSON.stringify([{ id, name: 'Disposable Orbiter', authMode: 'API Key', createdAt: new Date().toISOString(), updatedAt: new Date().toISOString() }]));
    await writeFile(join(account, 'auth.json'), auth, { mode: 0o600 }); await writeFile(join(data, 'profiles.json'), catalog);
    const y = edge === 'top' ? monitor.y + 72 : monitor.y + monitor.height / monitor.scale - 92;
    await writeFile(join(data, 'hud/settings.json'), JSON.stringify({ enabled: true, size: 56, opacity: 0.4, monitor: null, x: monitor.x + monitor.width / monitor.scale - 360, y, onlyOpen: false, animation: true, reducedMotion: false, sound: false }));
    const env = { ...process.env, HOME: home, CODEX_HOME: global, MULTI_CODEX_DATA_DIR: data, XDG_CONFIG_HOME: join(home, 'config'), XDG_DATA_HOME: join(home, 'xdg-data'), XDG_CACHE_HOME: join(home, 'cache'), NO_AT_BRIDGE: '1', GDK_BACKEND: 'wayland' };
    for (const k of Object.keys(env)) if (/^(OPENAI_|CODEX_)/.test(k) && k !== 'CODEX_HOME') delete env[k];
    app = spawn(launcher, [], { env, stdio: ['ignore', 'ignore', 'pipe'] });
    let errors = ''; app.stderr.on('data', b => { errors = (errors + b).slice(-5000); process.stderr.write(b); });
    const layer = async () => Object.values(await json('layers')).flatMap(m => Object.values(m.levels).flat()).find(l => l.pid === app.pid && l.namespace === 'multi-codex-orbit');
    const initial = await until(layer, l => l && l.h === 88 && l.w === 340, `${edge} layer creation`).catch(async e => { throw new Error(`${e.message}; exit=${app.exitCode}; stderr=${errors}; layers=${JSON.stringify(await json('layers'))}`); });
    assert.equal((await json('clients')).some(w => w.pid === app.pid && w.title === 'Multi Codex Orbit'), false, 'Orbit must not be a managed application window');
    const main = await until(async () => (await json('clients')).find(w => w.pid === app.pid && w.title === 'Multi Codex'), Boolean, 'fixture main');
    // GTK maps the layer before the first WebKit/React layout is ready.
    await new Promise(r => setTimeout(r, 4000));
    if (process.env.MULTI_CODEX_TEST_HUD_SCREENSHOT) await exec('grim', ['-g', `${initial.x},${initial.y} ${initial.w}x${initial.h}`, `${process.env.MULTI_CODEX_TEST_HUD_SCREENSHOT}-${edge}-compact.png`]);
    await pointer(monitor.x + 100, monitor.y + 300);
    // A normal Wayland launch may correctly preserve the previous app's focus.
    // Establish this fixture's focus before checking that hover preserves it.
    await lua(`local w=hl.get_window('address:${main.address}'); assert(w); hl.dispatch(hl.dsp.focus({window=w}))`);
    await until(() => json('activewindow'), w => w.pid === main.pid, `${edge} fixture keyboard focus`);
    await pointer(initial.x + initial.w - 76, edge === 'top' ? initial.y + 44 : initial.y + initial.h - 44);
    const expanded = await until(layer, l => l?.h > initial.h + 100, `${edge} hover expansion`);
    for (let i = 0; i < 24; i++) {
      await new Promise(r => setTimeout(r, 80));
      const current = await layer();
      assert.deepEqual([current.x, current.y, current.w, current.h], [expanded.x, expanded.y, expanded.w, expanded.h], `${edge} overlay hover must stay stable`);
      assert.equal((await json('activewindow')).pid, main.pid, 'hover must not take keyboard focus');
    }
    if (edge === 'top') assert.equal(expanded.y, initial.y);
    else assert.equal(expanded.y + expanded.h, initial.y + initial.h);
    if (process.env.MULTI_CODEX_TEST_HUD_SCREENSHOT) await exec('grim', ['-g', `${expanded.x},${expanded.y} ${expanded.w}x${expanded.h}`, `${process.env.MULTI_CODEX_TEST_HUD_SCREENSHOT}-${edge}.png`]);
    await pointer(monitor.x + 100, monitor.y + 300);
    await until(layer, l => l?.h === initial.h, `${edge} hover dismissal`);
    // Press the actual drag handle. Move through the compositor and release;
    // persisted position and the fixed native extent must agree afterwards.
    const dragX = initial.x + initial.w - 114, dragY = initial.y + 44;
    await pointer(dragX, dragY); await button(true);
    await new Promise(r => setTimeout(r, 180));
    const dx = -70, dy = edge === 'top' ? 24 : -24;
    await pointer(dragX + dx, dragY + dy);
    await new Promise(r => setTimeout(r, 300)); await button(false);
    const moved = await until(layer, l => l?.x === initial.x + dx && l?.y === initial.y + dy, `${edge} actual pointer drag`);
    assert.equal(moved.w, initial.w); assert.equal(moved.h, initial.h);
    const saved = await until(async () => JSON.parse(await readFile(join(data, 'hud/settings.json'), 'utf8')), settings => settings.x === moved.x && settings.y === moved.y, `${edge} drag release persists position`);
    assert.equal(saved.x, moved.x); assert.equal(saved.y, moved.y);
    for (let i = 0; i < 6; i++) {
      await exec('busctl', ['--user', 'call', 'com.multicodex.Hud', '/com/multicodex/Hud', 'com.multicodex.Hud1', 'Toggle']);
      await until(layer, l => !l, 'hide overlay');
      await exec('busctl', ['--user', 'call', 'com.multicodex.Hud', '/com/multicodex/Hud', 'com.multicodex.Hud1', 'Toggle']);
      await until(layer, Boolean, 'restore overlay');
    }
    await lua(`local w=hl.get_window('address:${main.address}'); assert(w); hl.dispatch(hl.dsp.window.fullscreen_state({internal=2,client=2,window=w}))`);
    await until(layer, l => !l, 'fullscreen hides overlay');
    await lua(`local w=hl.get_window('address:${main.address}'); assert(w); hl.dispatch(hl.dsp.window.fullscreen_state({internal=0,client=0,window=w}))`);
    await until(layer, Boolean, 'fullscreen restores overlay');
    assert.equal(digest(await readFile(join(account, 'auth.json'))), digest(auth));
    assert.equal(digest(await readFile(join(data, 'profiles.json'))), digest(catalog));
    console.log(`PASS: ${edge} layer, fixed bounds, stationary hover, keyboard focus, repeated hide/restore, fullscreen and fixture preservation`);
    const exited = once(app, 'exit'); app.kill('SIGTERM'); await exited; app = null;
  }
  assert.equal(await artifactDigest(), artifactSha256, 'The tested native launcher changed during acceptance');
  if (process.env.MULTI_CODEX_TEST_HUD_SCREENSHOT) await writeFile(`${process.env.MULTI_CODEX_TEST_HUD_SCREENSHOT}-result.json`, JSON.stringify({ passed: true, artifactSha256, topAndBottom: true, actualDrag: true, hideRestoreCyclesPerPosition: 6, fixtureLoginsAndCatalogUnchanged: true }));
} finally {
  try { await button(false); } catch {}
  if (input?.exitCode === null) { const ended = once(input, 'exit'); input.kill('SIGTERM'); await ended; }
  if (app?.exitCode === null) { const exited = once(app, 'exit'); app.kill('SIGTERM'); await exited; }
  if (original.address && (await json('clients')).some(w => w.address === original.address)) await lua(`local w=hl.get_window('address:${original.address}'); if w then hl.dispatch(hl.dsp.focus({window=w})) end`);
  await rm(root, { recursive: true, force: true });
}

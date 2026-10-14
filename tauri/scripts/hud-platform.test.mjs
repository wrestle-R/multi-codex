import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import vm from 'node:vm';
const kde = await readFile(new URL('../../cli/crates/core/assets/kde-bridge.js', import.meta.url), 'utf8');
test('bundled KDE integration matches the native adapter source', async () => {
  assert.equal(await readFile(new URL('../platform/kde/bridge.js', import.meta.url), 'utf8'), kde);
});
test('KDE configures only the HUD and hides for visible fullscreen on its chosen output', () => {
  const output = { geometry: { x: 0, y: 0, width: 1200, height: 900 } }; const other = { geometry: { x: 1200, y: 0, width: 1200, height: 900 } };
  const desktop = { id: 'one' }; const hud = { normalWindow: true, pid: 7, caption: 'Multi Codex Orbit', frameGeometry: { x: 10, y: 20 }, onAllDesktops: false };
  const app = { normalWindow: true, pid: 8, caption: 'Other app', fullScreen: true, output, desktops: [desktop] };
  const workspace = { screens: [output, other], windowList: () => [hud, app], currentDesktop: desktop }; let reply;
  const run = () => { vm.runInNewContext(kde, { workspace, request: { action: 'hud', pid: 7, x: 200, y: 200, rectangle: { x: 12, y: 34, width: 200, height: 88 } }, callDBus: (...args) => { reply = JSON.parse(args.at(-1)); } }); return reply; };
  assert.equal(run().fullscreen, true); assert.equal(hud.keepAbove, true); assert.equal(hud.onAllDesktops, true); assert.equal(hud.frameGeometry.y, 34); assert.equal(app.keepAbove, undefined);
  app.minimized = true; assert.equal(run().fullscreen, false);
  app.minimized = false; app.output = other; assert.equal(run().fullscreen, false);
  app.output = output; app.desktops = [{ id: 'two' }]; assert.equal(run().fullscreen, false);
});
test('GNOME uses the selected monitor and never moves another application', async () => {
  let source = await readFile(new URL('../platform/gnome/multicodex-desktops@multicodex.desktop/extension.js', import.meta.url), 'utf8');
  source = source.replace(/^import .*;\n/gm, '').replace('export default class MultiCodexDesktops', 'globalThis.Bridge = class MultiCodexDesktops');
  let geometry = { x: 12, y: 34, width: 100, height: 88 }; let above = false; let sticky = false; let monitor;
  const window = { get_window_type: () => 1, is_override_redirect: () => false, get_pid: () => 7, get_title: () => 'Multi Codex Orbit', make_above: () => { above = true; }, stick: () => { sticky = true; }, move_resize_frame: (_, x, y, width, height) => { geometry = { x, y, width, height }; }, get_frame_rect: () => geometry };
  const context = { Extension: class {}, Meta: { WindowType: { NORMAL: 1 }, Rectangle: class { constructor(value) { Object.assign(this, value); } } }, global: { display: { list_all_windows: () => [window], get_monitor_index_for_rect: rectangle => { monitor = rectangle.x; return 2; }, get_monitor_in_fullscreen: index => index === 2 } } };
  vm.runInNewContext(source, context);
  const state = JSON.parse(new context.Bridge().Hud(JSON.stringify({ pid: 7, x: 1700, y: 300, rectangle: { x: 1600, y: 20, width: 200, height: 88 } })));
  assert.equal(state.fullscreen, true); assert.equal(monitor, 1700); assert.equal(above && sticky, true); assert.equal(state.position.x, 1600);
});

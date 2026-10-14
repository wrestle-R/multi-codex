import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';
import Shell from 'gi://Shell';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

const XML = `<node><interface name="com.multicodex.Desktops1">
<method name="Hud"><arg type="s" direction="in"/><arg type="s" direction="out"/></method>
<method name="Inventory"><arg type="s" direction="out"/></method>
<method name="Move"><arg type="s" direction="in"/><arg type="s" direction="in"/><arg type="s" direction="out"/></method>
</interface></node>`;

export default class MultiCodexDesktops extends Extension {
    enable() {
        this._ids = new WeakMap();
        this._shortcut = global.display.grab_accelerator('<Super><Shift>h', Meta.KeyBindingFlags.NONE);
        if (this._shortcut) {
            Main.wm.allowKeybinding(Meta.external_binding_name_for_action(this._shortcut), Shell.ActionMode.ALL);
            this._shortcutSignal = global.display.connect('accelerator-activated', (_display, action) => {
                if (action === this._shortcut) Gio.DBus.session.call('com.multicodex.Hud', '/com/multicodex/Hud', 'com.multicodex.Hud1', 'Toggle', null, null, Gio.DBusCallFlags.NONE, 1000, null, null);
            });
        }
        this._bridge = Gio.DBusExportedObject.wrapJSObject(XML, this);
        this._bridge.export(Gio.DBus.session, '/com/multicodex/Desktops');
        this._owner = Gio.bus_own_name_on_connection(Gio.DBus.session, 'com.multicodex.Desktops', Gio.BusNameOwnerFlags.NONE, null, null);
    }
    disable() {
        if (this._shortcutSignal) global.display.disconnect(this._shortcutSignal);
        if (this._shortcut) global.display.ungrab_accelerator(this._shortcut);
        this._shortcutSignal = null;
        this._shortcut = null;
        if (this._owner) Gio.bus_unown_name(this._owner);
        this._owner = null;
        this._bridge?.unexport();
        this._bridge = null;
        this._ids = null;
    }
    _id(workspace) {
        if (!this._ids.has(workspace)) this._ids.set(workspace, `gnome:${GLib.uuid_string_random()}`);
        return this._ids.get(workspace);
    }
    _windows() {
        return global.display.list_all_windows().filter(window => window.get_window_type() === Meta.WindowType.NORMAL && !window.is_override_redirect());
    }
    Hud(raw) {
        const request = JSON.parse(raw);
        const hud = this._windows().find(w => w.get_pid() === request.pid && w.get_title() === 'Multi Codex HUD');
        if (hud) { hud.make_above(); hud.stick(); if (request.rectangle) { const r = request.rectangle; hud.move_resize_frame(false, Math.round(r.x), Math.round(r.y), Math.round(r.width), Math.round(r.height)); } }
        const monitor = global.display.get_monitor_index_for_rect(new Meta.Rectangle({x:Math.round(request.x), y:Math.round(request.y), width:1, height:1}));
        if (monitor < 0) throw new Error('HUD display no longer exists');
        return JSON.stringify({supported:true, fullscreen:global.display.get_monitor_in_fullscreen(monitor), position:hud ? {x:hud.get_frame_rect().x,y:hud.get_frame_rect().y} : null});
    }
    Inventory() {
        const manager = global.workspace_manager;
        const windows = this._windows();
        const desktops = [];
        for (let i = 0; i < manager.n_workspaces; i++) {
            const workspace = manager.get_workspace_by_index(i);
            desktops.push({
                id: this._id(workspace), name: `Desktop ${i + 1}`, monitor: null,
                current: workspace === manager.get_active_workspace(),
                windows: windows.filter(window => window.located_on_workspace(workspace)).map(window => ({
                    id: String(window.get_id()), pid: window.get_pid(),
                    application: window.get_wm_class() || 'Application', title: window.get_title() || 'Untitled window',
                })),
            });
        }
        return JSON.stringify({protocolVersion: 1, capabilities: {backend: 'gnome', enumerateDesktops: true, enumerateWindows: true, moveWindows: true, reason: null}, desktops});
    }
    Move(id, destination) {
        const window = this._windows().find(window => String(window.get_id()) === id);
        if (!window) throw new Error('The selected window no longer exists');
        const manager = global.workspace_manager;
        let target = null;
        for (let i = 0; i < manager.n_workspaces; i++) {
            const workspace = manager.get_workspace_by_index(i);
            if (this._id(workspace) === destination) target = workspace;
        }
        if (!target) throw new Error('The selected desktop no longer exists');
        if (window.is_on_all_workspaces()) throw new Error('Unpin this window before moving it to one desktop');
        window.change_workspace(target);
        target.activate(global.get_current_time());
        window.activate(global.get_current_time());
        return 'ok';
    }
}

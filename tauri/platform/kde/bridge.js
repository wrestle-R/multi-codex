// Protocol 1. Executed on demand through KWin's public scripting API (Plasma 6).
// The host supplies a private callback destination, nonce and optional move request.
(function () {
    try {
        const windows = workspace.windowList().filter(w => w.normalWindow && !w.deleted);
        if (request.action === 'move') {
            const window = windows.find(w => String(w.internalId) === request.window);
            const desktop = workspace.desktops.find(d => `kde:${d.id}` === request.destination);
            if (!window) throw new Error('The selected window no longer exists');
            if (!desktop) throw new Error('The selected desktop no longer exists');
            if (window.onAllDesktops) throw new Error('Unpin this window before moving it to one desktop');
            window.desktops = [desktop];
            workspace.currentDesktop = desktop;
            workspace.activeWindow = window;
        }
        const snapshot = {
            protocolVersion: 1,
            capabilities: {backend: 'kde', enumerateDesktops: true, enumerateWindows: true, moveWindows: true, reason: null},
            desktops: workspace.desktops.map(d => ({
                id: `kde:${d.id}`, name: d.name || `Desktop ${d.x11DesktopNumber}`, monitor: null,
                current: d.id === workspace.currentDesktop.id,
                windows: windows.filter(w => w.onAllDesktops || w.desktops.some(wd => wd.id === d.id)).map(w => ({
                    id: String(w.internalId), pid: w.pid,
                    application: String(w.resourceClass || 'Application'), title: w.caption || 'Untitled window',
                })),
            })),
        };
        callDBus(request.callback, '/com/multicodex/KWinReply', 'com.multicodex.KWinReply1', 'Publish', request.nonce, JSON.stringify(snapshot));
    } catch (error) {
        callDBus(request.callback, '/com/multicodex/KWinReply', 'com.multicodex.KWinReply1', 'Publish', request.nonce, JSON.stringify({error: String(error)}));
    }
})();

# Platform support for the 1.3.1 candidate

This candidate is **not cleared for publication**. The full standard-macOS desktop requirement has not passed its feasibility gate. A successful CI build is not a functional support claim.

| Environment | Desktop inventory / placement | Validation performed |
| --- | --- | --- |
| Hyprland | Native IPC; Lua when available, legacy dispatch otherwise | Live inventory and real VS Code placement on Hyprland 0.56.2; full packaged matrix pending |
| GNOME Shell 45–51 | Bundled protocol-1 Shell extension | Contract/source tests; live GNOME and packaged Wayland/X11 validation pending |
| KDE Plasma 6 | Bundled protocol-1 on-demand KWin script | Inventory, empty desktop, placement and missing destination tested on isolated KWin 6.7.5 Wayland; packaged/X11 validation pending |
| macOS 27 and 26, Apple Silicon | Current-desktop launch; native Spaces controls unavailable | No real-Mac functional validation; release blocked |
| Other Linux desktops | Current-desktop launch, explicit unavailable-control message | No desktop-placement support claim |

Linux candidate packages target x86_64. Distribution families are covered by AppImage, DEB and RPM; this does not imply testing every distribution/version. Intel Macs and Windows are outside the 1.3.1 support promise. Linux session-bus, native libraries and system credential-store dependencies must be present.

## Desktop inventory and placement

The picker lists application windows, not individual browser or editor tabs. Desktop identifiers belong to the current desktop session, not permanent saved destinations. Inventories refresh every two seconds while the picker is idle. GNOME IDs survive workspace renumbering within the enabled extension session. Multiple active desktops on separate Hyprland monitors are all marked current. GNOME and KDE share virtual desktops across displays, so their desktop monitor field is unset.

On Hyprland, the picker includes numeric desktops 1–10, including empty destinations that the compositor creates when used, plus any other existing desktops. GNOME and KDE list the desktops provided by their integrations. Linux cards group windows by application and display locally installed app icons, names and counts. Hovering or focusing a card shows its window titles and monitor information.

Placement identifies a new window using structured profile process arguments. When Electron rewrites Linux process arguments, exact profile database lock files held open by a recognized VS Code process provide the fallback; recognized renderer processes can be traced to that owning process. Ambiguous matches are rejected. The destination is verified after moving the identified window. Pinned windows must be unpinned before placement to a single desktop. “Current desktop” also waits for the new window and verifies its destination when the backend supports placement. The dialog closes and returns to the main page only after successful launch completion.

Every launch uses a unique single-folder `.code-workspace` descriptor under the existing profile's `launch-workspaces/<id>` directory. This prevents VS Code from reusing another window of the same folder despite `--new-window`. The descriptor refers to the original absolute project path and stays available for VS Code session restore. Project files are not modified or relocated. Inherited VS Code CLI routing variables are removed from the child command. [VS Code workspace format](https://code.visualstudio.com/docs/editing/workspaces/multi-root-workspaces#_workspace-file-schema)

Failed placement keeps a retry token in memory for up to 30 minutes. Retry moves the already-opened window; “Keep the opened window” accepts it without launching again. Closing the dialog discards the token and leaves the window open. A normal subsequent Launch intentionally creates a new window. Restarting Multi Codex clears retry state; inspect already-opened windows before launching again.

## GNOME installation

The extension is shipped in `tauri/platform/gnome/multicodex-desktops@multicodex.desktop` and included in packaged resources under `desktop-integrations/gnome`. Install from the source tree:

```bash
mkdir -p "${XDG_DATA_HOME:-$HOME/.local/share}/gnome-shell/extensions/multicodex-desktops@multicodex.desktop"
cp tauri/platform/gnome/multicodex-desktops@multicodex.desktop/{metadata.json,extension.js} \
  "${XDG_DATA_HOME:-$HOME/.local/share}/gnome-shell/extensions/multicodex-desktops@multicodex.desktop/"
```

Log out and back in, then enable it:

```bash
gnome-extensions enable multicodex-desktops@multicodex.desktop
```

Refresh the picker. Its bridge exports only Inventory and Move on `com.multicodex.Desktops1`, protocol version 1. Unsupported bridge versions are rejected. The extension does not read credentials, launch commands, or expose arbitrary evaluation. Like other session-bus desktop controls, it is accessible to applications in the same user's desktop session. Supported Shell version metadata is an installation constraint, not evidence of live validation on every version. [GNOME extension documentation](https://help.gnome.org/system-admin-guide/extensions.html)

## KDE integration

Plasma 6 exposes desktop/window controls through KWin scripting. Multi Codex embeds `tauri/platform/kde/bridge.js`, writes each request into an owner-only temporary script, loads it through KWin's session-bus API, receives a nonce-bound inventory response and unloads the script. No permanent plugin or separate window manager needs installation. If scripting/session-bus access is unavailable, the picker explains that placement is unavailable. [KWin scripting API](https://develop.kde.org/docs/plasma/kwin/api/)

The isolated integration check requires KWin 6, GTK4 Python bindings, and dbus-run-session:

```bash
bash tauri/scripts/test-kde-bridge.sh
```

## macOS feasibility blocker

The required feature is inventory of existing Spaces with other applications' windows, placement of a newly opened VS Code window onto a selected Space, and verification of that destination. Public Core Graphics window listing and AppKit's active-Space notification do not establish that full capability; `NSWindow.moveToActiveSpace` concerns an application's own window. No guessed desktop list, private WindowServer integration, extra window manager, or keyboard macro is substituted in this candidate.

- [Core Graphics window listing](https://developer.apple.com/documentation/coregraphics/cgwindowlistcopywindowinfo(_:_:))
- [AppKit active-Space notification](https://developer.apple.com/documentation/appkit/nsworkspace/activespacedidchangenotification)
- [NSWindow moveToActiveSpace](https://developer.apple.com/documentation/appkit/nswindow/collectionbehavior-swift.struct/movetoactivespace)
- [Apple's current macOS versions](https://support.apple.com/en-us/109033)

A real demonstration on Apple Silicon macOS 27 and 26 remains mandatory before marking either native-Spaces gate passed. If a reliable native implementation cannot satisfy it, the agreed release cannot ship without revising that requirement. Current-desktop launch is an explicitly available behavior, not completion of the Spaces requirement.

## Tools and storage

Managed data remains in the existing platform data directory plus `multi-codex` (normally `$XDG_DATA_HOME/multi-codex` or `~/.local/share/multi-codex` on Linux; `~/Library/Application Support/multi-codex` on macOS). Profiles remain under `profiles/<id>` with separate Codex homes, VS Code user-data directories and extensions. Upgrades do not relocate them.

The global import home uses nonempty inherited `CODEX_HOME`, then a saved global-home setting, then `~/.codex`. Paths must be absolute. Launch settings saves VS Code/Codex executable overrides for custom installations. Overrides are validated as executable files and are passed directly, never evaluated as shell commands. Empty fields restore automatic discovery. Tool overrides apply immediately; changing the global home requires restarting the app. Persisted paths are owner-only; credentials are not stored in executable settings.

Automatic discovery uses PATH, existing extension binaries and platform defaults, including `/Applications`, `~/Applications`, `/opt/homebrew/bin` and `/usr/local/bin` on macOS. Missing tools report a settings action. macOS process discovery uses KERN_PROCARGS2 argument boundaries rather than matching substrings in ps output. Keychain remains the macOS credential store; permission/locked-Keychain behavior still needs real-device validation.

## Release procedure and evidence

The tag workflow builds draft candidates only. The explicit Publish validated candidate workflow downloads the draft's exact assets and checks `docs/releases/v1.3.1-validation.json`, checksums and test evidence before publication.

For every required check, record `status: passed`, exact OS/environment version, tester, timestamp and an evidence reference. Record each tested package's SHA256 in the manifest. Rebuilding or re-signing changes the artifact and requires validation of that resulting package. Do not mark a packaged check passed based on a source test or a five-second process smoke test.

```bash
cd tauri
npm run test:release-gates
npm run check:release-gates -- /absolute/path/to/candidate-assets
```

The second command must currently fail. Remaining blockers are tracked in the manifest. Signed macOS candidates require Developer ID certificate credentials and notarization credentials configured through GitHub repository secrets. [Tauri signing documentation](https://v2.tauri.app/distribute/sign/macos/)

Functional validation on each supported environment must include login/import; credential refresh and persistence across restart; isolated profile launch; minimal PATH/Finder/Dock launch; executable overrides; Unicode/spaced paths; folder access denial; upgrade without data loss; empty/missing desktops; multiple monitors; repeated/concurrent launches; pre-existing profile windows; placement retry without duplicates; permission denial/revocation; and verified destination membership. Record failures with credential-free diagnostics. Real Mac checks must include Keychain denial and Gatekeeper/notarization after installing the DMG.

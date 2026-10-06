# Platform support for v1.3.5

v1.3.5 adds a standalone Codex app launch option to the existing isolated VS Code launcher. It is published at the repository owner's request with the testing recorded below. Authenticated isolation and native startup checks passed; broader package/distribution coverage and Apple signing/notarization remain open. The Mac release requires explicit unsigned-install opt-in and may require first-open approval in macOS Privacy & Security.

| Environment | Launch and desktop behavior | Evidence and remaining limits |
| --- | --- | --- |
| Linux x86_64, Hyprland | VS Code or verified Codex app; optional desktop picker with native inventory and verified placement | Live VS Code placement on Hyprland 0.56.2 and authenticated standalone isolation passed; packaged matrix pending |
| Linux x86_64, GNOME Shell 45–51 | Optional picker through the bundled Shell extension | Contract/source tests passed; live GNOME and packaged Wayland/X11 validation pending |
| Linux x86_64, KDE Plasma 6 | Optional picker through an on-demand KWin script | Isolated KWin 6.7.5 Wayland inventory and placement checks passed; packaged/X11 validation pending |
| Other Linux desktops | Current-desktop launch when placement is unavailable | No desktop-placement support claim; distribution-specific package checks pending |
| Apple Silicon macOS 26 | VS Code or verified Codex app opens on the current desktop; no desktop picker or Spaces control | Authenticated isolation on a [Mac GitHub runner](https://github.com/wrestle-R/multi-codex/actions/runs/37345518314) and [native launch/package CI](https://github.com/wrestle-R/multi-codex/actions/runs/37359106358) passed; [real DMG install/update and native startup/restart](https://github.com/wrestle-R/multi-codex/actions/runs/37422386648) passed with account/chat fixtures; signing/notarization and wider functional checks pending |

Standalone isolation is enabled only for desktop app version `26.930.51102` on Linux x86_64 and macOS arm64. Other versions and architectures fail closed until tested. Linux releases use AppImage, DEB and RPM; their availability does not mean every distribution/version has been tested. macOS 27, Intel Macs and Windows have no v1.3.5 validation claim. Linux requires its usual session bus, native libraries and credential-store dependencies.

## Launch settings and account data

The first-run welcome saves platform and installed-app detection; installed apps are checked again later. When both apps are available, Launch settings offers **VS Code only**, **Codex app only**, or **Both**. **Both** shows two launch buttons. With one usable app, the choice section is hidden and the available target is selected automatically. Existing installations initially keep VS Code; a new installation with both verified apps starts with Both. An unverified standalone version cannot be selected.

**Preferred folder + Browse** uses the built-in folder picker. The saved folder is its starting location on each launch; a different project can be chosen then. On Linux, **Show desktop picker** is on by default and can be turned off. The option does not appear on macOS. Mac launches continue on the current desktop after the folder choice. Executable paths and global Codex home are under Advanced.

Each account keeps its existing `profiles/<id>/codex-home` for both apps. Standalone desktop cookies and app state use a separate `desktop-data` directory for that account; VS Code keeps its separate user-data directory and extensions. The app preserves the normal browser HOME for sign-in links. A desktop sign-out stays signed out on restart and does not silently restore stale credentials from the keyring. Editing or deleting an account and cleaning its cache are blocked while either app is using it. The global auth file is not changed by an isolated launch. [Linux and Mac authenticated isolation evidence](releases/v1.3.5-auth-isolation.json)

Managed data stays in the existing platform data directory plus `multi-codex`: normally `$XDG_DATA_HOME/multi-codex` or `~/.local/share/multi-codex` on Linux, and `~/Library/Application Support/multi-codex` on macOS. Upgrades do not relocate profiles. Global import uses a nonempty inherited `CODEX_HOME`, then the saved global-home setting, then `~/.codex`. Override paths must be absolute executable files and are passed directly, never interpreted as shell commands. Empty executable fields restore automatic discovery; changing the global home requires restarting Multi Codex.

## macOS launch checks

Mac GUI apps can have a restricted PATH. Codex CLI discovery includes VS Code extension binaries in `bin/macos-aarch64/codex`, `bin/macos-x86_64/codex`, and older `darwin-*` layouts. It can also use the CLI bundled with the verified desktop app. The sign-in link and Open browser action use the native macOS browser opener.

VS Code and standalone Electron instances can hit macOS socket path limits with long profile paths. Multi Codex creates short, owner-only runtime aliases under `/tmp/multi-codex-<uid>/` that point to the existing profile data. The aliases are recreated after reboot, recognized during process detection, and removed when a profile is deleted. Inherited editor IPC variables are removed from child launches. Linux keeps its own launch and process-identification paths.

The [authenticated runner](https://github.com/wrestle-R/multi-codex/actions/runs/37345518314) used two different disposable accounts to check cold credential reuse, refresh, logout, peer isolation and signed-out restarts. The [CI run](https://github.com/wrestle-R/multi-codex/actions/runs/37359106358) exercised native Keychain, disposable VS Code startup, the real standalone desktop launch twice with Unicode/spaced private paths, a visible window, and Apple Silicon app/DMG build inspection. The CI smoke uses disposable fixtures; it is separate from the authenticated isolation run and from installing and testing a signed/notarized release DMG.

Multi Codex deliberately offers no Mac Spaces inventory or placement. This matches the requested current-desktop flow; the earlier Spaces requirement is outside v1.3.5 scope.

## Linux desktop inventory and placement

The picker lists application windows, not individual editor tabs. Desktop identifiers belong to the current session. Inventories refresh every two seconds while the picker is idle. On Hyprland it includes numbered desktops 1–10 and other existing desktops; GNOME and KDE use the desktops reported by their integrations. Window cards show local application icons, names, counts and window details.

Placement identifies the new VS Code or Codex app window from its profile process arguments. If Electron rewrites those arguments, exact profile database locks held by a recognized process provide a fallback. Ambiguous matches are rejected. The destination is checked after moving the window. A failed move offers a retry for the already-opened window, avoiding a duplicate launch. Disabling the desktop picker uses current-desktop launch.

VS Code opens a unique single-folder `.code-workspace` descriptor in the profile's `launch-workspaces/<id>` directory. This keeps repeated launches from reusing a window even for the same folder. The descriptor refers to the original project path; project files are not moved. [VS Code workspace format](https://code.visualstudio.com/docs/editing/workspaces/multi-root-workspaces#_workspace-file-schema)

### GNOME installation

The bundled extension is `tauri/platform/gnome/multicodex-desktops@multicodex.desktop`. Install it from source:

```bash
mkdir -p "${XDG_DATA_HOME:-$HOME/.local/share}/gnome-shell/extensions/multicodex-desktops@multicodex.desktop"
cp tauri/platform/gnome/multicodex-desktops@multicodex.desktop/{metadata.json,extension.js} \
  "${XDG_DATA_HOME:-$HOME/.local/share}/gnome-shell/extensions/multicodex-desktops@multicodex.desktop/"
```

Log out and back in, then run `gnome-extensions enable multicodex-desktops@multicodex.desktop` and refresh the picker. Its session-bus bridge exposes Inventory and Move on `com.multicodex.Desktops1`, protocol version 1; unsupported versions are rejected. Supported Shell metadata is an installation constraint, not proof of live validation on each version. [GNOME extension documentation](https://help.gnome.org/system-admin-guide/extensions.html)

### KDE integration

Plasma 6 uses the bundled `tauri/platform/kde/bridge.js`. Each request loads an owner-only temporary KWin script, receives a nonce-bound inventory response, and unloads the script. No permanent plugin is installed. If KWin scripting or the session bus is unavailable, the picker reports that placement cannot be used. The isolated integration check is `bash tauri/scripts/test-kde-bridge.sh`. [KWin scripting API](https://develop.kde.org/docs/plasma/kwin/api/)

## Release validation

The tag workflow initially builds a draft and publishes checksums with the exact assets and matching install/update scripts. v1.3.5 is published at the owner's explicit request with its completed tests and remaining limitations documented. The separate [Publish validated candidate workflow](../.github/workflows/publish-validated.yml) still requires every check in [the validation manifest](releases/v1.3.5-validation.json) for fully validated publication. The manifest records actual outcomes with OS version, tester, time, evidence and SHA-256. Rebuilding, changing or signing an artifact changes what must be validated.

```bash
cd tauri
npm run test:release-gates
npm run check:release-gates -- /absolute/path/to/candidate-assets
```

The second command must fail until the required package evidence exists. Remaining work includes installed-package login/import, credential refresh and persistence, app launch under restricted GUI environments, path and permission cases, upgrade without profile loss, repeated/concurrent launches, and Linux desktop-placement checks on the stated environments. A signed macOS release also needs Developer ID and notarization credentials plus Gatekeeper and Keychain-denial checks on the installed DMG. [Tauri signing documentation](https://v2.tauri.app/distribute/sign/macos/)

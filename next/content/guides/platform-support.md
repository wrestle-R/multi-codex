# Platform support

Multi Codex v1.4 adds native Windows x64 support, a Mac menu-bar account picker, and a persistent maximized launcher. The VS Code extension uses separate manifest-derived packages for Linux x64, macOS arm64/x64, and Windows x64. Release publication requires native platform checks and validation of the exact downloadable packages.

| Environment | Launch and desktop behavior | Evidence and remaining limits |
| --- | --- | --- |
| Linux x86_64, Hyprland | VS Code or verified Codex app; optional desktop picker with native inventory and verified placement | Live VS Code placement on Hyprland 0.56.2 and authenticated standalone isolation passed; packaged matrix pending |
| Linux x86_64, GNOME Shell 45–51 | Optional picker through the bundled Shell extension | Contract/source tests passed; live GNOME and packaged Wayland/X11 validation pending |
| Linux x86_64, KDE Plasma 6 | Optional picker through an on-demand KWin script | Isolated KWin 6.7.5 Wayland inventory and placement checks passed; packaged/X11 validation pending |
| Other Linux desktops | Current-desktop launch when placement is unavailable | No desktop-placement support claim; distribution-specific package checks pending |
| Windows x64 | VS Code, CLI and the verified Codex app launch on the current desktop; no virtual-desktop picker | Native Windows GitHub runner exercises Credential Manager, private ACLs, locking, two-account VS Code/CLI guards, packaged VSIX switching, and exact NSIS install/update/restart. Fixtures exercise local isolation; real OAuth and token rotation require connectivity. |
| Apple Silicon macOS 26 | VS Code or verified Codex app opens on the current desktop; no desktop picker or Spaces control | Authenticated isolation on a [Mac GitHub runner](https://github.com/wrestle-R/multi-codex/actions/runs/37345518314) and [native launch/package CI](https://github.com/wrestle-R/multi-codex/actions/runs/37359106358) passed; [real DMG install/update and native startup/restart](https://github.com/wrestle-R/multi-codex/actions/runs/37563569940) passed with account/chat fixtures; signing/notarization and wider functional checks pending |

Standalone isolation is enabled only for desktop app version `26.930.51102` on Linux x86_64 and macOS arm64, and the official Windows x64 MSIX app build `26.930.61225`. Other versions and architectures fail closed until tested. Linux releases use AppImage, DEB and RPM; their availability does not mean every distribution/version has been tested. The desktop launcher has no macOS 27 or Intel Mac validation claim; the extension separately supports both Mac architectures. Windows support targets native x64; WSL, Remote SSH, containers and Windows virtual-desktop placement are outside this scope. Linux requires its usual session bus, native libraries and credential-store dependencies.

## Launch settings and account data

The first-run welcome saves platform and installed-app detection; installed apps are checked again later. Launch settings offers **VS Code**, **Codex**, **All available** and **CLI only**, disabling unavailable targets. The CLI button appears alongside the selected apps whenever a CLI is available. Existing launch preferences remain in place, and an unverified standalone version cannot be selected.

**Preferred folder + Browse** uses the built-in folder picker. The saved folder is its starting location on each launch; a different project can be chosen then. On Linux, **Show desktop picker** is on by default and can be turned off. The option does not appear on macOS or Windows. Mac and Windows launches continue on the current desktop after the folder choice. Executable paths and global Codex home are under Advanced.

Each account keeps its existing `profiles/<id>/codex-home` for VS Code, standalone and CLI launches. Standalone desktop cookies and app state use a separate `desktop-data` directory for that account; VS Code keeps its separate user-data directory and extensions. The app preserves the normal browser HOME for sign-in links. A desktop sign-out stays signed out on restart and does not silently restore stale credentials from the keyring. Editing or deleting an account and cleaning its cache are blocked while an app or CLI is using it. The global auth file is not changed by an isolated launch. [Linux and Mac authenticated isolation evidence](https://github.com/wrestle-R/multi-codex/blob/main/release/evidence/v1.3.5-auth-isolation.json)

Managed data stays in the existing platform data directory plus `multi-codex`: normally `$XDG_DATA_HOME/multi-codex` or `~/.local/share/multi-codex` on Linux, and `~/Library/Application Support/multi-codex` on macOS, and `%APPDATA%\multi-codex` on Windows. Upgrades do not relocate profiles. Global import uses a nonempty inherited `CODEX_HOME`, then the saved global-home setting, then `~/.codex`. Override paths must be absolute executable files and are passed directly, never interpreted as shell commands. Empty executable fields restore automatic discovery; changing the global home requires restarting Multi Codex.

## Mac menu bar and launcher window

The menu-bar icon lists saved accounts. Each account submenu offers only installed supported targets: **Open VS Code**, **Open Codex**, and **Open CLI**. Launches reuse folder preferences and account safety guards. **Open Multi Codex** restores the maximized main window; **Refresh All Usage** checks all accounts. Closing the window leaves the menu-bar process running. **Quit Multi Codex** exits the launcher and leaves separately launched tools open.

On Hyprland, the launcher monitors its own mapped window and restores maximization after desktop changes or accidental floating restores, without moving it or stealing focus. Other platforms restore maximization when the main window regains focus. The live Linux test covers three desktop switches and repair of the floating screenshot state.

## Windows launch and installation

[Native Windows validation](https://github.com/wrestle-R/multi-codex/actions/runs/37914130058) passed for the exact v1.4.0 installer and v0.2.0 Windows VSIX. [All four packaged extension targets](https://github.com/wrestle-R/multi-codex/actions/runs/37914129972) and [native Mac account menus](https://github.com/wrestle-R/multi-codex/actions/runs/37909325891) have runner evidence. The [release manifest](https://github.com/wrestle-R/multi-codex/blob/main/release/evidence/v1.4.0-validation.json) records package hashes and the remaining validation limits.

Use the website's Windows PowerShell install/update commands or the release's `Multi.Codex_<version>_x64-setup.exe`. Installation is per-user. The scripts select the current release's Windows asset dynamically and verify its SHA-256 before running NSIS. Updates preserve account and conversation data, which remain outside the application directory.

Windows accounts use Credential Manager and protected directories granting access to the current user and SYSTEM. The launcher and extension share `%APPDATA%\multi-codex`. The extension contains a native account helper, an executable startup wrapper and authenticated named pipes. It supports live account switching in local native VS Code without changing PowerShell's persistent execution policy.

VS Code discovery prefers native `Code.exe`; CLI discovery resolves npm's native `codex.exe`. Advanced settings accept absolute `.exe` paths. PowerShell and Windows Terminal launches encode the command as UTF-16, quote literal paths, and clear inherited credential overrides before selecting the account home. Running-profile detection recognizes native editor arguments and the private encoded-command marker, preventing deletion or cache cleanup during use.

Standalone Codex discovery reads the installed **OpenAI.Codex** MSIX location rather than guessing a WindowsApps path. Unverified desktop versions remain disabled. The official package passed native runner checks with two independent app windows and account homes, deletion/cache guards, unchanged peer/global credential files, and two cold starts. Windows runner credentials are inert fixtures; real browser sign-in, credential rotation and cloud permissions were not exercised by that test.

## macOS launch checks

CLI launch opens a private, owner-only `.command` file through Launch Services, without Apple Events automation permission. The file contains paths rather than credentials and removes itself before starting the CLI. Credential overrides are cleared before selecting the account's isolated Codex and SQLite homes. A [native Terminal check](https://github.com/wrestle-R/multi-codex/actions/runs/37562655635) covers spaced/Unicode paths, profile process detection, deletion/cache guards, and unchanged peer/default credentials. Linux supports Konsole, GNOME Terminal, XFCE Terminal or xterm. CLI launch skips the desktop picker on both platforms.

Mac GUI apps can have a restricted PATH. Codex CLI discovery includes VS Code extension binaries in `bin/macos-aarch64/codex`, `bin/macos-x86_64/codex`, and older `darwin-*` layouts. It can also use the CLI bundled with the verified desktop app. The sign-in link and Open browser action use the native macOS browser opener.

VS Code and standalone Electron instances can hit macOS socket path limits with long profile paths. Multi Codex creates short, owner-only runtime aliases under `/tmp/multi-codex-<uid>/` that point to the existing profile data. The aliases are recreated after reboot, recognized during process detection, and removed when a profile is deleted. Inherited editor IPC variables are removed from child launches. Linux keeps its own launch and process-identification paths.

The [authenticated runner](https://github.com/wrestle-R/multi-codex/actions/runs/37345518314) used two different disposable accounts to check cold credential reuse, refresh, logout, peer isolation and signed-out restarts. The [CI run](https://github.com/wrestle-R/multi-codex/actions/runs/37359106358) exercised native Keychain, disposable VS Code startup, the real standalone desktop launch twice with Unicode/spaced private paths, a visible window, and Apple Silicon app/DMG build inspection. The CI smoke uses disposable fixtures; it is separate from the authenticated isolation run and from installing and testing a signed/notarized release DMG.

Multi Codex deliberately offers no Mac Spaces inventory or placement. This matches the requested current-desktop flow; the earlier Spaces requirement is outside the current scope.

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

The tag workflow initially builds a draft and uploads checksums with the exact assets and matching install/update scripts. The [Publish tested release workflow](https://github.com/wrestle-R/multi-codex/blob/main/.github/workflows/publish-validated.yml) requires the platform `releaseChecks` in [the validation manifest](https://github.com/wrestle-R/multi-codex/blob/main/release/evidence/v1.4.0-validation.json): Linux/Mac/Windows builds and installer checks, account isolation, all four packaged extension targets, launcher behavior and the Mac menu. It also downloads and verifies all five app packages, four VSIX files and four installer scripts against the recorded hashes before publishing as the latest stable release. The manifest records actual outcomes with environment, tester, time, evidence and SHA-256. Rebuilding, changing or signing an artifact changes what must be validated.

```bash
cd tauri
npm run test:release-gates
npm run check:release-gates -- /absolute/path/to/candidate-assets
```

Publication requires those passed checks and exact asset bytes. Broader installed-package login/import, permission and Keychain-denial cases, concurrent sessions and the full Linux distribution/desktop matrix remain unfinished. They can be checked separately with `npm run check:release-gates -- --full-validation /absolute/path/to/candidate-assets`; that command currently fails because those tests are incomplete.

Developer ID signing and notarization are optional, including in the broader functional check. The build workflow keeps the `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD` and `APPLE_SIGNING_IDENTITY` secrets available for signing, and `APPLE_ID`, `APPLE_PASSWORD` and `APPLE_TEAM_ID` for notarization. Without them it builds an unsigned release. When configured, signature/notarization verification runs; errors in that configured path are not silently treated as success. [Tauri signing documentation](https://v2.tauri.app/distribute/sign/macos/)

For the unsigned release, use the website installation guide’s explicit Mac installer opt-in. After trying to open the installed app, approve it through **System Settings → Privacy & Security → Open Anyway**, then confirm **Open** if macOS requests approval. The installer never disables Gatekeeper or automatically removes quarantine. [Apple's first-open instructions](https://support.apple.com/en-us/102445)

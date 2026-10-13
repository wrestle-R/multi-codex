# Multi Codex CLI

Manage saved Codex accounts on macOS and Linux using the same local store as the desktop launcher. Account credentials remain separate. Launching another account opens a new terminal; existing terminals keep their current login.

## Install

```sh
npm install --global @russeldp/multi-codex
multi-codex --version
multi-codex doctor
```

Node.js 22.22.2+ is required. The current Linux x64 binary requires glibc 2.39+ (for example Ubuntu 24.04+); musl-based distributions are excluded. The Linux x64 package is tested on the development laptop. macOS ARM64 and Linux ARM64 package definitions are present; publish/install availability and native verification must be checked before describing them as supported releases. Windows and Intel Macs are excluded. The wrapper checks macOS 26+ and selects an exact OS/architecture package.

An official Codex engine must already be installed, either as the Codex CLI or through a compatible official VS Code extension. The CLI does not install a second editor. Linux needs a working unlocked Secret Service credential store; macOS uses Keychain. There is no plaintext credential-store fallback. A graphical session and an installed terminal are required for `launch`.

Before global installation, inspect `command -v multi-codex`. If that name belongs to an existing desktop shim or another tool, select a separate npm prefix and invoke its full command path. Installation must not replace unrelated commands.

## Accounts and launching

```sh
multi-codex accounts list
multi-codex accounts add --name Work
multi-codex accounts add --name Work --no-browser
multi-codex accounts import --name Work --file /private/path/auth.json
multi-codex accounts import-current --name Personal
multi-codex accounts show Work
multi-codex accounts edit Work --name Client --notes "Client work"
multi-codex accounts reconnect Client
multi-codex launch --account Client --target cli --workspace /path/to/project
multi-codex launch --account Client --target cli --terminal kitty --workspace .
multi-codex launch --account Client --target vscode --workspace .
multi-codex launch --account Client --target codex --workspace .
multi-codex accounts remove Client --yes
```

Names are exact and case-sensitive; a full profile UUID also works. Ambiguous names are refused. Import accepts `--stdin` instead of `--file` without putting credentials into command arguments. `import-current` reads the configured default home and leaves it unchanged. Device sign-in prints only the official one-time URL and code to stderr; Ctrl+C cancels and removes owned temporary state.

Account removal deletes managed private account data. Shared conversations remain available to other accounts. Run without `--yes` for an interactive confirmation; scripts require explicit consent.

Running the root command in a terminal opens an account/action menu. Redirected use displays help; use an explicit subcommand for scripts. Terminal rendering uses no ANSI color sequences, so `--no-color` and `NO_COLOR` are safe.

## Usage, settings and diagnostics

```sh
multi-codex usage --account Client
multi-codex usage --all --json
multi-codex usage --all --watch --interval 60
multi-codex credits --account Client
multi-codex status
multi-codex settings get
multi-codex settings set workspace /path/to/projects
multi-codex settings set terminal ghostty
multi-codex settings set launch-target cli
multi-codex terminals list
multi-codex desktops list
multi-codex doctor --json
```

Usage reflects only windows and credits actually returned by the engine. Missing expiry stays null; API-key accounts do not receive invented subscription quotas. Failed refreshes may include a saved result marked `stale`. Bulk failures include individual results and return nonzero. Ctrl+C stops watch mode. Desktop placement is optional: `launch --target vscode --desktop ID` uses verified compositor capabilities, and reports placement failure separately from a started editor.

`--data-dir` overrides `MULTI_CODEX_DATA_DIR`, which otherwise overrides the platform data root. Overrides must be absolute and outside the default Codex home. `--global-codex-home` selects the default import source without moving managed profiles. Inside a managed terminal, its private `CODEX_HOME` (including the VS Code bridge runtime) is recognized as an account home and is not mistaken for the global import source, so account-management and new-account launches keep working. Standard roots are `~/.local/share/multi-codex` on Linux (or `$XDG_DATA_HOME/multi-codex`) and `~/Library/Application Support/multi-codex` on macOS. CLI and desktop share IDs, keyring names and locks.

The supported settings keys are workspace, terminal, launch-target, code-path, codex-path, global-codex-home, hide-desktop-picker and color-theme. `settings reset [KEY] --yes` resets shared settings; it does not remove accounts. `settings set color-theme sage` (or ocean, sand, rose, plum, orange) updates the shared GUI palette preference; the desktop reads it on startup. The CLI itself renders plain terminal output.

## Storage, previews and recovery

```sh
multi-codex storage --json
multi-codex storage cache clear --account Client --yes
multi-codex storage chats preview --account Client --older-than 30d
multi-codex storage chats clear --account Client --older-than 30d --preview-token UUID --yes
multi-codex storage chats preview --shared --all
multi-codex storage chats clear --shared --all --preview-token UUID --yes
multi-codex storage backups list
multi-codex storage backups verify --account Client
multi-codex storage backups restore --account Client --yes
multi-codex storage migration preview --account Client
multi-codex storage migration prepare --account Client
multi-codex storage migration apply --account Client --yes
multi-codex storage migration rollback --account Client --yes
multi-codex storage migration queue
multi-codex storage migration enable --yes
multi-codex storage extensions prepare
multi-codex storage extensions preview-legacy --account Client
multi-codex storage extensions clear-legacy --account Client --token UUID --yes
multi-codex storage extensions preview-generations
multi-codex storage extensions clear-generations --token DIGEST --yes
```

Preview tokens expire after ten minutes. Deletion rechecks history activity and refuses changed previews, busy consumers and unsafe paths. Without `--preview-token`, clear builds and applies a fresh preview of the explicitly selected scope. Private bulk cleanup uses `--all-accounts`; an omitted account never selects everyone. `--shared` affects the shared library for all accounts. Shared history is not a complete shared Codex home or shared authentication database.

The 30-day cutoff is last activity, including archived chats; protected parents retain newer descendants. Cleanup removes local saved history, not cloud conversations or project files. Partial native deletion failures report progress. Cache clearing preserves credentials, settings, skills and installed extensions. Storage totals do not sum overlapping subsets.

Migration retains originals and requires verified snapshots. When a profile is closed, exact recognized stale IPC sockets, temporary Codex helper links and Electron Singleton links and closed UUID bridge-window directories (including private runtime copies) are moved intact into a journaled `migrations/retained-runtime` archive without following or deleting them. Other unusual entries remain refused; active profiles are never normalized. Restore preserves current credentials and displaced newer work; closed clients and durable recovery journals protect transitions. The backups commands inspect the current managed snapshot for each account, not arbitrary external backup archives. Never use real personal history for destructive tests.

## Desktop shortcuts and updates

```sh
multi-codex integration status
multi-codex integration install --desktop-shortcut
multi-codex integration remove --yes
multi-codex update --check
multi-codex update
npm install --global @russeldp/multi-codex@latest
npm uninstall --global @russeldp/multi-codex
```

Portable AppImage shortcut installation requires the actual desktop AppImage source. CLI installation alone does not supply the desktop app. Removing recognized shortcuts leaves executables and profile data intact. Updates query the npm registry and show the npm upgrade command; they never install a desktop updater or silently replace npm-managed files. Uninstall leaves profiles and history on disk.

Platform payloads include the CLI, account helper and managed editor bridge. A pinned Ed25519 key verifies the signed payload manifest, then SHA-256 checks every executable/resource before starting. Tampered or mismatched package versions are refused. No install hook downloads mutable binaries, changes shell startup files or starts a daemon.

## Scripting contract

Every command supports help. `--json` emits schema version 1: `{ "schemaVersion": 1, "ok": true, "data": ... }`, or an error object with `code`, `message` and optional per-account `details`. Diagnostics and device instructions use stderr; credentials and conversation text are never returned. Watch mode emits one JSON object per refresh. `--quiet` suppresses optional progress, not results or errors.

Exit codes: 0 success; 1 operation/partial failure; 2 invalid arguments or ambiguous account; 3 missing local capability; 4 busy/stale preview; 5 authentication/network failure; 130 cancellation. Backend failures can still map to general operation failure when they lack a structured classification.

## Develop and package

All CLI files live under `cli/`. The desktop consumes `cli/crates/core` through a path dependency.

```sh
cargo test --manifest-path cli/Cargo.toml --workspace
cargo build --manifest-path cli/Cargo.toml --release -p multi-codex-cli
node cli/scripts/package.mjs linux-x64
```

Packaging requires `MULTI_CODEX_CLI_SIGNING_KEY_FILE` pointing to a private key outside the repository whose public key matches `cli/npm/root/bin/public-key.mjs`. The packaging script builds the CLI and editor bridge, verifies the signing key, and writes tarballs under ignored `cli/artifacts/`. It does not publish. Publish tested platform packages first and root package last, using matching versions. Never package private notes, credentials, environment files or session data. Native platform gaps must remain documented.

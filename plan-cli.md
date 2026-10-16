# Multi Codex CLI — complete implementation plan

Status: **planning only; no CLI implementation is authorized in this task.**

Decisions confirmed on 12 October 2026:

- Deliver one complete CLI release with all applicable Multi Codex features. Do not describe a limited first version followed by a second version.
- Support Apple Silicon Macs on **macOS 26+**, Linux x64, and Linux ARM64. The Linux ARM64 target is the interpretation of “also arm64” stated back to the owner; verify this target before packaging.
- Selecting another account opens a **new terminal with that account**. It does not change authentication inside an existing terminal conversation.
- Reuse saved accounts and the same local storage as the desktop launcher and VS Code extension.
- Keep default Codex authentication untouched.
- Detailed release documentation belongs on the website. GitHub hosts artifacts and links to website release pages.

## 1. Outcome and scope

A user should be able to install Multi Codex, add and manage accounts, inspect usage, launch workspaces, manage storage, update the tool, and diagnose problems entirely from a terminal. Installing or opening the desktop launcher must not be required.

Example: a user is working as Naigaon, then runs:

```sh
multi-codex launch --account Chriso --target cli --workspace ~/Projects/client
```

Multi Codex opens a new terminal in the selected workspace, running Codex with Chriso's isolated credentials and history. The original Naigaon terminal continues unchanged. Closing either terminal does not delete its saved history.

The CLI manages local accounts and clients. It does not create subscription credits, combine quotas, provide a cloud account-sync service, or automatically rotate accounts when one runs out of usage.

All commands below are **proposed interfaces**, not commands available in the current repository.

## 2. Platform contract

| Platform | Planned package | Requirements and verification |
|---|---|---|
| macOS | Native arm64 CLI archive; installation into a user-owned executable directory | Apple Silicon, macOS 26+, native Keychain access, real Terminal launch, private account directories |
| Linux x64 | Native x86_64 CLI archive; optional distro packages | Document the minimum glibc/distribution baseline after testing; functional Secret Service/keyring connection |
| Linux ARM64 | Native aarch64 CLI archive; optional distro packages | Native ARM64 runner coverage, compatible official Codex engine, terminal discovery and keyring verification |

Windows and Intel Macs are outside this requested CLI release. Desktop Windows support is separate and must continue working when shared code is reorganized.

The CLI must not depend on Tauri, WebKit, a webview, or the desktop app for basic account and usage operations. Commands that launch another graphical program naturally need a graphical session. On a headless host, report this requirement rather than claiming a terminal window was opened.

Linux ARM64 support is a real release requirement, not just a cross-compilation target. Do not publish an ARM64 package until account persistence, usage, terminal launches and installation have passed native checks. Standalone Codex Desktop launching remains available only when a compatible, verified desktop client exists for that platform. Unsupported client capabilities must have an explicit reason; account management and Codex CLI launching should remain usable.

## 3. Feature parity

| Current desktop capability | CLI equivalent |
|---|---|
| Saved account list and account tier | `accounts list`, `accounts show` |
| Browser/device-code sign-in | `accounts add`, `accounts reconnect` |
| Import current authentication | `accounts import-current` |
| Paste/import authentication JSON | `accounts import --file`; safe stdin support |
| Rename, notes and profile deletion | `accounts edit`, `accounts remove` |
| Per-account usage and Refresh all | `usage --account`, `usage --all`, `usage --watch` |
| Usage windows, reset countdowns, credits and expiry | Usage table plus `credits --account` |
| Launch VS Code, Codex Desktop and Codex CLI | `launch --target vscode\|codex\|cli` |
| Workspace picker/preferred folder | `--workspace`, terminal folder picker and `settings set workspace` |
| Preferred CLI terminal | `terminals list`, `settings set terminal` |
| Linux desktop placement | `desktops list`, optional `launch --desktop` where verified |
| Running/error state | `status`, `accounts show`, `doctor` |
| Profile storage and removable caches | `storage`, `storage cache clear` |
| All-chat and 30-day chat deletion | `storage chats preview`, `storage chats clear` |
| Update installation | `update` with signed packages and progress |
| Desktop-launcher integration | `integration status/install/remove` when the desktop package is present |
| Colour preferences | `settings get/set`, readable terminal colour controls |
| Shared account storage with VS Code extension | Reuse the existing metadata, credential locks and leases |

GUI-only visual elements are represented by appropriate terminal controls. A graphical theme picker does not require recreating a graphical page inside the terminal; its stored preference can still be inspected or changed.

## 4. Command structure

Use one discoverable executable, `multi-codex`. Every command and subcommand supports `--help`. The root help starts with common account, launch, usage and storage commands.

```text
multi-codex
  accounts list | show | add | import | import-current | edit | reconnect | remove
  launch
  usage
  credits
  status
  storage
    cache clear
    chats preview | clear
  terminals list
  desktops list
  settings get | set | reset
  integration status | install | remove
  update
  doctor
  version
```

Running the root command with an interactive terminal should open a compact account selector with a clear action menu. In scripts or redirected output, print help or structured output; never hang waiting for an invisible picker.

### Global options

| Option | Meaning |
|---|---|
| `--json` | Stable machine-readable result on stdout; diagnostics on stderr |
| `--no-color` | Plain output; also respect `NO_COLOR` and non-TTY output |
| `--quiet` | Suppress progress without hiding failures |
| `--data-dir PATH` | Explicit shared account-store override, validated as an absolute path |
| `--global-codex-home PATH` | Source default Codex home, distinct from managed account homes |
| `--version` | CLI version and concise build information |

Resolve accounts by exact name or complete profile UUID. Reject ambiguous names rather than selecting the first match. Names containing spaces must work as normal quoted shell arguments.

## 5. Account management

```sh
multi-codex accounts list
multi-codex accounts show Naigaon
multi-codex accounts add --name Work
multi-codex accounts import-current --name Personal
multi-codex accounts import --name Work --file /secure/location/auth.json
multi-codex accounts edit Work --name Client --notes "Client projects"
multi-codex accounts reconnect Client
multi-codex accounts remove Client
```

- Device login displays the expected one-time link and code, opens the browser when available, and can operate with `--no-browser`. Cancellation removes temporary login state.
- Imported credentials are validated by the official installed Codex engine. The default authentication file is read, never replaced.
- Account removal explains that it removes the managed account data, including local conversations. It requires confirmation, or an explicit `--yes` in automation.
- Reconnection preserves the account name, notes and existing isolated data. Refuse credential replacement while the account is leased or running.
- Duplicate names, missing credentials, rejected login, unavailable keyring and token-refresh failures have separate actionable messages.
- Never put credentials in shell arguments, command history, JSON output, debug logs or telemetry. File paths and private stdin are the supported import surfaces.

## 6. Launching: new terminal, isolated account

```sh
multi-codex launch --account Naigaon --target cli --workspace ~/Projects/app
multi-codex launch --account Chriso --target cli --terminal ghostty --workspace .
multi-codex launch --account Chriso --target vscode --workspace .
multi-codex launch --account Chriso --target codex --workspace .
```

The default target follows the shared launch preference; absent a preference, an interactive invocation asks which installed client to use. An explicit target never silently changes to a different client.

Required launch sequence:

1. Resolve the selected account and canonical workspace directory.
2. Inspect the account's current credential and required locks.
3. Resolve the installed official engine or application and verify relevant compatibility.
4. Build a process invocation using an argument array, never an interpolated shell command.
5. Strip inherited credential/provider/home overrides that could select the wrong account.
6. Set the account's `CODEX_HOME` and matching SQLite home. For Desktop and VS Code, also isolate app user data.
7. Open the requested new terminal or application and verify the launch sufficiently to report a truthful result.

Terminal selection reuses the current discovery rules: native Terminal on Mac; installed supported terminals on Linux. Respect the user's explicit preference and report when it is unavailable. The new terminal should keep an understandable failure message visible if Codex exits before starting normally.

Multiple accounts can run concurrently. Multiple workspaces for one account reuse that account's managed home. Do not describe this as one fresh history directory per workspace.

Existing terminal sessions are never reauthenticated by this command. Continuing the original account's conversation under a different account is outside this confirmed interaction model.

## 7. Usage and reset credits

```sh
multi-codex usage --account Naigaon
multi-codex usage --all
multi-codex usage --all --watch
multi-codex usage --all --json
multi-codex credits --account Naigaon
```

- Render whichever usage windows the backend actually returns: five-hour, weekly or monthly. Include remaining percentages, exact reset timestamps, local countdowns and checked-at time.
- Show reset-credit count, status and expiry when returned. Missing expiry is “not reported,” never a fabricated date.
- API-key accounts do not receive invented ChatGPT quota windows.
- Usage reads may refresh credentials. Reuse the shared credential-operation lock and persist refreshed tokens consistently with the desktop app and extension.
- `--watch` uses a documented interval and stops cleanly on Ctrl+C. Refresh-all concurrency is bounded; one account failure does not suppress the successful accounts.
- Display stale saved results explicitly after an offline refresh failure. JSON preserves per-account success, failure and freshness information.
- Expiring-credit reminders appear during interactive watch/status use. Do not install a background daemon or send notifications merely because the CLI was installed.

## 8. Storage and local conversations

```sh
multi-codex storage
multi-codex storage --account Naigaon --json
multi-codex storage cache clear --account Naigaon
multi-codex storage chats preview --account Naigaon --older-than 30d
multi-codex storage chats clear --account Naigaon --older-than 30d
multi-codex storage chats clear --account Naigaon --all
```

Storage output separates installed extensions, VS Code data, Codex data and Desktop data. Show local transcript size as a subset of Codex data; cache size is also a subset of total usage. Never add overlapping categories to claim a larger total.

The current launcher has no fixed storage cap. Manual cleanup reclaims selected data but does not impose a limit. Do not advertise the 30-day action as automatic retention.

Chat-cleanup requirements:

- Include active-history and archived local chats and relevant subagent descendants.
- “Older than 30 days” means last activity, not just creation date or date-folder name. Apply a strict cutoff and preserve uncertain/newer activity.
- Show selected count, transcript bytes and protected older parents before deletion.
- Protect an old parent when the native cascade would remove newer subagent chats.
- Use Codex's supported deletion operation to maintain its history metadata. Keep prompt history and title indexes consistent.
- Use a short-lived preview token and revalidate activity before applying it. If history changes, require a new preview instead of expanding deletion silently.
- Block cleanup while an account is running or leased. Use shared locks to prevent a new launch or credential operation racing with deletion.
- Refuse symlink escapes, foreign-profile paths and attempts to operate on the default/global Codex home.
- Preserve authentication, settings, skills, plugins and installed extensions.
- Require confirmation for deletion. Non-interactive commands need explicit account, scope and `--yes`; `--json` is not consent.
- Report partial progress honestly if native deletion fails after removing some chats. Do not imply an all-or-nothing transaction that the underlying operation cannot provide.
- This deletes local saved history; it does not delete cloud conversations or project source files.

For bulk cleanup across accounts, require an explicit `--all-accounts`, show a per-account preview, and skip/refuse busy accounts with a clear result. Never infer “all accounts” from an omitted account argument.

## 9. Settings, diagnostics and desktop integration

```sh
multi-codex settings get
multi-codex settings set workspace ~/Projects
multi-codex settings set terminal ghostty
multi-codex settings set launch-target cli
multi-codex terminals list
multi-codex desktops list
multi-codex status
multi-codex doctor
```

Use the existing shared settings format and validate paths through the same services as the GUI. Existing profiles remain at their current locations when changing the global home preference. Explain which environment override takes precedence.

`doctor` reports platform/architecture, official engine discovery, available terminals/apps, keyring connectivity, readable profile store, permissions, conflicting settings and update support. It never prints auth-file contents, tokens, private conversation text or unrestricted environment dumps.

Desktop placement uses the existing capability reporting. On an unsupported compositor or Mac Spaces configuration, show a reason instead of inventing a desktop list. Account operations do not depend on this optional capability.

Integration commands should report “desktop app not installed” when appropriate. Removing a launcher shortcut must not uninstall the CLI or delete account data.

## 10. Updates and installation

```sh
multi-codex update
multi-codex update --check
```

`update` is the deliberate action to check, download, verify and install the appropriate CLI package. Show real progress and the resulting installed version. `--check` only reports availability. Avoid a second installation prompt for this explicitly requested update unless the OS requires authentication.

- Publish signed CLI artifacts for the three confirmed targets. Desktop updater artifacts are separate and cannot substitute for a CLI ARM64 binary.
- Verify a pinned signing key before replacement. A TLS connection or a checksum alone is not a substitute for the payload signature.
- Stage in the installed binary's filesystem and replace atomically with rollback when possible. Reject unsupported/system-managed destinations with package-manager guidance.
- Never overwrite an unrelated executable or a desktop-launcher shim with the same name. Detect any existing `multi-codex` command and provide a documented migration path.
- Keep accounts, default login, settings and existing running Codex sessions intact.
- Do not require a real `.env` file for ordinary CLI use. If implementation changes project environment files, follow the repository's Varte sync instructions.
- Include uninstall instructions that distinguish removing the executable from deleting user profile data.

Use a user-owned executable directory for normal script installation. Support a package manager path only after the actual packaging/update behavior has been tested. A CLI update must not accidentally download the Tauri desktop app.

## 11. Shared architecture

Extract platform-neutral profile/settings/usage/storage services into a reusable Rust core. Keep the core independent of Tauri-specific IPC, events and app handles. The desktop app, native extension helper and new CLI depend on that core.

The CLI layer should contain argument parsing, interactive prompts, terminal rendering, JSON serialization and signal handling. Core services own credential validation, profile locks, launching rules and data mutation.

Preserve these existing storage roots:

```text
macOS: ~/Library/Application Support/multi-codex
Linux: $XDG_DATA_HOME/multi-codex (normally ~/.local/share/multi-codex)
profiles/<uuid>/codex-home
profiles/<uuid>/vscode-user-data
profiles/<uuid>/desktop-data
profiles/<uuid>/vscode-extensions
```

Do not create a second account store, silently migrate every profile, or copy credentials into a CLI-only database. Preserve existing profile UUIDs and keyring naming. Serialize metadata writes and respect extension leases across clients.

Native keyring backends should be selected per platform without making a Linux CLI depend on a graphical webview. Headless Secret Service failures must have clear setup guidance; there must be no silent plaintext fallback.

## 12. Output and errors

Interactive output should be compact and readable: account name, plan, status, usage freshness, selected workspace and explicit outcome. Use progress only while a real operation is pending. Respect reduced/no-motion terminal preferences and avoid incessant spinner output when redirected.

JSON responses should include a schema version and predictable fields. Do not expose backend protocol objects indiscriminately: they may contain private previews or paths. `accounts list --json` contains account metadata, never credentials.

Suggested exit contract:

| Code | Meaning |
|---|---|
| 0 | Completed successfully; update-check found no newer version is also success |
| 1 | Operation failed |
| 2 | Invalid arguments, ambiguous account or unsupported option |
| 3 | Required local capability unavailable: engine, terminal, keyring or permission |
| 4 | Busy account, lock conflict or stale deletion preview |
| 5 | Authentication or network operation failed |
| 130 | User cancelled with Ctrl+C |

Finalize these codes before publishing; they become a scripting contract. Partial bulk operations return nonzero and include individual results. Human errors should state the failing action and one concrete next step.

## 13. Verification and completion criteria

This is one release with the following required gates, not a phased roadmap:

1. Account creation/import/reconnection/removal works with native credential stores on all three target architectures.
2. GUI, CLI and extension see the same accounts and preserve existing IDs, settings and credentials.
3. A new terminal opens as account A; another opens as B. Verify identities and that A is unaffected. Repeat with spaces and long paths.
4. Default/global auth remains byte-for-byte unchanged after launch, usage, reconnect and cleanup tests.
5. Chat cleanup tests cover active and archived records, strict 30-day boundaries, recent descendants, stale previews, symlink escapes, locks, partial failure and preserved non-chat data.
6. Usage handles free/paid/API-key accounts, missing windows/credits, offline failures and real token refresh without exposing secrets.
7. All machine-readable commands produce valid stable JSON, separate stderr diagnostics and deterministic exit codes. Test non-TTY behavior and cancellation.
8. Signed install/update verifies the exact artifacts, rejects tampering, rolls back replacement failures and preserves account/chat sentinels.
9. Native Mac Terminal and Linux terminal launches are tested, including ARM64. Cross-compilation alone does not satisfy this gate.
10. Refactoring the core preserves desktop builds and existing extension/platform tests, including Windows, even though the CLI has no Windows package.
11. The website documents every command, installation requirements, isolation semantics, storage/deletion behavior, update flow, troubleshooting and known capability limits.
12. GitHub packages have matching versions/checksums/signatures and only a short link to the website's detailed release page.

Real OAuth, native keyrings, terminal spawning and packaged upgrades need platform checks alongside disposable fixture tests. Never use real user history for destructive verification.

## 14. Implementation order within the complete release

Inventory and isolate shared services first; define commands/output contracts next; implement account/usage/launch/settings/storage/update adapters; then complete native integration and package verification; finish website documentation and publication preparation. These are development steps for one complete release, not separate public versions.

## 15. Remaining design questions

- Confirm Linux ARM64 packaging/distribution baseline and availability of the official compatible engine on the actual target hosts.
- Choose whether the root interactive account menu should launch immediately after selection or show an explicit action menu. Proposed default: action menu, so choosing an account does not accidentally launch something.
- Confirm preferred installation channels: standalone archives/script first as a delivery format for the complete release, and whether Homebrew/distro repositories must also be ready at publication.
- Decide the exact non-interactive bulk-operation policy: proposed default is explicit target/scope plus `--yes`, with no automatic selection of all accounts.

The confirmed new-terminal behavior, one-release scope and Mac/Linux-only platform scope are settled; do not reopen those decisions without new user instructions.

# Multi Codex CLI — complete implementation plan

Status: **implemented with local verification; Linux x64 npm publication complete, 13 October 2026.** The grilling interview confirmed npm naming (scoped fallback @russeldp/multi-codex), publication of passing targets, one final main commit and separate desktop release scope.

Current delivery constraints:

- Keep only `main`; consolidate all work in one final commit.
- Deliver npm packages for macOS and Linux only. Agent is authorized to publish passing npm packages and deploy website changes. No new desktop GitHub release or release tags.
- Use local disposable fixtures for ongoing verification. Retain existing native evidence with its exact source and scope; do not claim that desktop runner results verify the future CLI. Native checks unavailable locally remain explicitly unverified.
- Owner authorizes updating the laptop launcher and activating shared storage after local verification. Preserve existing profiles, default authentication and backups; busy migrations remain deferred.

Decisions confirmed on 12 October 2026:

- Deliver one complete CLI release with all applicable Multi Codex features. Do not describe a limited first version followed by a second version.
- Support Apple Silicon Macs on **macOS 26+**, Linux x64, and Linux ARM64. Linux ARM64 is included in the planned deliverables; record native verification separately from cross-compilation.
- Selecting another account opens a **new terminal with that account**. It does not change authentication inside an existing terminal conversation.
- Reuse saved accounts and the same local storage as the desktop launcher and VS Code extension.
- Keep default Codex authentication untouched.
- Publish the updated command documentation on the website; desktop GitHub release assets remain outside this task.

## 1. Outcome and scope

A user should be able to install Multi Codex, add and manage accounts, inspect usage, launch workspaces, manage storage, update the tool, and diagnose problems entirely from a terminal. Installing or opening the desktop launcher must not be required.

Example: a user is working as Naigaon, then runs:

```sh
multi-codex launch --account Chriso --target cli --workspace ~/Projects/client
```

Multi Codex opens a new terminal in the selected workspace, running Codex with Chriso's isolated credentials and the managed history policy. The original Naigaon terminal continues unchanged. Closing either terminal does not delete its saved history.

The CLI manages local accounts and clients. It does not create subscription credits, combine quotas, provide a cloud account-sync service, or automatically rotate accounts when one runs out of usage.

The commands below describe the implemented command contract; see [README.md](README.md) for current installation and exact examples.

## 2. Platform contract

| Platform | Planned package | Requirements and verification |
|---|---|---|
| macOS | npm platform package containing a native arm64 executable and helper | Apple Silicon, macOS 26+, native Keychain access, real Terminal launch, private account directories |
| Linux x64 | npm platform package containing a native x86_64 executable and helper | glibc 2.39+; functional Secret Service/keyring connection |
| Linux ARM64 | npm platform package containing a native aarch64 executable and helper | Native ARM64 runner coverage, compatible official Codex engine, terminal discovery and keyring verification |

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
| Update installation | `update --check` and npm upgrade guidance |
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
    backups list | verify | restore
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
- Account removal explains that it removes managed private account data. Shared conversations remain available to other accounts; removal must never cascade into shared history. It requires confirmation, or an explicit `--yes` in automation.
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

Existing terminal sessions are never reauthenticated by this command. Shared VS Code conversations retain the implemented broker's single-writer and account-handoff rules. A new terminal does not automatically inherit the old terminal's conversation; do not bypass broker leases by pointing multiple engines at shared writable databases.

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
- For private cleanup, block while the account is running or leased. For shared cleanup, refuse any affected live writer or consumer and apply the shared broker protections. Use shared locks to prevent a new launch or credential operation racing with deletion.
- Refuse symlink escapes, foreign-profile paths and attempts to operate on the default/global Codex home.
- Preserve authentication, settings, skills, plugins and installed extensions.
- Require confirmation for deletion. Non-interactive commands need explicit account, scope and `--yes`; `--json` is not consent.
- Report partial progress honestly if native deletion fails after removing some chats. Do not imply an all-or-nothing transaction that the underlying operation cannot provide.
- This deletes local saved history; it does not delete cloud conversations or project source files.

For bulk cleanup across accounts, require an explicit `--all-accounts`, show a per-account preview, and skip/refuse busy accounts with a clear result. Never infer “all accounts” from an omitted account argument.

Backup commands expose existing verified backups and recovery state through the shared core. `storage backups restore --account NAME --backup ID` requires a verified snapshot, closed consumers, explicit confirmation and current credential preservation; it retains displaced newer work. Never overwrite the current authentication with an older snapshot credential. A pending journal must offer recovery rather than permit launch into an incomplete profile. Add these commands only through the existing restore services, not an independent file-copy implementation.

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

The npm installation is the supported delivery channel. `update --check` queries the configured npm registry for the root package version without changing anything. `update` identifies the installed package and gives the exact npm upgrade command; it must not replace npm-owned files with a desktop updater or launch an unrequested global install. JSON includes current/latest versions and the required action. If registry access fails, return a network error rather than “up to date.”

### Package layout and installation

- Use a thin root npm wrapper and three optional platform packages: `darwin-arm64`, `linux-x64`, and `linux-arm64`. Final package names depend on the owner's npm scope and name availability; keep names centralized so a rename does not alter the command contract.
- Each platform package contains the Rust CLI, the account helper needed by managed launches, license, build manifest and verification metadata. The wrapper selects the exact OS/architecture, forwards arguments/stdin/stdout/stderr, exit status and signals, and reports a missing optional dependency clearly.
- Root metadata allows only `darwin`/`linux`; each platform package declares its precise `os`/`cpu`. Reject Windows, Intel Macs, unsupported macOS versions and incompatible Linux runtimes with actionable errors. Do not silently download a different architecture.
- Lock root and optional dependency versions together. Use an explicit `files` allowlist and inspect `npm pack --dry-run --json`; exclude auth fixtures, private notes, build caches, source sessions and environment files.
- Include executables with executable mode, deterministic manifests and SHA-256 digests. Sign the payload manifest with a pinned verification key and verify the platform payload before execution. Keep private signing material outside packages and repository history. An unsigned development package is explicitly a local test artifact, not publication-ready.
- Build Linux binaries against a declared glibc baseline and audit dynamic dependencies. Do not claim universal distro or musl compatibility. Confirm the official engine supports each packaged target; do not silently depend on the desktop bundle.
- Test packed tarballs in an isolated npm prefix using a compatible Node LTS runtime; cover spaces, offline execution after install, signals, missing binaries, altered payloads, upgrade and uninstall. Avoid installing over the user's real launcher shim.
- Detect command-name collisions with the existing `multi-codex` desktop shim. Document the existing command's path and a deliberate replacement procedure; never overwrite an unrelated executable through an installer hook.
- Do not use install hooks to download mutable “latest” binaries, alter shell startup files, migrate profiles, or start a daemon. Removing npm packages leaves managed account data intact.
- Deliver tarballs, package inventory, hashes/signatures, validation results, and a manual publication checklist. Owner publishes platform packages first and root package last after verifying names, versions, access and provenance. This task never runs `npm publish`.

No real `.env` file is needed for normal CLI operation. Any authorized environment-file change follows the repository's Varte sync instructions.

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

These are acceptance criteria for the complete CLI. With hosted testing stopped, unavailable native criteria remain open and must be listed in the handoff; they do not authorize more runner jobs:

1. Account creation/import/reconnection/removal works with native credential stores on all three target architectures.
2. GUI, CLI and extension see the same accounts and preserve existing IDs, settings and credentials.
3. A new terminal opens as account A; another opens as B. Verify identities and that A is unaffected. Repeat with spaces and long paths.
4. Default/global auth remains byte-for-byte unchanged after launch, usage, reconnect and cleanup tests.
5. Chat cleanup tests cover active and archived records, strict 30-day boundaries, recent descendants, stale previews, symlink escapes, locks, partial failure and preserved non-chat data.
6. Usage handles free/paid/API-key accounts, missing windows/credits, offline failures and real token refresh without exposing secrets.
7. All machine-readable commands produce valid stable JSON, separate stderr diagnostics and deterministic exit codes. Test non-TTY behavior and cancellation.
8. Packed npm installation/upgrade validates signed payloads, rejects tampering, preserves account/chat sentinels and leaves a usable previous installation on failure.
9. Native Mac Terminal and Linux terminal launches are tested, including ARM64. Cross-compilation alone does not satisfy this gate.
10. Refactoring the core preserves desktop builds and existing extension/platform tests, including Windows, even though the CLI has no Windows package.
11. Local documentation covers every command, npm installation, isolation semantics, storage/deletion behavior, updates, troubleshooting and capability limits. Website publication is separate.
12. The npm tarball set has matching versions, explicit contents, checksums/signatures and publication instructions. Publish only verified targets under the latest owner authorization; desktop releases remain separate.

Real OAuth, native keyrings, terminal spawning and packaged upgrades need platform checks alongside disposable fixture tests. Never use real user history for destructive verification.

## 14. Implementation work order

1. Reapply the preserved CLI extraction onto the final shared-storage source. Resolve moved-file changes explicitly, especially `profiles.rs`, `profile_restore.rs` and native fixtures. The old extraction predates the restore corrections.
2. Establish `cli/crates/core` with no Tauri/WebKit dependency for CLI operations. Desktop and helper reuse the same implementation; retain native platform modules behind target/features. Preserve profile IDs, storage formats, keyring names, metadata locks, durable restore journals and broker ownership rules.
3. Wire argument parsing and dispatch. Freeze JSON schema version 1 and exit codes, account resolution, TTY detection, confirmation behavior, signals and credential-safe errors before adding interactive rendering.
4. Implement account CRUD/authentication, usage/credits, settings/status/doctor and engine discovery through core APIs. Account commands must not create a second store or trigger automatic migration.
5. Implement client/terminal launches, optional desktop placement and integration. Test process arguments and home isolation using owned fixtures. Report a started process separately from verified account identity.
6. Implement storage reporting and preview/apply cleanup, with explicit private versus shared scope. Shared chat deletion is global: require a shared-scope preview and confirmation, enforce broker leases, and never repeat the same shared deletion once per account. Keep backup restore/recovery reachable through documented storage commands if exposed by the desktop.
7. Finish npm wrapper, three native packages, signature verification and registry update guidance. Build a package without the desktop app installed and prove basic commands do not load GUI libraries.
8. Run local command/packaging/interoperability checks and relevant desktop regressions. Record host, source revision, exact package hashes, pass/fail/skip and limits. Do not schedule hosted jobs under the current instruction.
9. Prepare the complete owner handoff with package artifacts, command reference, manual publication order and outstanding native verification. Do not call the CLI complete while command handlers remain placeholders.

These are implementation steps for one complete deliverable, not separate limited public versions.

## 15. Contracts to settle during implementation

- Use an explicit action menu after account selection, so selection alone does not launch a client.
- Non-interactive deletion requires an explicit target, scope and `--yes`; `--json` never grants consent. Bulk operations require `--all-accounts` and report per-account results with a nonzero partial-failure exit.
- Resolve data root by explicit `--data-dir`, then the validated `MULTI_CODEX_DATA_DIR` override, then existing platform defaults. Treat global Codex home as an import/discovery source, not a destination for managed credentials.
- Finalize the Linux glibc baseline from actual build and dependency evidence. Native ARM64 and macOS behavior stay unverified if the required hosts are unavailable under the no-runner instruction.
- Final npm scope/name and signing public key are publication prerequisites supplied or confirmed by the owner. Continue local implementation and package validation using clearly marked development names/keys until then; do not reserve names or create releases.

## 16. Current implementation inventory

All CLI source, Rust workspace, shared core, npm wrapper/platform definitions, signing/packaging scripts and tests live under `cli/`. Account CRUD/device login, usage/credits/watch, account resolution, launches, storage/preview cleanup, backup restore/migration, extension maintenance, settings/palette, desktop shortcut integration, diagnostics and npm update guidance are wired to shared services.

Desktop consumes the same core while retaining its existing test selectors. Acceptance fixes are consolidated as uncommitted changes on the sole main branch until final verification and one commit. Current Linux packaging has a glibc 2.39+ baseline, signed Ed25519 payloads and an exact platform wrapper. No Windows or Intel Mac CLI package.

Local source/packed process tests exercise native credential persistence and cleanup, default-auth preservation, argument isolation with spaces, refusal/confirmation behavior, and tampered-payload rejection. Terminal fixtures verify invocation semantics; actual native editor/broker and installed-laptop checks are separate. Mac/ARM64 package definitions exist, but usable artifacts and native verification remain prerequisites for publication of those targets. Linux x64 and root packages are published as `multi-codex-linux-x64@1.0.5` and `@russeldp/multi-codex@1.0.5`. npm rejected the unscoped root name as too similar to an existing package; the approved owner-scope fallback retains the `multi-codex` command. Managed-terminal account commands are covered by a process regression check.

Owner explicitly authorizes npm publication and website deployment by the agent, one final main commit, and local launcher upgrade to the shared-storage build. No new desktop GitHub release or further hosted runner testing. Existing accounts and both retained/new pre-upgrade backups must remain protected. Busy accounts queue migration rather than having their clients forcibly closed.

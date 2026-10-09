# Changelog

## 0.2.1 — 2026-10-09

- Fix saving full-sized Windows OAuth credentials in Credential Manager while preserving existing saved accounts.
- Keep credential replacement atomic when a Windows vault write fails.

## 0.2.0 — 2026-10-09

- Wait for Codex initialization during account selection and report lost connections with a recovery action.
- Keep long chat history responses connected with bounded, incremental protocol framing.
- Match Windows backend executable paths across native case and separator differences.
- Add native Windows x64 support with Credential Manager, private account ACLs, authenticated named pipes, and an executable startup wrapper.
- Refresh all saved accounts from the top-bar Check Usage action, with progress, independent failures, remaining percentages and reset times.
- Keep active Windows wrappers intact during reactivation and wait for account/backend processes to release their handles during cleanup.
- Verify matching Linux x64, macOS arm64/x64 and Windows x64 VSIX files; filenames and protocol versions derive from the extension manifest.
- Keep the website as the primary setup and compatibility guide.

## 0.1.3 — 2026-10-09

- Start every Codex backend with the current launch home’s login, configuration, resources and history.
- Stop restoring old project account selections on startup; verify and announce explicit account changes.
- Resolve the installed official Codex engine on each open so extension updates cannot leave a deleted executable path.
- Refresh existing launcher configuration during activation and preserve the initial login when a first switch fails.

## 0.1.1 — 2026-10-09

- Fix desktop account discovery when VS Code inherits a managed profile's `CODEX_HOME`, including symlinked paths.
- Replace the large welcome buttons with a native account list, compact toolbar actions and visible loading/error states.
- Connect switching automatically when an account is selected, preserving the existing activity guards and current window.
- Show usage beside account rows and recover account loading after a failed helper startup.

## 0.1.0 — 2026-10-08

Initial Linux x64 and macOS (Apple Silicon and Intel, macOS 26+) pre-release.

- Manage accounts independently or share the desktop app's local account store.
- Add, rename and remove accounts, and inspect usage per account.
- Attach to compatible running Codex backends and switch ChatGPT accounts without reloading VS Code, either extension or the Codex panel.
- Verify the selected identity and roll back failed account changes.
- Refuse switching during observed active work, approvals and uncertain activity.
- Use the official extension's bundled engine, including its current macOS directory layouts; no separate CLI installation is required.
- Build and test each platform's VSIX on native Linux and Mac runners.

See the README for tested Codex versions, private integration hooks, queued-message limits and API-key restrictions. Windows, Linux ARM64 and remote environments are not supported by this release.

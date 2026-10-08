# Changelog

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

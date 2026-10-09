# Codex startup fix — 2026-10-09

Multi Codex extension 0.1.3 fixes conflicts with VS Code opened by the desktop app.

## Confirmed causes

- A persisted `chatgpt.cliExecutable` wrapper started a new private authentication home without the current login.
- The global home used for shared account discovery also replaced the desktop account home used for configuration and resources.
- Saved project selections silently restored a previous account after startup.
- One installed profile referenced Codex 26.1002.51308 after its engine was removed by the update to 26.1007.21434. This prevented backend startup.
- The newer backend can return no token through `getAuthStatus` for a file-based ChatGPT login. Verified rollback now checks the backend routing identity against the private auth file in that case.

## Result

Each startup uses the launch environment’s current Codex home, with a configured/default fallback. It copies the file-based login and configuration into a private backend home, and links resources and history to the source home. Account storage remains separate. Previous project selections are ignored. Explicit account changes display a notification after identity verification and preserve the current conversation. Launcher refreshes replace files atomically. The official executable is resolved from the same VS Code installation on each startup.

The desktop launcher source also explicitly sets `CODEX_SQLITE_HOME` to the selected account home. This source change was tested; the running installed desktop binary was not replaced. The installed extension already uses the current launch home when that variable is absent.

## Verification

- All 32 extension tests passed, including the native helper, fresh opens, changed launch homes, old selection files, deleted engine paths, first-switch rollback, activity guards and authentication checks.
- The Rust desktop launch environment test passed.
- Real VS Code startup and live attachment flows passed against installed Codex 26.1007.21434 with synthetic accounts and a loopback service. These checked account A → B → A, usage, a running-turn switch rejection, and continuation of the same chat as B without restarting the host, backend or panel.
- The final Linux x64 VSIX was extracted and tested in real VS Code with the same startup flow.
- Version 0.1.3 was installed into both existing profiles containing Multi Codex. Their saved launcher/configuration files were backed up and repaired to reference existing executables and the new extension.
- The seven original tracked account files (account metadata and six auth files) were unchanged by installation. No real login, account switch or chat was performed during local installation.
- Both installed launcher scripts passed shell syntax checks and reference existing engine/helper paths.

The update takes effect when the existing VS Code windows are reopened. Browser OAuth and macOS were not tested in this Linux session. This package was installed locally; it was not published to Marketplace.

## Artifact

`extension/multi-codex-linux-x64-0.1.3.vsix`

SHA-256: `e91424f0bfdc871d61ab07bbf0557bcd91b891ce081251e4a3a61745a16e37b4`

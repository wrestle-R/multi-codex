# Windows support — deferred context

Status: deferred at the user's request on 2026-10-05. Do not include Windows implementation in the current Linux/macOS launch and settings work.

Multi Codex is a Tauri/React app that currently launches isolated VS Code profiles. Its Rust backend relies on Unix file modes, Unix-specific APIs, Linux /proc inspection and macOS sysctl process inspection. A Windows port needs platform implementations, not just an installer change.

Future scope:
- Introduce Windows equivalents for private directory/file access, atomic settings persistence, executable discovery, process detection, native browser opening and profile launch.
- Use Windows-native app data paths and credential-store integration. Preserve stable profile IDs and data across upgrades.
- Handle Windows paths, spaces and Unicode in both the Rust backend and folder-picker UI.
- Add Windows CI, native build/test jobs and a Tauri NSIS or MSI installer, plus a PowerShell installer/updater.
- Begin with ordinary workspace launches; defer virtual-desktop inventory/placement.
- Verify fresh install, account isolation, simultaneous/repeated launches, restart persistence, missing apps, permissions and upgrade without data loss.

Do not assume desktop Codex/ChatGPT accepts VS Code flags or that Codex CLI credential files also authenticate desktop UI sessions. Verify the installed app version and authentication boundary first.

Reference: https://v2.tauri.app/distribute/windows-installer/

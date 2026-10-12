# Storage cleanup and in-app updates

These controls are implemented in the development source and are **not included in the published v1.4.1 packages**. The [changelog](/releases) identifies published versions and their exact capabilities. Use the existing installer/update commands for v1.4.1.

## Understand profile storage

Each saved account has separate managed directories:

| Category | Contents |
|---|---|
| Installed extensions | VS Code extensions installed for this account |
| VS Code data | Settings, workspace state, local storage, caches and logs |
| Codex data | Local conversations, history databases, skills, plugins, configuration and temporary files |
| Codex Desktop data | The Desktop client's separate app state and browser-engine data |

Chat transcript size is part of Codex data. Cache size is part of total storage. Do not add either subset to the total a second time.

Multi Codex does not enforce a maximum storage size. Data grows as you use accounts and install extensions or plugins. Choosing manual cleanup does not install an automatic retention policy.

On Linux, the default account store is `~/.local/share/multi-codex`. On Mac it is `~/Library/Application Support/multi-codex`. Each account uses `profiles/<profile-id>/codex-home`; saved chats are in its `sessions` and `archived_sessions` folders. Other database records support the chat history list.

An online AI response does not mean a local conversation can be restored from cloud storage after deletion. These cleanup controls operate on local account history.

## Clear disposable cache

Open the storage total in the app's header. A profile's cache button removes its recognised disposable VS Code caches, crash reports and logs, plus Codex's `cache`, `.tmp`, `tmp` and `log` directories.

Credentials, conversations, settings, skills, plugins and installed extensions are preserved. Desktop caches are not currently included in this allowlist. “Clean” means no recognised removable cache; it does not mean the profile uses no space.

“In use” means its sessions must be closed before cleanup. Close that account's VS Code windows, Codex Desktop windows and Codex CLI sessions first.

## Clear saved local chats

Open **Profile storage → Data and chats** for the account you want to clean. Inspect the category breakdown, then select either:

- **Clear all chats:** select saved local chats, including archived and subagent chats.
- **Clear chats older than 30 days:** select chats last active more than 30 days ago. A recently continued old chat is kept. Older parents are also kept when deleting them would remove newer subagent chats.

The confirmation shows the selected chat count and transcript size. Nothing is deleted until you click **Delete selected chats**. Deletion is permanent and those local chats cannot be resumed.

Codex's native deletion operation maintains conversation metadata. Multi Codex also removes the selected conversations' entries from local CLI prompt history and the title index. Your account, configuration, skills, plugins and installed extensions are kept; cloud conversations and project source files are not deleted.

If the history changes after the preview, cleanup stops and asks for a new preview. An unsupported Codex version, unavailable engine, busy profile, unsafe linked path or uncertain chat state blocks cleanup with an explanation. Use a compatible current Codex engine that supports local thread deletion.

Native deletion can complete some selected chats before a later operation fails. The error reports that partial progress; check history and preview again rather than assuming the operation was rolled back.

## Update from the app

In a release containing these controls, click **Update** in the header. Multi Codex checks the stable release, downloads the matching package, verifies its signature, installs it and restarts. Progress and the restart state are shown in the app. A failed check or installation offers a retry.

Saved accounts and profile data are preserved. Updating the launcher does not intentionally close separately launched Codex sessions.

Supported in-app update packages are Linux x64 AppImage, Apple Silicon macOS application bundles, and Windows x64 installers. Linux `.deb` and `.rpm` installations continue to use their package manager. A development binary is not an installed AppImage.

Signed updater packages and a matching `latest.json` must be published before this flow can download an update. Existing v1.4.1 release assets do not include them. Update-signature verification is separate from Apple Developer ID signing and notarization; signing an updater payload does not remove macOS first-open requirements.

## Release publishing requirements

The release workflow builds and signs updater packages, signs the final repacked Linux AppImage, and creates a manifest referencing the exact platform files and signatures. Publishing checks the manifest alongside normal checksums and functional release gates.

GitHub Actions needs the matching `TAURI_SIGNING_PRIVATE_KEY` secret and, if applicable, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. The public verification key belongs in the Tauri configuration. Private signing material must never be committed, placed in public release notes or printed in logs.

Full release details are published on this website at `/releases/v<VERSION>`. GitHub release descriptions contain a short link to that page and host the download artifacts. See the [release guidelines](/docs/release-guidelines).

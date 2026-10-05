# Launch settings and app selection — approval preview

Date: 2026-10-05
Status: visual revision 2 approved by the user on 2026-10-05. Settings, welcome, detection and Mac runner work are implemented for v1.3.4. Standalone launch remains gated pending authenticated isolation on both platforms.

## Settings and folder selection

Keep Preferred folder with a Browse action visible. Reuse WorkspacePickerDialog for selecting the preference, with Choose this folder/Cancel and no launch side effect. Keep executable overrides and global Codex home under a collapsed Advanced disclosure. Rename Save paths to Save settings.

Show the desktop-picker toggle only after the backend positively identifies Linux. Keep it hidden during loading, detection failure and on macOS. Keep the existing Mac direct-launch behavior.

## First run and available apps

The welcome screen is deliberately simple: the existing brand mark, a short welcome message, a single Get started action and an unobtrusive automatic detection status. No settings, folder fields, app-selection controls, paths or installation checklist appear on welcome. Use a short opacity/translation entrance, a 3px slow mark drift and restrained button hover; respect reduced-motion preferences.

On welcome, read the platform from the native backend and automatically detect the installed VS Code app, Codex CLI availability (including its IDE-extension fallback), and the desktop Codex/ChatGPT app. Save setup completion and detected state. Revalidate platform/app availability at startup and before launch, since apps can later be installed or removed. An existing user's accounts and defaults are preserved.

Show the entire Open accounts with section in settings ONLY when both apps and their dependencies are present. Hide its heading, selector and installed-status labels when only one app is available; automatically use the available target. With no usable target, give a recoverable installation action after welcome without presenting misleading launch choices. When both are present, offer VS Code extension only, Codex app only and Both. The standalone target additionally requires validated authentication isolation. Show two launch buttons only when Both is selected and both targets are usable. Never switch accounts in a running global desktop app as a fallback.

## Isolation investigation

Local installed desktop app: ChatGPT 26.930.51102 at /usr/lib/chatgpt, launcher /usr/bin/chatgpt. Read-only archive inspection found CODEX_HOME resolution and CODEX_ELECTRON_USER_DATA_PATH handling for desktop user data and instance locks. Mac-specific demo launch code sets both paths when opening a new app instance. These are local implementation findings, not a public stable API promise.

Two disposable signed-out Linux instances ran concurrently with separate temporary HOME/XDG directories, Codex homes and desktop-data paths. They created separate cookie databases and desktop state. No account credentials were imported. This establishes independent signed-out instance startup only. It does not establish isolation of two authenticated identities, credential refresh, Keychain identities, or reuse of existing VS Code auth in the desktop UI. Desktop launch integration must remain gated until those pass. Probe evidence resides outside the repository at /tmp/mci-42l7ae5d/results.json.

Public documentation: https://learn.chatgpt.com/docs/config-file/environment-variables and https://learn.chatgpt.com/docs/developer-commands . CODEX_HOME support does not by itself establish desktop browser-session isolation.

## More thorough Mac CI using GitHub runners

Use GitHub-hosted macOS runners; no AWS. Add isolated temp directories and controlled test credentials/mocks for executable detection, onboarding, setting migration, folder selection, direct launch and error states. Exercise paths with spaces/Unicode, empty GUI PATH and inherited environment. Assert default auth/config and installed user data are unchanged.

Expand package verification to installation into a temporary Applications directory, restart persistence and real disposable VS Code/desktop startup; verify actual windows and child-process state instead of only process survival. For desktop isolation, run two signed-out app instances, compare their data/socket/session paths and use a controlled test account only if login can be safely automated. Save sanitized logs, screenshots and test reports as CI artifacts. Separate signed/notarized installer checks from unsigned startup checks. Record interactive sign-in/permission checks as unverified when runner automation cannot exercise them.

## After visual approval

1. Complete the authenticated desktop isolation proof before enabling that launch target. If it fails, report the finding and revise the scope with the user.
2. Implement the approved settings, welcome screen, detection and target-specific launch flows with backward-compatible saved settings and unchanged existing profile IDs/data paths.
3. Validate Linux and Mac CI; use demo profiles for polished real screenshots in both themes, matching the user's populated reference. Do not publish generated concept images as screenshots of the working app.
4. Commit implementation and README screenshot updates; prepare the next release and await successful builds. Preserve honest release validation/signing status.
5. Back up local app data and old executable, verify account IDs and credential files, download/checksum the exact release artifact, replace only the executable and verify startup/accounts. Keep rollback backups. Never reinstall or recreate current profiles.

## Visual approval artifacts — revision 2

The earlier generated account layout is rejected as a README screenshot. Preserve the existing accounts interface, current brand mark and app-data storage total. README images must be real captures of the implemented app with realistic demo profiles, captured from a native desktop window at 1180x980 so all three populated profiles and the footer fit; include both themes. Never replace working-app screenshots with generated designs, invented storage indicators or decorative UI.

These revision-2 images are browser-rendered design prototypes using the actual application CSS and fonts, not screenshots of implemented production features:

- [Simple welcome](previews/welcome-v2.png)
- [Linux settings, one installed app: app-selection section hidden](previews/settings-single-v2.png)
- [Linux settings, both installed apps](previews/settings-both-v2.png)
- [Mac settings: desktop toggle hidden](previews/settings-mac-v2.png)
- [Animated interactive preview](previews/interface-preview.html)

The preview HTML lives under docs/plans and remains a design reference; v1.3.4 contains the corresponding production settings/welcome changes. Browse demonstrates the existing folder-picker layout. It will reuse the real WorkspacePickerDialog implementation after approval. Settings/navigation in the static preview are visual prototypes, not saved preferences or app launch code.

## v1.3.4 implementation boundary

Repeatable standalone probe commands, interactive test instructions and current evidence are tracked in [Standalone isolation verification](standalone-isolation-verification.md). The standalone target stays disabled while authenticated checks are incomplete.

VS Code remains the only enabled launcher. The app-choice section is completely absent unless both VS Code and the standalone app are detected; when both are installed it shows VS Code selected and the standalone choices disabled with the verification reason. No dual launch buttons are advertised as working. CODEX_HOME plus a separate desktop user-data directory is promising implementation evidence, but automated signed-out startup does not establish authenticated Linux/Mac isolation. No real user credentials were copied into the standalone probe.

Welcome is shown only for an empty account list with incomplete setup. Existing accounts skip onboarding. Get started atomically saves setup completion and a native detection snapshot in the existing settings file; startup/settings/prelaunch query current installation state. Missing VS Code gives a recoverable installation/path message. The snapshot never overrides the detected OS.

GitHub macOS CI and release jobs run the real disposable Keychain and VS Code startup tests, inspect the Apple Silicon bundle, start it twice in an isolated HOME with spaces/Unicode and an empty GUI PATH, require an on-screen native window, and compare unchanged global auth/config, profile metadata and legacy settings fixtures. Evidence is uploaded even on failure. Real account sign-in, permission dialogs, standalone account isolation and signing/notarization remain separate unverified checks.

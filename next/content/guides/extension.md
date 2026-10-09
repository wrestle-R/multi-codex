# Multi Codex

Manage saved accounts and change the account used by the **existing official Codex panel**. Account switches leave VS Code, Multi Codex, the Codex webview and its backend running. An existing local chat can continue using the newly selected account.

- Add accounts using browser sign-in or import an existing Codex login.
- Select accounts from the activity bar or status bar.
- Check account usage without switching accounts.
- Share saved accounts with the Multi Codex desktop app, or use the extension on its own.
- Block account changes while the backend reports active work or uncertain activity.

**Linux, macOS and Windows pre-release.** Platform packages target Linux x64, Apple Silicon Macs, Intel Macs and Windows x64. The Mac native helper requires macOS 26 or newer. The extension is free and open source. Your Codex subscription or API usage is billed separately. Multi Codex is an independent project and is not affiliated with or endorsed by OpenAI.

This preview uses an experimental Codex backend authentication API. Live attachment uses a private Node process/stdio compatibility hook; later startups use Codex's development-only executable setting. It is not an official OpenAI extension API integration. This release targets local VS Code on Linux x64, macOS arm64/x64 and Windows x64. Remote SSH, containers, WSL, Linux ARM64 and vscode.dev are not supported yet.

## Version 0.2.0 downloads

All four platform packages are included in [Multi Codex v1.4.0 on GitHub](https://github.com/wrestle-R/multi-codex/releases/tag/v1.4.0). The owner uploads Marketplace updates separately. If v0.2.0 or your target platform is not listed yet, download its VSIX and use **Extensions: Install from VSIX…**.

## Get started

1. Install the official `openai.chatgpt` Codex extension.
2. Install the pre-release of **Multi Codex** by **russeldanielpaul**. VS Code selects the matching platform package. For a downloaded package, run **Extensions: Install from VSIX…** and choose the Linux x64, macOS Apple Silicon (`darwin-arm64`), macOS Intel (`darwin-x64`) or Windows x64 (`win32-x64`) VSIX for your machine.
3. Open Multi Codex in the activity bar. Existing desktop accounts appear automatically; use **+** in the Accounts toolbar to add an account if you are starting fresh.
4. Click an account row to connect switching automatically and use that account in the current official Codex panel. Hover over an account to check usage; the result appears beside its name.

**On the verified Codex build, initial attachment and account changes need no restart or reload at all.** The bridge attaches to the existing backend without replacing its process or restarting either extension. An incompatible or ambiguous backend is refused; VS Code is never automatically reloaded. See the compatibility limits below.

You do not need the Multi Codex desktop app, a separately installed Codex CLI, or the Codex desktop client. The extension uses the engine shipped inside the official VS Code extension. The VSIX contains the native account helper.

The [website](https://multi-codex.vercel.app/docs/extension) is the primary guide. Matching platform VSIX files are available from [GitHub Releases](https://github.com/wrestle-R/multi-codex/releases/latest). Artifact filenames and protocol client versions come from the extension manifest.

## Accounts and desktop synchronization

Existing desktop accounts are discovered automatically from the same local store:

VS Code launched by a desktop account can inherit that profile's `CODEX_HOME`. The extension keeps shared account storage separate from that managed profile and resolves the global home from desktop settings or the default home. Account loading failures appear in the panel; use **Refresh** to retry after correcting a setting. No account needs to be copied or added again.

- Linux: `$XDG_DATA_HOME/multi-codex`, normally `~/.local/share/multi-codex`.
- macOS: `~/Library/Application Support/multi-codex`.
- Windows: `%APPDATA%\multi-codex`.

Without desktop accounts, the list starts empty. **Add Account** supports browser sign-in, importing the current Codex login, and importing an auth JSON file. The same saved accounts become visible to the desktop app if it is installed later. Rename and removal are available from an account's context menu.

The account list refreshes every 1.5 seconds and after account changes. Both clients read the same `profiles.json`, use the same profile IDs and share the existing OS keyring integration. There is no cloud synchronization service. `multiCodex.dataDirectory` and `multiCodex.globalCodexHome` can override the store and source Codex home using absolute paths.

This repository's updated desktop service and the extension helper serialize metadata writes using a file lock. Active extension accounts hold shared leases that prevent deletion, credential replacement and cache cleanup. Older installed desktop releases do not understand these new locks: update/rebuild the desktop app before concurrently editing shared accounts there.

The active account is shown in the status bar and account list. Check every saved account with **Multi Codex: Check All Accounts’ Usage**, or use an account's **Check This Account's Usage** context action without switching. Usage comes from Codex's backend and displays the returned usage windows; unavailable limits are not invented. API-key billing is not a ChatGPT usage window.

## How switching works

On first enable, Multi Codex locates exactly one child process for the official bundled engine inside the shared local extension host. A reversible stdio adapter routes that already-running process through the bridge while retaining its original protocol reader. The bridge probes its authentication identity before enabling account changes. No installed official-extension file is changed.

The configured `chatgpt.cliExecutable` wrapper handles later normal startups, launching the **official bundled engine** with a private credential home and forwarding its normal protocol. Every normal startup reads the current launch’s `CODEX_HOME` (or the configured/default home), copies its file-based login and configuration into the private credential home, and keeps its resources and history available. Shared account storage uses the separate global home. The launcher resolves the currently installed official engine on each startup, including after extension updates. Both attachment modes use a private authenticated Unix socket (Linux/Mac) or Windows named pipe to connect the account picker to the bridge.

When you select an account, the bridge:

1. Takes an exclusive switching guard and refuses newly submitted work during authentication.
2. Protects the saved account with a lease and reads its current credential privately.
3. Asks the backend for every loaded thread and verifies each is idle.
4. Calls `account/login/start` with the selected external ChatGPT tokens. The isolated startup bridge also supports API keys.
5. Reads the adopted authentication identity and checks it against the selected account.
6. Shows a verified-login confirmation, releases the previous account's lease, and allows work again.

Authentication failures and mismatched identities trigger verified rollback to the previous account. If rollback cannot be verified, switching stays blocked. The wrapper does not patch the installed official extension, overwrite its default auth file, invoke `workbench.action.reloadWindow`, or open another project window.

Live ChatGPT attachment changes in-memory authentication without changing the original login file. API-key switching is refused in this mode because the backend would write its original credential store; it is available through the isolated wrapper on a later normal startup. Credentials in that wrapper are private to each backend. Existing session directories, resources and the SQLite history home remain available from the source Codex home. Selecting an account intentionally preserves the current conversation; this is **not a guarantee that prior conversation content is isolated between accounts**. Cloud conversations, organization-specific tools and permissions may depend on the selected account. Account selection applies to this running window. Each later startup begins with the current launch home’s login; old project selections are ignored. The status bar indicates when Codex is using the current home and shows its path in the tooltip.

Expired external tokens are refreshed through a short-lived bundled backend using the saved account's home. Credential-operation locks serialize refresh/usage operations between extension windows. Browser sign-in and real token rotation require OpenAI connectivity; these live flows are not covered by the offline fixture tests.

## Connection recovery

Account selection waits for backend initialization. If the backend or protocol connection is lost, the extension reports the failure and keeps switching blocked. Save your work and reopen Codex or VS Code to start a fresh backend. Long chat responses no longer hit the former 8 MiB bridge limit. A failed connection is never treated as a verified idle session.

## Switching guards and limits

Switching is blocked while the bridge observes a submitted/running turn, active thread, subagent, compaction, command/process, approval, requested input, token refresh or known queued message. Concurrent switches are rejected. Loaded-thread inspection catches active chats outside the visible panel. Missing accounts, backend disconnection, unknown activity and opaque queue notifications fail closed.

**Codex does not expose every queue held in the panel's UI.** The current IDE backend emits a queue-change notification without exposing a reliable queue-inspection method. That notification blocks switching conservatively. Purely UI-local queued follow-ups may be invisible until submitted. Clear queued follow-ups before changing accounts; the guard cannot promise atomic queue isolation without cooperation from the official extension. Uncertain queue state may require closing Codex's backend at a safe time before switching becomes available again.

Live attachment was verified on Linux x64 with VS Code 1.140.0 against Codex extension 26.1002.51308 / backend 0.162.0-alpha.2 and Codex extension 26.930.61225 / backend 0.160.1. It relies on Node's private `_getActiveHandles` API and reversible stream method interception, not a public Codex integration contract. Future host or Codex changes can make it unavailable. Multiple matching processes, an API-key current login, broken framing, or failed identity verification stop attachment safely. A backend that cannot attach can use the isolated wrapper on its next normal startup; this fallback does not promise restart-free initial setup.

Only this window's bridged backend is switched. Existing unbridged VS Code windows and other Codex clients keep their own accounts. This is not automatic quota-based account rotation. A Codex update may change experimental authentication or executable behavior; identity verification and activity checks must pass before a switch is accepted.

To restore the bundled executable setting, run **Multi Codex: Disable Account Switching** and reopen VS Code when convenient. If uninstalling first, remove the `chatgpt.cliExecutable` setting manually. Disable/enable never triggers an automatic reload.

## Development and local verification

Use Node 22+ (Node 24 recommended), Rust and the platform's native build dependencies.

```bash
cd extension
npm ci
npm run build:helper
npm test
npm run test:vscode
npm run test:attachment
npm run package -- --target linux-x64 --pre-release
```

`npm test` runs guard, protocol bridge, rollback, identity and concurrency tests. Set `MULTI_CODEX_TEST_ENGINE` to the official extension's bundled engine to include native account-store integration tests and to run `npm run test:engine`.

`test:vscode` discovers the newest locally installed official extension, launches a disposable VS Code profile, creates synthetic accounts, and serves authentication routing, usage and model responses on loopback. Port 8000 must be available. `test:attachment` starts the official backend without a preconfigured bridge and verifies that enabling Multi Codex keeps the already-running process and both extensions alive; it also checks that the original login remains byte-for-byte unchanged. Both modes record stable extension-host/activation/backend IDs, test A → B → A, block switching during a real streaming backend turn and continue that same thread as B. It also inspects the rendered Codex webview via CDP. A **disposable copy** of the official extension gets a test-only fetch shim so its web requests go to the local fixture and its saved onboarding flags are initialized for an existing-user scenario; installed extension files are untouched. OAuth, first-use onboarding, cloud authorization, real quota enforcement and OS keyring availability on other systems still need live/platform verification.

Local results and screenshots are written to `.test-results/`. Tests never install into your normal VS Code profile, publish a package or push Git changes. VS Code itself may perform its own built-in background checks; the Codex fixture uses synthetic credentials and local responses.

The **VS Code extension packages** workflow builds each native helper on its target operating system. It tests the extracted VSIX in real VS Code with the official Codex extension, checks live attachment and isolated startup, and uploads installable packages only after those checks pass. These fixtures verify account routing and activity guards; they do not certify real OAuth, subscription limits or cloud permissions.

## Support

Report problems or request platform support in [GitHub Issues](https://github.com/wrestle-R/multi-codex/issues). Include your operating system, VS Code version, Codex extension version and the command that failed. Never include auth files, tokens or private conversation content.

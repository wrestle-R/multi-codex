# Multi Codex for VS Code — local preview

Manage saved accounts and change the account used by the **existing official Codex panel**. Account switches leave VS Code, Multi Codex, the Codex webview and its backend running. An existing local chat can continue using the newly selected account.

This preview uses an experimental Codex backend authentication API. Live attachment uses a private Node process/stdio compatibility hook; later startups use Codex's development-only executable setting. It is not an official OpenAI extension API integration. The packaged preview targets local Linux x64; macOS code paths require their own native build and validation. Remote SSH, containers, WSL, Windows and vscode.dev are not supported yet.

## Install locally

1. Install the official `openai.chatgpt` Codex extension.
2. In VS Code, run **Extensions: Install from VSIX…** and select `multi-codex-linux-x64-0.1.0.vsix` from this directory.
3. Run **Multi Codex: Enable Account Switching**.
4. Open the Multi Codex activity bar, add or select an account, and use the official Codex panel normally.

**On the verified Codex build, initial attachment and account changes need no restart or reload at all.** The bridge attaches to the existing backend without replacing its process or restarting either extension. An incompatible or ambiguous backend is refused; VS Code is never automatically reloaded. See the compatibility limits below.

You do not need the Multi Codex desktop app, a separately installed Codex CLI, or the Codex desktop client. The extension uses the engine shipped inside the official VS Code extension. The VSIX contains the native account helper.

## Accounts and desktop synchronization

Existing desktop accounts are discovered automatically from the same local store:

- Linux: `$XDG_DATA_HOME/multi-codex`, normally `~/.local/share/multi-codex`.
- macOS source implementation: `~/Library/Application Support/multi-codex`.

Without desktop accounts, the list starts empty. **Add Account** supports browser sign-in, importing the current Codex login, and importing an auth JSON file. The same saved accounts become visible to the desktop app if it is installed later. Rename and removal are available from an account's context menu.

The account list refreshes every 1.5 seconds and after account changes. Both clients read the same `profiles.json`, use the same profile IDs and share the existing OS keyring integration. There is no cloud synchronization service. `multiCodex.dataDirectory` and `multiCodex.globalCodexHome` can override the store and source Codex home using absolute paths.

This repository's updated desktop service and the extension helper serialize metadata writes using a file lock. Active extension accounts hold shared leases that prevent deletion, credential replacement and cache cleanup. Older installed desktop releases do not understand these new locks: update/rebuild the desktop app before concurrently editing shared accounts there.

The active account is shown in the status bar and account list. Check active-account usage with **Multi Codex: Check Usage**, or use an account's **Check This Account's Usage** context action without switching. Usage comes from Codex's backend and displays the returned usage windows; unavailable limits are not invented. API-key billing is not a ChatGPT usage window.

## How switching works

On first enable, Multi Codex locates exactly one child process for the official bundled engine inside the shared local extension host. A reversible stdio adapter routes that already-running process through the bridge while retaining its original protocol reader. The bridge probes its authentication identity before enabling account changes. No installed official-extension file is changed.

The configured `chatgpt.cliExecutable` wrapper handles later normal startups, launching the **official bundled engine** with a private credential home and forwarding its normal protocol. Both attachment modes use a private authenticated Unix socket to connect the account picker to the bridge.

When you select an account, the bridge:

1. Takes an exclusive switching guard and refuses newly submitted work during authentication.
2. Protects the saved account with a lease and reads its current credential privately.
3. Asks the backend for every loaded thread and verifies each is idle.
4. Calls `account/login/start` with the selected external ChatGPT tokens. The isolated startup bridge also supports API keys.
5. Reads the adopted authentication identity and checks it against the selected account.
6. Records the selection, releases the previous account's lease, and allows work again.

Authentication failures and mismatched identities trigger verified rollback to the previous account. If rollback cannot be verified, switching stays blocked. The wrapper does not patch the installed official extension, overwrite its default auth file, invoke `workbench.action.reloadWindow`, or open another project window.

Live ChatGPT attachment changes in-memory authentication without changing the original login file. API-key switching is refused in this mode because the backend would write its original credential store; it is available through the isolated wrapper on a later normal startup. Credentials in that wrapper are private to each backend. Existing session directories, resources and the SQLite history home remain available from the source Codex home. Selecting an account intentionally preserves the current conversation; this is **not a guarantee that prior conversation content is isolated between accounts**. Cloud conversations, organization-specific tools and permissions may depend on the selected account. The most recently selected account for each project is restored on a later startup when activity can be verified.

Expired external tokens are refreshed through a short-lived bundled backend using the saved account's home. Credential-operation locks serialize refresh/usage operations between extension windows. Browser sign-in and real token rotation require OpenAI connectivity; these live flows are not covered by the offline fixture tests.

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
npm run package -- --target linux-x64
```

`npm test` runs guard, protocol bridge, rollback, identity and concurrency tests. Set `MULTI_CODEX_TEST_ENGINE` to the official extension's bundled engine to include native account-store integration tests and to run `npm run test:engine`.

`test:vscode` discovers the newest locally installed official extension, launches a disposable VS Code profile, creates synthetic accounts, and serves authentication routing, usage and model responses on loopback. Port 8000 must be available. `test:attachment` starts the official backend without a preconfigured bridge and verifies that enabling Multi Codex keeps the already-running process and both extensions alive; it also checks that the original login remains byte-for-byte unchanged. Both modes record stable extension-host/activation/backend IDs, test A → B → A, block switching during a real streaming backend turn and continue that same thread as B. It also inspects the rendered Codex webview via CDP. A **disposable copy** of the official extension gets a test-only fetch shim so its web requests go to the local fixture and its saved onboarding flags are initialized for an existing-user scenario; installed extension files are untouched. OAuth, first-use onboarding, cloud authorization, real quota enforcement and OS keyring availability on other systems still need live/platform verification.

Local results and screenshots are written to `.test-results/`. Tests never install into your normal VS Code profile, publish a package or push Git changes. VS Code itself may perform its own built-in background checks; the Codex fixture uses synthetic credentials and local responses.

## Publishing later

Uploading is deferred. The package currently uses `multi-codex-local` as a local publisher placeholder. See [Marketplace requirements](../docs/extension-publishing.md) before choosing a permanent publisher ID and publishing a rebuilt package.

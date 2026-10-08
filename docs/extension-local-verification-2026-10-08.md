# VS Code account switching and launcher: local verification

This records the original Linux preview before the Marketplace release. For the subsequent Linux and native macOS packages, see [Marketplace release verification](extension-marketplace-verification-2026-10-08.json). The artifact hash and test counts below belong to this earlier preview.

The Linux x64 preview attaches to an already-running official Codex backend and switches accounts without reloading VS Code, reactivating either extension, recreating the panel, restarting the backend, or creating another chat. **Initial ChatGPT attachment also needs no restart on the two verified Codex builds.** This uses a private Node compatibility hook and an experimental backend authentication API, not a public Codex extension API.

The artifact is `extension/multi-codex-linux-x64-0.1.0.vsix`, packaged with an optimized native helper. SHA256: `a5c09e078c61410cc7e860b6d57ceb75cab316f95e254dece4ca74ca1bdefe73`. The VSIX was extracted; both runtime bundles and the helper matched the build byte-for-byte. That extracted package passed the actual VS Code checks below.

Verified on Linux x64 with VS Code 1.140.0:

| Official Codex extension | Bundled backend | Packaged live attachment |
| --- | --- | --- |
| 26.1002.51308 | 0.162.0-alpha.2 | Passed without any restart |
| 26.930.61225 | 0.160.1 | Passed without any restart |

| Check | Result |
| --- | --- |
| TypeScript checking, bundle build, optimized helper and VSIX packaging | Passed |
| Extension tests, including native helper processes | 26 passed, 0 failed, 0 skipped |
| Rust tests | 85 passed, 0 failed; 3 existing desktop-session tests ignored |
| Desktop UI tests and production frontend build | 78 passed; build passed |
| Already-running official backend attachment | Same backend PID, host PID and Multi Codex activation ID |
| Actual bundled backend A → B → A | Identity verified; same process and rendered panel |
| Startup wrapper in extracted VSIX | Passed independently |
| ChatGPT → API key → ChatGPT in isolated backend | Identity verified; same process |
| Live attachment and original login file | Byte-for-byte unchanged on both builds |
| Active-account status bar and usage | Account label matched; backend limits read |
| Actual streaming turn | Switching rejected until completion |
| Existing chat continued after switching | Same thread; local model requests attributed to A, then B |
| Missing account, failed authentication, wrong identity | Previous verified identity retained/restored |
| First live switch fails before a managed selection | Original unmanaged identity restored |
| Failed rollback | New work and account switches blocked |
| Concurrent switches or work arriving during activity inspection | Refused/cancelled before unsafe authentication |
| Pre-existing hidden active thread, approvals, opaque queue notifications | Switching blocked |
| API-key adoption during live attachment | Refused before changing the original credential store |
| Fragmented UTF-8, batched frames, detachment | Original reader receives each frame once; methods restored |
| Invalid attachment framing or ambiguous backend processes | Attachment/switching fails closed |
| Standalone add/import and shared account discovery | Passed without desktop app |
| Concurrent metadata updates, leases, credential locks | No lost records; active accounts protected across processes |
| Fresh helper removes a persisted account | Passed with credential rollback support |
| Project selection on later startup | Restored; source configuration/auth unchanged |
| Incorrect private socket token; backend termination | Rejected; switching disabled after disconnection |
| Production desktop binary in live Hyprland | Tiled before Code opens, after launch, and after Code closes |
| Dependency audit and Rust formatting | Zero npm advisories; formatting passed |

The launcher fix removes its forced floating restore state and verifies compositor state after mapping. Its previous one-shot startup operation could race GTK's maximize request. Live checks used disposable native app data and Code profiles. The existing running launcher also had its floating state corrected without restarting it. A rebuilt optimized desktop binary is available locally at `tauri/src-tauri/target/release/multi-codex-desktop`; the installed AppImage was not replaced or republished.

The Codex tests use synthetic credentials and loopback authentication/usage/model fixtures. A disposable official-extension copy receives a fetch shim and existing-user onboarding flags; installed official-extension files and normal VS Code settings remain untouched. The backend and VS Code can still perform their own background checks. Cleanup terminates only processes whose home is inside that exact disposable fixture and removes its synthetic accounts. Runtime screenshots and detailed reports are ignored by Git and excluded from the VSIX.

Live OAuth, real refresh-token rotation, cloud conversations, organization permissions and real quota enforcement need separate verification. Codex does not expose every queue held in the UI; clear UI-local queued follow-ups before switching. Unknown/opaque queue notifications block switching conservatively. Live attachment supports ChatGPT accounts; API keys require the isolated wrapper on a later normal startup. Unsupported or ambiguous host builds are refused without automatically reloading VS Code. macOS, Windows, remote environments and other Codex versions are not certified by this result. Older installed desktop releases do not understand extension account leases; use the updated desktop source when editing shared accounts concurrently.

The original work was split into three source commits. Marketplace publishing was deferred at that stage; publication was subsequently authorized under the existing publisher `russeldanielpaul`. See [installation and compatibility](../extension/README.md), [summarized local test evidence](../extension/verification.json), and [publishing requirements](extension-publishing.md).

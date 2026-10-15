# Latest repair amendment — 15 October 2026

The owner now authorizes execution of this plan using disposable fixtures and expands Orbit to Desktop, CLI and VS Code. This amendment supersedes the older Desktop-only and plan-only statements below. Preserve all real logins, chats and active clients. Local launcher installation remains **UI Update button only**. Do not commit until the owner has classified the concrete candidate as stable or preview; CI success does not establish owner acceptance.

## Revised acceptance criteria

1. **Real overlay on Hyprland:** use a compositor overlay/layer surface rather than a tiled application window. Orbit must not trigger layout changes, window switching, focus stealing or whole-screen repaint/flicker on hover. No blank input region may block the desktop. Fixed account slots, bounded panel and monitor-safe placement; drag must still work.
2. **Open UI is different from loaded backend:** the screenshot shows one actually open chat while the old observer reports three retained threads. History, backend caches, worker threads and closed tabs must not become additional Orbiters. Record the source and confidence of presence. Unknown UI visibility must stay unknown; do not fabricate an open-chat count from recent activity or open file descriptors.
3. **Avatars at every slot:** one plush avatar for one session; two diagonal small avatars for two; three/four in corners. Keep a reachable overflow avatar/count for more. No numbered 1/2/3 replacement buttons. With the filter off, closed accounts stay visible, dim and motionless; open accounts move subtly when animation is enabled. Respect reduced motion.
4. **Recoverability and opacity:** keyboard shortcut, tray and Settings must independently restore a hidden Orbit. Hovered strip and details are fully opaque; configured resting opacity must not be multiplied by compositor inactive-window opacity. No native tooltip should cause extra transient windows.
5. **Tray icon:** contrast on light/dark panels and visible hit area, including the reported Hyprland top bar; verify image content as well as menu actions.
6. **Update channels and human classification:** Settings offers Stable updates / All updates (including previews). Major/minor/patch version numbers do not imply stability. The app update feed and website release listing must use explicit stability metadata. Stable excludes previews; All receives both; no downgrade, unsigned payload or channel race during download. v1.7.0 is owner-reported unstable. Ask for classification **before every commit**, after presenting its tested result. No automatic stable publication. The owner explicitly classified v1.7.1 and its testing commits **Stable** on 15 October 2026; publication still requires passing candidate checks.

## Execution sequence and evidence

- Establish failing real-world reproductions and preservation hashes; audit the existing cases below, retaining their IDs.
- Fix overlay, geometry, hover and hide/restore first. Native acceptance must precede the requested animation change.
- Test exact visibility against the real Desktop UI: visit A, switch to B, close A, leave backend caches loaded, then open a second window/tab. Only genuine open UI sessions count. Repeat with CLI idle/working/exit and VS Code current/closed/editor-switch cases. Do not substitute a raw app-server process for an actual editor claim.
- Implement mini avatars and inactive appearance after the reliability lane passes. Check 0/1/2/3/4/5/20 sessions and 1/7/25 accounts at min/default/max size, light/dark and reduced motion.
- Test update-channel selection against a local fake release feed with real signed updater fixtures: stable-only, previews, newer stable after preview, same/lower versions, malformed data, incompatible targets, signature failures, channel change during download, offline/retry and persisted preference. Exercise the actual Update UI for local installation after an authorized published candidate exists.
- Run applicable existing P0/P1 cases below. Native Mac/GNOME/KDE/Windows results must come from the candidate source and be labeled separately; unavailable environments are BLOCKED, never converted to passes by unit tests.
- Present precise results and remaining gaps to the owner, ask Stable or Preview before committing, then follow the selected release policy. Keep detailed execution evidence, failures, repairs and pending actions in ignored local-docs; never upload private account screenshots or credentials.

## Current result (15 October continuation; local evidence, not release acceptance)

- PASS: actual two-account Desktop UI excludes backend caches, survives sidebar hiding, removes presence when leaving the chat, and preserves the full transcript on warm/cold reopen. Production ProfileService warm routing runs with every credential-store operation forbidden. Observer termination leaves Desktop and its chat alive.
- PASS: actual official VS Code UI submission is counted, a retained backend history read does not replace it, and New Chat removes it. Prewarmed starts are excluded; visible idle selection survives engine eviction; generation guards reject stale selection replies. The selected account and single submitted native turn are asserted.
- PASS: native CLI active/idle/exit and authoritative existing-bridge publication/unload; raw app-server descriptors deliberately do not establish UI presence.
- PASS: actual mini avatars, diagonal/corner/overflow layout, gray/static inactive accounts, reduced-motion/filter UI, persistent release-channel preference, stale-channel result rejection, installation lock, signed-package shape checks, and stable/preview website filtering. Local suites: UI119, core173, native176 (overlap), extension67, website62; skips remain explicitly documented.
- Hyprland final fixed-artifact evidence: `local-docs/resume-layer-final-result.json` and its log cover top/bottom overlay bounds, sustained hover, real pointer drag/persistence, focus, repeated hide/restore, fullscreen and inert auth/catalog preservation. Read the actual result before marking individual complete case matrices passed.
- OPEN: current candidate native Mac/GNOME/KDE/Windows, signed production packages/real Update UI, actual tray-host contrast, mixed monitors/scaling, accessibility/performance/soak and unexecuted scale matrices. Earlier v1.7.0 CI is historical evidence only. Owner classification remains required before every commit.
- Detailed commands, failed attempts/repairs, environment versions, exact artifact JSON and remaining scope are in ignored `local-docs/resume-validation.md`. The broad original case table below remains a contract; passing a narrower local slice does not mark an entire four-platform P0/P1 row complete.

---

# Orbit (HUD) comprehensive testing plan

Date: 15 October 2026 (Asia/Calcutta)
Status: **Plan only — the cases below have not been executed by this document-creation task.**
Scope: Multi Codex Orbit, its account Orbiters, local Codex Desktop observer/proxy, native adapters, settings, navigation and tray lifecycle.

## 1. Purpose and acceptance contract

Prove Orbit displays trustworthy live Desktop state, returns the user to the exact account/chat, and preserves credentials, active conversations and default login. Use the confirmed `local-docs/hud-tray-plan.md` as behavior input and current source as implementation evidence. Existing private historical reports are leads to revalidate, not blanket current-version certification.

This task creates a plan only. It does not authorize running destructive fixtures against live profiles, installing/updating the user's launcher, restarting active clients, publishing releases or dispatching hosted jobs. Later execution must remain within its authorized scope. Use disposable roots and accounts. Current user-facing name is Orbit; source commands/files still use `hud`.

Required native acceptance: Apple Silicon macOS 26+, Hyprland, GNOME and KDE. Test Windows for launcher regression, not as an Orbit support target. CLI, VS Code, remote sessions, in-HUD approval/reply controls, avatar picking and historical usage charts are outside Orbit scope.

## 2. Priorities, results and release rules

- **P0:** wrong account/chat, credential/history mutation, cross-account leak, protocol corruption or unrelated native-window manipulation. Any failure blocks acceptance.
- **P1:** required feature or significant reliability/accessibility behavior. Any unresolved failure blocks a full-support claim; remediation or an explicit approved scope change is required.
- **P2:** cosmetic or secondary polish; record impact and disposition.
- Results: NOT RUN, PASS, FAIL, BLOCKED or NOT APPLICABLE. Missing environment is BLOCKED, never PASS. An expected failure remains FAIL with an issue, not a waived pass.
- Each case needs fixture/version, action, expected outcome and evidence. Repeat every native P0/P1 case applicable to each required desktop. Unit/browser/VM contract tests cannot certify native focus, fullscreen, tray, audio, packaged updating or compositor behavior.

## 3. Test environments and fixtures

| Lane | Required setup | What it establishes |
| --- | --- | --- |
| Unit/UI | Pinned repository dependencies, Vitest/jsdom, Rust core tests | Deterministic reducer, identity, settings and UI interaction contracts |
| Protocol | Real supported engine with local synthetic service and disposable homes | Byte forwarding, lifecycle, attention, privacy and exit semantics |
| Native Desktop | Packaged Desktop, two distinct synthetic accounts, multiple windows/workspaces | Exact transcript routing, process isolation and warm/cold reopening |
| macOS | Apple Silicon macOS 26+; record exact OS/Desktop/build | AppKit/Spaces, fullscreen per monitor, shortcuts, menu and lifecycle |
| Hyprland | Actual Wayland session; record compositor version/config | Structured compositor operations, pin/float, maximized vs fullscreen |
| GNOME | Actual supported Shell/Mutter session and enabled app bridge | Chosen-output fullscreen, extension lifecycle and activation |
| KDE | Actual supported Plasma/KWin session and installed app bridge | Scoped keepAbove, persistent shortcut and workspace activation |
| Packaging | Exact candidate native artifacts and prior supported version | Resources, signed updater payload and production GUI update |
| Regression | Windows native launcher; other existing app suites | Existing account/backend/update flows remain intact |

For Linux record distro, Wayland/X11, compositor/bridge version and tray host. A nested compositor run is useful but label it separately from a normal logged-in desktop. Test each claimed display-server mode; do not infer X11 from Wayland or vice versa. Record the actual Desktop engine version; a second newer version is compatibility sampling, not a claim of universal compatibility.

Fixtures: A/B accounts with distinct auth canaries and same display name; C with no open chat; D without quota. Give A/B distinct marker transcripts and a shared-history thread. Create 0/1/2/3/4/5/20 loaded-chat sets plus 100 history-only entries. Include working, idle, question, approval, error and inferred human task. Use no real credentials. Test 1/6/25-account catalogs, missing settings, corrupt files, restarted PID, duplicate events, absent monitor and unavailable bridge.

Before native work capture fixture-owned process IDs, auth/catalog/default-home hashes and desktop geometry. Restrict cleanup to roots and PIDs created by the harness. Never kill all processes by application name or reuse production account roots. Credential-only backups are not a full conversation rollback. Use fixture-local disk limits for ENOSPC, never fill the user's disk. End with preservation and cleanup assertions even when a case fails.

## 4. Detailed test cases

Run each row from a clean stated fixture unless it explicitly tests continuity. Capture actual values, not only screenshots. The expectation is the acceptance contract; discrepancies in current implementation are defects to record.

### Account identity and layout

| ID | Priority | Action / setup | Expected result |
| --- | --- | --- | --- |
| ID-01 | P1 | Start with zero accounts | Show a usable empty state; no invented account, crash or sign-in action. |
| ID-02 | P1 | Add one disposable account, restart, rename it and change theme | The automatically assigned Orbiter remains identical; no picker is required. |
| ID-03 | P1 | Load existing accounts without prior Orbit settings | Backfill stable identities without credential, login or history changes. |
| ID-04 | P1 | Compare frontend and native identity for 100 synthetic public account IDs | Kind, color and seed agree exactly, including Unicode and long IDs. |
| ID-05 | P1 | Load 1, 2, 6, 12 and 25 accounts | Every account remains reachable; strip and popovers stay within usable display bounds. |
| ID-06 | P1 | Give one account 0, 1, 2, 3, 4, 5 and 20 loaded chats | 0–1 uses avatar; 2–4 uses dots within equal account slot; 5+ uses three dots and correct +N. |
| ID-07 | P1 | Mix waiting, working and idle chats then reorder incoming snapshots | Attention sorts first; displayed target IDs remain correct and source arrays are not mutated. |
| ID-08 | P1 | Open overflow and select first, middle and last hidden chat | Every overflow chat is accessible and opens its exact target. |
| ID-09 | P1 | Toggle only-open filter while chats load/unload | Only accounts with loaded local Desktop chats remain; disabling filter restores all accounts. |
| ID-10 | P2 | Use duplicate names, emoji, RTL, very long names and HTML-like labels | Labels stay readable and escaped; IDs rather than names determine ownership. |

### Live session telemetry and passive proxy

| ID | Priority | Action / setup | Expected result |
| --- | --- | --- | --- |
| EV-01 | P0 | Replay identical engine RPC stream with proxy enabled and disabled | Requests and responses remain byte-identical; engine behavior and exit status agree. |
| EV-02 | P0 | Fragment Unicode JSON, split delimiters, send >2 MiB lines and rapid interleaved replies | No corruption, truncation, deadlock or cross-request contamination. |
| EV-03 | P0 | Run version/non-app-server commands through proxy | Arguments, streams and exit codes pass through without creating fake sessions. |
| EV-04 | P1 | List 100 historical chats without loading them | No history rows become live dots. |
| EV-05 | P1 | Start, resume, load and unload real fixture chats | Only loaded chats appear; idle loaded chats stay counted; unload removes the correct chat. |
| EV-06 | P1 | Replay working, idle, completed, cancelled, error and interrupted turns | States converge to native truth; completion alone does not imply needs-user. |
| EV-07 | P1 | Open two Desktop processes/windows for one account | Distinct loaded chats aggregate without double counting identical account/thread identity. |
| EV-08 | P0 | Use same thread ID under two disposable accounts | Statuses, receipts, quotas and routing remain scoped to account plus session. |
| EV-09 | P1 | Kill fixture engine, reuse PID with different start time, leave stale snapshot | Stopped/reused process never appears as a live session. |
| EV-10 | P1 | Lose telemetry directory access, corrupt one snapshot or disconnect engine | No fabricated healthy state; unrelated accounts remain usable; recovery resumes cleanly. |
| EV-11 | P1 | Deliver duplicate, late and reordered lifecycle messages | Old state does not resurrect unloaded chats or overwrite a newer attention epoch. |
| EV-12 | P1 | Launch CLI, VS Code and remote tasks alongside Desktop | Only local Desktop sessions contribute to Orbit. |
| EV-13 | P1 | Enable Orbit while an already running unwrapped Desktop stays open | No forced restart; unavailable telemetry is treated honestly until a managed launch. |
| EV-14 | P0 | Flood stderr and slow snapshot writes while engine responds | Observer failure/backpressure does not block or modify the real conversation. |

### Attention, acknowledgment and reminders

| ID | Priority | Action / setup | Expected result |
| --- | --- | --- | --- |
| AT-01 | P1 | Trigger native question, approval, error and human-task reply separately | Distinct status and accessible description; badge remains until correct acknowledgment. |
| AT-02 | P1 | Use human-task corpus: login, token update, API-key check, ambiguous request, negation and ordinary completion | Record true/false positives and negatives; ordinary completion never automatically equals a user task. |
| AT-03 | P1 | Poll same event 20 times and rerender HUD | One brief popup per event; persistent badge remains; no repeated sound. |
| AT-04 | P1 | Create a new question on same thread after acknowledgment | New epoch/event key produces a fresh badge and one reminder. |
| AT-05 | P0 | Inspect popup with secret canaries in title and message | Popup contains only avatar and status icon; no title, excerpt, path or secret. |
| AT-06 | P1 | Leave question waiting for 10 minutes | Popup expires; badge remains and actual native question is not dismissed. |
| AT-07 | P1 | Successfully click waiting chat through Orbit | Only that event badge clears; native approval/question still requires action inside Desktop. |
| AT-08 | P1 | Fail navigation by removing fixture executable or permission | Badge stays; actionable error appears and retry works. |
| AT-09 | P1 | Read history, fetch usage or focus another account | No unrelated badge is acknowledged. |
| AT-10 | P1 | View exact waiting chat through Desktop rather than Orbit | Check confirmed requirement that opening chat clears badge; if exact-view receipt is unavailable, record explicit product gap, not a pass. |
| AT-11 | P1 | Restart renderer, restart launcher and receive >4096 distinct events | Check receipt durability and bounded storage separately; old unresolved events do not cause uncontrolled reminder replay. |
| AT-12 | P1 | Trigger simultaneous attention on 10 chats | Every badge survives; document popup scheduling policy and detect silently lost reminders. |
| AT-13 | P1 | Keep default sound off, enable sound, mute OS, deny audio initialization | Default is silent; enabled sound is once per new event; visual reminder survives audio failure. |
| AT-14 | P1 | Disable Orbit while a popup/sound is pending, then re-enable | No hidden popup or delayed sound leaks through disable; chats continue unchanged. |

### Exact account/chat navigation

| ID | Priority | Action / setup | Expected result |
| --- | --- | --- | --- |
| NAV-01 | P0 | Click account A chat while B is foreground and both have similar titles | A exact transcript comes forward; B account, chat, window and credentials remain unchanged. |
| NAV-02 | P0 | Navigate warm/cold with two accounts sharing one historical chat | Use current runtime ownership and selected account, never historical source account alone. |
| NAV-03 | P0 | Close target between snapshot and click | Reopen same thread under correct saved account; no duplicate account instance. |
| NAV-04 | P0 | Click rapidly 10 times, including during cold startup | Launches are serialized/coalesced; no duplicate clients or mixed thread routes. |
| NAV-05 | P1 | Click zero-session account | Open its Desktop window without selecting arbitrary historical chat. |
| NAV-06 | P1 | Click idle avatar, working avatar, each dot and overflow row | Every affordance routes to the identity it displays. |
| NAV-07 | P0 | Navigate between multiple windows/workspaces/displays of same account | Activate exact target window/workspace without moving other windows. |
| NAV-08 | P0 | Try invalid, deleted and mismatched account/thread IDs through native command | Reject safely; never fall back to default account or another chat. |
| NAV-09 | P0 | Use paths with spaces, Unicode, symlinks and similar executable names | Verify canonical executable and account user-data identity before warm routing. |
| NAV-10 | P1 | Remove account while its popover is open | Stale action fails cleanly; no orphan launch or stale badge acknowledgment. |
| NAV-11 | P1 | Navigate archived/deleted/unavailable target in fixture | Explain failure and preserve history; no synthetic replacement conversation. |
| NAV-12 | P0 | Record account and process identities before/after all routing tests | Unselected accounts keep exact auth bytes and running sessions; default login remains untouched. |

### Hover, usage and accessibility

| ID | Priority | Action / setup | Expected result |
| --- | --- | --- | --- |
| UI-01 | P1 | Hover avatar and each session dot, then move pointer into popover | Usage and intended session details appear; popover stays usable while pointer enters it. |
| UI-02 | P1 | Tab/Shift+Tab through accounts, dots, overflow and session rows; Enter/Space activates | Equivalent keyboard navigation, visible focus and correct target; no keyboard trap. |
| UI-03 | P1 | Press Escape in popover/error/settings then resume focus | Dismiss correct surface and restore sensible focus without unintended navigation. |
| UI-04 | P1 | Feed known 0%, 37%, 100% used windows and reset timestamps | Remaining equals 100 minus used; units/window labels/reset timezone are correct. |
| UI-05 | P1 | Omit limits, token counts, context window or reset values | Show unavailable/omit value; never invent zero, full quota or token usage. |
| UI-06 | P1 | Mix account-level quota with different per-session token totals | No cross-account or cross-session attribution; context-window capacity is not mislabeled remaining quota. |
| UI-07 | P1 | Fail fresh usage request with cached data then recover | Fallback is honest about freshness; cache never overrides newer valid data or leaks deleted accounts. |
| UI-08 | P1 | Send malformed, negative, out-of-range, NaN-like and huge metrics | Reject or safely display unavailable; no nonsensical progress or renderer crash. |
| UI-09 | P2 | Test all six palettes, light/dark and system theme changes | HUD follows chosen theme across windows without changing identities. |
| UI-10 | P1 | Enable OS reduced motion and explicit Reduce motion; disable working animation | No unnecessary motion; working status remains discernible without movement. |
| UI-11 | P1 | Inspect accessible names with screen reader and high contrast/200% text sizing | Account/session/state/overflow and controls are distinguishable; status is not color-only. |
| UI-12 | P2 | Use minimum size, opacity and display-edge popovers | No clipped action, invisible focus, offscreen details or unusably small interactive target; log conflicts. |
| UI-13 | P1 | Inject snapshot timeout, command rejection and malformed receipt storage | UI recovers without unhandled loop; native errors are readable and do not expose secret content. |

### Settings and persistence

| ID | Priority | Action / setup | Expected result |
| --- | --- | --- | --- |
| SET-01 | P1 | Disable via main Settings then independently via Orbit settings | Orbit disappears including popup/sound; main launcher and chats remain usable. |
| SET-02 | P1 | Save Small 40, Medium 56, Large 76, custom 32/57/100 px | Preset selection and custom value agree; all slots and native window geometry scale consistently. |
| SET-03 | P1 | Save opacity 35%, 95% and 100% then restart | Correct persisted opacity, including child surfaces. |
| SET-04 | P1 | Save monitor/filter/animation/reduced-motion/sound with nondefault position | All values survive restart; changing enabled alone preserves other preferences. |
| SET-05 | P1 | Cancel after edits; close/reopen settings | Unsubmitted changes never persist. |
| SET-06 | P1 | Delay Save, double-submit, attempt Cancel and concurrent setting changes | No duplicate/conflicting writes; save state and final committed values are accurate. |
| SET-07 | P1 | Deny settings read/write, fill fixture disk, interrupt atomic save | Recoverable error; last valid file remains intact; no false saved confirmation. |
| SET-08 | P1 | Supply truncated JSON, unknown fields and old/missing settings | Safe validated defaults/migration; no credential reset or source data destruction. |
| SET-09 | P1 | Submit size 31/101, opacity below .35/above 1, invalid coordinates and nonfinite values | Native boundary validates independently of sliders and rejects invalid state safely. |
| SET-10 | P1 | Reset position then change selected display | Return to chosen display top-right; stale coordinates clear. |
| SET-11 | P1 | Toggle enabled/hide settings while poll/save/restart occurs | Final persisted preference wins; disabled Orbit does not unexpectedly reappear. |

### Native window, monitor and fullscreen behavior

| ID | Priority | Action / setup | Expected result |
| --- | --- | --- | --- |
| WIN-01 | P1 | Start fresh on supported desktop | Compact HUD begins top-right on chosen display without stealing typing focus. |
| WIN-02 | P1 | Drag via handle, click avatar and resize main launcher | Only handle drags; clicks still navigate; launcher maximize never targets HUD. |
| WIN-03 | P1 | Place ordinary app above/behind, switch workspaces and show desktop | HUD stays above ordinary windows with intended workspace visibility; other apps are not reconfigured. |
| WIN-04 | P1 | Enter/exit actual fullscreen on selected output | Hide during fullscreen and restore automatically afterward without focus theft. |
| WIN-05 | P1 | Maximize ordinary window without fullscreen | HUD remains visible; Hyprland maximized state is not treated as real fullscreen. |
| WIN-06 | P1 | Fullscreen other monitor; minimize fullscreen app; switch its workspace | Only visible fullscreen on chosen display affects HUD. |
| WIN-07 | P1 | Manually hide, enter/exit fullscreen and restart compositor bridge | Manual hide is not undone by automatic fullscreen restoration. |
| WIN-08 | P1 | Use Command/Super+Shift+H twice while another app is focused | Toggle exactly once per press; correct registration and no focus transfer. |
| WIN-09 | P1 | Cause shortcut conflict or remove bridge support | No overriding user binding; actionable limitation; unsupported HUD remains hidden without substitute. |
| WIN-10 | P1 | Use monitors with negative origins, portrait mode and mixed 100/150/200% scaling | Remember position in correct coordinate system; popover and drag boundaries remain valid. |
| WIN-11 | P1 | Disconnect chosen monitor, dock/undock, change primary and reconnect | HUD stays recoverable on available display; no invisible offscreen permanent placement. |
| WIN-12 | P1 | Change resolution/scale while expanded or dragging | Clamp/reposition coherently; no endless geometry oscillation. |
| WIN-13 | P1 | Sleep/wake, lock/unlock and restart compositor integration | Restore healthy state with no stale fullscreen/monitor/process identity. |
| WIN-14 | P1 | Start unsupported compositor or disable required integration | No fallback overlay; launcher/account management still works. |
| WIN-15 | P0 | Place unrelated window with same caption or PID-like metadata nearby | Platform mutations require actual HUD identity and never move/resize/hide unrelated windows. |

### Tray and launcher lifecycle

| ID | Priority | Action / setup | Expected result |
| --- | --- | --- | --- |
| LIFE-01 | P1 | Close launcher main window while a fixture chat works | Tray and HUD remain in same intended launcher process; chat keeps working. |
| LIFE-02 | P1 | Reopen main window from tray six consecutive times | Same process and one window; no duplicate tray/observer or event handlers. |
| LIFE-03 | P1 | Choose each account from tray including closed account | Correct account launches/focuses without changing default login. |
| LIFE-04 | P1 | Use tray Settings, usage refresh and Orbit show/hide | Actions update correct window/state without opening accidental chats. |
| LIFE-05 | P1 | Choose tray Quit with HUD/popover/settings open | Launcher/HUD/tray/owned shortcuts exit; independent Desktop chats keep running. |
| LIFE-06 | P1 | Use normal platform application Quit and start a second launcher | Quit semantics agree; singleton behavior prevents duplicate HUDs/trays. |
| LIFE-07 | P1 | Run Linux without tray host, then restart tray host | No inaccessible background-only trap; limitations and recovery are explicit. |
| LIFE-08 | P1 | Crash renderer or kill fixture launcher while chat runs, then relaunch | No credential loss; observer/runtime recovery and stale cleanup are safe. |
| LIFE-09 | P1 | Start with Orbit disabled and close/reopen launcher | Disabled stays disabled; tray control does not silently overwrite unrelated settings. |

### Privacy, isolation and security

| ID | Priority | Action / setup | Expected result |
| --- | --- | --- | --- |
| SEC-01 | P0 | Insert unique fake secrets in prompts, responses, questions and auth files; exercise every state | No canary in HUD snapshots, receipts, popup, logs, screenshots or exported evidence; explicit named chat metadata is reviewed separately. |
| SEC-02 | P0 | Inspect persisted telemetry schema and file permissions | Allowlisted identity/state/usage metadata only; no message previews, request bodies, credentials or answers; private filesystem access. |
| SEC-03 | P0 | Compare protected account/catalog/auth/default-home hashes before/after suite | No unexpected changes; intentional fixture changes documented separately; no real account mutation. |
| SEC-04 | P0 | Replace fixture HUD paths with symlinks and malformed oversized files | No external-file overwrite, path traversal or unsafe follow; reads/writes fail safely. |
| SEC-05 | P0 | Invoke privileged HUD commands from unintended renderer or with hostile identifiers | Capability/argument boundaries restrict access; no arbitrary executable/path opening. |
| SEC-06 | P0 | Capture fixture outbound connections during idle, telemetry and navigation | No new analytics/secret exfiltration; expected native service traffic distinguished from Orbit. |
| SEC-07 | P1 | Inspect process argv, environment diagnostics and crash evidence | Secrets never copied into reports; collection uses allowlist and redaction. |
| SEC-08 | P1 | Delete disposable account and restart | Its visible cached data/receipts no longer cross into other identities; retention is documented. |

### Reliability, performance and upgrade

| ID | Priority | Action / setup | Expected result |
| --- | --- | --- | --- |
| REL-01 | P1 | Measure idle with Orbit off/on at 1, 6 and 25 accounts | Report CPU, RSS, wakeups and disk writes against same-host baseline; investigate sustained growth. |
| REL-02 | P1 | Send 100 events/sec for 60 sec with 50 loaded sessions | UI responsive; final state correct; proxy throughput does not regress materially. |
| REL-03 | P1 | Run 8-hour soak alternating working/waiting/idle and 100 hover/navigation cycles | No progressive listener/timer/audio-context/file/process leak; no duplicated reminders. |
| REL-04 | P1 | Block snapshot response beyond 1.5-sec poll interval then recover | One in-flight refresh; no unbounded backlog; UI converges after recovery. |
| REL-05 | P1 | Run disk full/read-only telemetry, malformed message, engine EOF and bridge timeout independently | No deadlock or data loss; accurate error/absence and bounded retries. |
| REL-06 | P1 | Upgrade disposable packaged previous version to candidate using production Update button | Settings, identities, logins and chats preserved; progress/retry/restart real; no direct RPC or file-copy substitute. |
| REL-07 | P1 | Fail/cancel fixture update and retry with valid signed payload | Prior installation remains usable; updater reports actual outcome and preserves state. |
| REL-08 | P1 | Run first launch after upgrade twice and reopen version notes | Minimal Orbit notes once after successful receipt; dismissal persists; version label correct. |
| REL-09 | P1 | Test clean supported package install plus portable launch where distributed | Bundled avatars/helper/platform bridge work without developer checkout or local dev server. |
| REL-10 | P1 | Run Windows launcher with Orbit unavailable | Existing Windows account/tray/update behavior remains functional; no accidental Linux/macOS support claim. |

## 5. End-to-end acceptance journeys

1. **First use:** clean disposable install → add A/B → confirm permanent distinct Orbiters → open A working and B idle → hover usage → close launcher → tray reopen → explicit Quit. Verify all auth hashes and Desktop processes.
2. **Attention:** A requests approval while B asks a question → verify badges and icon-only silent popup → wait for popup expiry → click B → exact transcript → B badge clears only → answer inside Desktop → A remains waiting → new B question can alert again.
3. **Account ownership race:** open same shared-history chat under A/B → B foreground → click A → cold-close A during next click → verify A reopens same transcript once, B stays unchanged and default login remains intact.
4. **Display recovery:** select external monitor → drag → fullscreen there → manually hide → exit fullscreen → remain hidden → unplug → show via tray → sleep/wake → restore usable geometry with no focus theft.
5. **Failure and recovery:** remove fixture bridge → safely hide HUD → restore bridge → corrupt one observer snapshot → unaffected account remains usable → restart fixture process → stale snapshot ignored → quota/attention converge without duplicate reminders.
6. **Packaged upgrade:** prior disposable installation with custom Orbit settings and active synthetic chat → candidate advertised via production updater → click real UI Update button → observe progress/restart → same identities/settings/auth/transcript → release notes once. A signed SDK harness is separate supporting evidence, not a replacement for this journey.

Execute journeys 1–5 on every required native platform and journey 6 for each distributed updater target. Repeat routing and lifecycle six consecutive times to expose stale process/window references.

## 6. Existing automation and proposed additions

| Existing asset | Reuse for | Remaining proof required |
| --- | --- | --- |
| `tauri/src/Hud.test.tsx` | Hover/click, slot overflow, private popup, disabled Orbit | Native focus, race/error cases, accessibility, receipt durability |
| `tauri/src/App.test.tsx` | Settings enable switch and release-note receipt | Real settings persistence and production Update button |
| `cli/crates/core/src/hud.rs` tests | Stable identity/settings, live vs history, minimized snapshots, stale PID | Full event corpus, crash/fault/performance coverage |
| `extension/tests/hud-native.test.mjs` | Actual engine/proxy fixture | Actual rendered Desktop and native compositor behavior |
| `tauri/scripts/hud-platform.test.mjs` | GNOME/KDE adapter contract and KDE bundle parity | Actual native integration on each desktop |
| `tauri/scripts/test-hud-desktop.mjs` | Two-account native transcript routing | Current artifact/version plus broader races/workspace cases |
| `tauri/scripts/test-hud-window.mjs` | Isolated Hyprland launcher/HUD | Broader display, keyboard and recovery matrix |
| `tauri/scripts/test-hud-compositor.py`, `test-hud-gnome-session.sh` | Native/nested compositor fixtures | Document exact runtime and avoid substituting nested for physical session coverage |
| `.github/workflows/hud-backend-validation.yml` | Hosted Mac/native backend/package checks | Inspect artifact SHA, skips and native assertions for each run |
| `.github/workflows/hud-linux-validation.yml` | GNOME lane | KDE/Hyprland and real multi-monitor evidence |
| `.github/workflows/macos-routing-validation.yml` | Focused Mac exact-chat routing | Affected target OS, foreground focus and multiple physical displays |

Add focused tests only where they establish missing behavior: account-scoped event receipts, failed-open acknowledgment, repeated events, simultaneous attention, corrupt storage, live/history boundaries and exact navigation races. Avoid tests that merely repeat implementation syntax. Keep native UI and protocol assertions separate so one cannot mask the other's failure.

### Verified command entry points

These commands are execution guidance, **not results**. Run from repository root unless shown otherwise. Dependencies/toolchain must be prepared first. Native commands require deliberate isolated fixture setup; inspect scripts before using them. Do not run all examples blindly against an active user session.

```bash
# UI and adapter contracts
npm --prefix tauri test -- src/Hud.test.tsx src/App.test.tsx
node --test tauri/scripts/hud-platform.test.mjs

# Focused Rust HUD tests, followed by broader affected native suite
cargo test --locked --manifest-path cli/Cargo.toml -p multi-codex-core hud::tests
cargo test --locked --manifest-path tauri/src-tauri/Cargo.toml

# Frontend type checking + production bundle
npm --prefix tauri run build

# Native protocol fixture: supported real engine and isolated setup required
node --test extension/tests/hud-native.test.mjs

# Native Desktop routing: configure exact fixture executables/helper/navigation first
node tauri/scripts/test-hud-desktop.mjs

# Hyprland native window fixture: run from tauri in suitable disposable session
(cd tauri && node scripts/test-hud-window.mjs)
```

Native routing configuration includes `MULTI_CODEX_TEST_STANDALONE`, `MULTI_CODEX_TEST_ENGINE`, `MULTI_CODEX_TEST_SHARED_HELPER`, `MULTI_CODEX_TEST_HUD_NAVIGATION` and `HUD_GUI_OUTPUT`. Hyprland window fixture can use `MULTI_CODEX_TEST_LAUNCHER`. Confirm current scripts and workflow setup for build paths and fixture prerequisites. Never point these to production profiles or log a full inherited environment.

## 7. Performance measurements and proposed budgets

These are initial acceptance targets, not existing measurements or previously confirmed product promises. Record hardware and baseline; review an unattainable target explicitly before changing it.

| Measurement | Proposed threshold / method |
| --- | --- |
| Event to visible state | p95 ≤3 seconds under normal load; timestamp source event and rendered state, ≥100 samples |
| Warm navigation | p95 ≤2 seconds to verified target transcript and foreground focus; ≥30 attempts/platform |
| Cold navigation | p95 ≤15 seconds on reference hardware; separate engine/app startup from Orbit routing |
| Idle CPU | Target ≤1% total-system CPU average over 10 minutes at six accounts; report core-normalization method |
| Memory stability | After warm-up, no monotonic growth; 8-hour RSS increase ≤20 MiB absent explained cache growth |
| Rendering | No repeated >100 ms UI stalls during normal hover, filtering or drag; collect native webview trace |
| Protocol overhead | Compare p50/p95 latency and throughput with proxy off/on; target <5% throughput loss and no dropped bytes |
| Resource cleanup | No fixture-owned orphan process, socket, shortcut registration, audio context or leaked file handle |

Measure off/on in alternating order at least three times. Include disabled Orbit baseline and event-burst peak. Timing flaky under overloaded CI is a diagnostic result; reproduce on controlled native hardware rather than silently raising thresholds.

## 8. Execution order and tracking

1. Freeze candidate source SHA, lockfiles, built helper hashes and native artifact hashes. Review confirmed behavior and open gaps.
2. Prepare isolated fixture roots and preservation manifest. Verify fixture cannot address production account roots.
3. Run deterministic UI/core/platform contracts. Resolve P0 failures before launching native clients.
4. Run protocol and two-account native routing; verify byte fidelity, transcript markers and preservation after each journey.
5. Run required platform window/tray/fullscreen/display/shortcut matrix with explicit per-platform case results.
6. Run fault injection, accessibility, performance/soak and packaging/GUI upgrade acceptance.
7. Repeat affected regression cases after fixes; rebuild if source changed and bind evidence to new artifact hashes.
8. Complete preservation and cleanup audit; publish a factual result summary with remaining BLOCKED/FAIL cases.

Maintain a case ledger with one row per case × platform × configuration. Expand this template; a single global pass cannot stand for four native platforms.

| Case | Platform / config | Source + artifact SHA | Status | Actual observation | Evidence | Issue / owner | Retest |
| --- | --- | --- | --- | --- | --- | --- | --- |
| NAV-01 | macOS exact version | pending | NOT RUN | — | — | — | — |
| WIN-04 | GNOME exact version / Wayland | pending | NOT RUN | — | — | — | — |

Evidence bundle per run: redacted result JSON, fixture manifest, case ledger, exact OS/compositor/Desktop/engine versions, source/build/artifact hashes, commands and exit codes, assertion logs, screenshots or short video for visual/focus behavior, timing/resource samples, preservation digest comparison, and cleanup report. Store bulky/private evidence under ignored `output/` or `local-docs/`; do not commit secret-bearing captures. Automated assertions must establish target transcript plus account/process identity; screenshot alone is insufficient.

Defect report: case ID, severity, exact platform/version, source/artifact identity, minimal fixture, reproducible steps, expected/actual, redacted evidence, preservation impact, workaround, responsible owner and required retest. If teardown fails, report failure even when product assertions passed.

## 9. Known gaps to prove explicitly

- Exact-chat opening outside Orbit: confirmed behavior wants badge clearing; prior implementation notes say reliable external exact-view receipts were unavailable. Test and resolve or disclose this gap explicitly.
- Reminder receipts: renderer/session storage and durable badge acknowledgment are different mechanisms. Test full renderer and app restart, duplicate epochs, simultaneous events and receipt eviction separately.
- Broad human-task inference intentionally accepts some false alarms. Record a labeled corpus and miss/false-alarm counts; do not describe it as perfect semantic understanding.
- Existing running unwrapped Desktop processes do not gain proxy telemetry retroactively. No test should force-close a real client to make coverage pass.
- Huge account catalogs, scaling, keyboard popovers, missing tray host and monitor unplug behavior need real accessibility/usability acceptance rather than source inspection.
- Historical native reports may use older engine, OS or candidate versions. Reuse evidence only with explicit source/artifact equivalence and matching tested behavior; otherwise mark NOT RUN/BLOCKED.
- macOS version-specific issues require the affected actual OS. Passing a different hosted macOS release does not close that gap.

## 10. Completion checklist

- [ ] All P0 and P1 cases applicable to each required native platform have fresh PASS evidence.
- [ ] Native macOS, Hyprland, GNOME and KDE results are individually reported, including display-server limits.
- [ ] Exact account/thread routing, cold reopen and repeated navigation proven with transcript + process identity.
- [ ] No credential, default login, live-client or conversation regressions; preservation assertions pass.
- [ ] Privacy canaries absent from prohibited storage, popup and evidence outputs.
- [ ] Native fullscreen, chosen monitor, shortcuts, tray close/reopen/Quit and display recovery proven.
- [ ] Accessibility, failure recovery, performance and soak findings resolved or explicitly dispositioned.
- [ ] Production GUI update proven separately from SDK/package harnesses when upgrade is in scope.
- [ ] No unexplained skips, flaky reruns, teardown failures or evidence/source mismatches.
- [ ] Every gap has a concrete next step; report accurate scope instead of declaring universal support.

Planning is complete when this document is reviewed. Product testing is complete only when the case ledger and release rules above are satisfied.

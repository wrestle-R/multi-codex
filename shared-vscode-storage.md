# MultiCodex: shared VS Code extensions and cross-account chats

Updated: 12 October 2026 (Asia/Calcutta).
Status: implementation authorized; shared-generation, migration, broker, handoff, and GUI source paths exist locally; Linux native broker continuation passes. Local preservation/recovery checks pass; native platform, authentication, GUI, and release acceptance remain pending.

This replaces the earlier planning-only document. The root `shared-vscode-storage.md` is the canonical plan; do not maintain a second copy under `plans/`. Checked boxes below mean the stated work actually exists or the stated check actually passed. An unchecked box is required work, including checks that have not run. Source fixes, fixture results, native authentication, GUI acceptance, and released functionality are recorded separately.

## 1. Requirements and order of work

The first requirement is preservation of current profile data. Existing accounts, credentials, complete conversations, attachments, editor state, workspaces, Copilot configuration, and shortcuts must survive migration and an application update. Keep recoverable copies and the original layout until the replacement has been verified.

The requested product behavior is:

- Reuse the installed VS Code executable. Do not install another editor runtime for production accounts.
- Store supported Codex extension package bytes once and reuse them across account windows. Initialize a minimal Codex setup rather than importing the user's complete ordinary extension collection.
- Preserve Copilot's availability, enabled/disabled state, UI, and settings. Preserve separately installed Copilot packages where required; bundled Copilot remains part of the existing editor.
- Keep credentials, `CODEX_HOME`, native SQLite state, VS Code user data, settings, and account-specific tools private.
- Make the complete shared chat library visible to every saved account. Any account can continue a chat started by another account using its own verified identity, preserving the original context and logical chat.
- Support multiple account windows at once. Different chats can run concurrently. Use one writer per chat with observers and a safe handoff as the proposed implementation default.
- Open the Codex sidebar automatically when launching a managed account.
- Provide recoverable migration, storage accounting, maintenance, and explicit cleanup controls.
- Keep update discovery quiet on startup and user refreshes. Show the Update button when a newer signed release is available, with working download, installation, progress, and restart.
- Fix and verify the unexpected Windows terminal during browser sign-in.
- Offer the Windows download as the `.exe` installer in public download links.
- Verify Windows, macOS, and Linux on native operating systems. Use two real accounts for the requested Mac and Windows authentication/switching acceptance.

The user's sequencing and local-install instructions take precedence:

1. Complete shared-storage implementation and local disposable tests.
2. Then run GitHub native shared-storage, migration, authentication, packaging, and updater checks. Do not dispatch these runners while implementation is incomplete.
3. Address failures and rerun the affected checks against the revised source.
4. Prepare the next release only after the required evidence is complete.
5. Leave this PC's installed app at **1.5.1** throughout development. The user will test the new release through the Update button personally. Do not replace its executable, migrate its real profiles through a development build, or stop its active Codex/editor sessions to accelerate testing.

Older-backup deletion was separately authorized. The older pre-2.4.5 snapshot was removed and the current pre-sharing backup retained. That completed action is not permission to delete legacy extension folders, current profile data, chat sources, or the retained current backup.

## 2. Verified current state

### 2.1 Completed foundations and evidence

| Item | Actual state | Evidence and limits |
| --- | --- | --- |
| Quiet updater discovery | Shipped in 1.5.1 | Startup, account/all-account/menu refresh, visibility/reconnect, and a 15-minute visible-app interval check quietly. The internal two-second process poll does not trigger network checks. |
| Existing local updater test | Completed for 1.5.0 → 1.5.1 | Actual GUI/public-feed update passed previously; accounts remained unchanged. This does not verify the next sharing release or native Mac/Windows SDK installation. |
| Current safety backup | Completed and retained | All six profiles copied; 151,232 regular files hashed, 13,745,660,589 logical file bytes. Installed 1.5.1 AppImage and existing desktop entry were also copied with matching digests. |
| Backup consistency | Live safety copy only | Original data retained; copy command succeeded. Active sessions were not stopped, so this is not an application-consistent closed-profile snapshot. Fresh closed-profile snapshots are still mandatory before migration. |
| Older backup removal | Completed | Removed only the specifically identified older snapshot after confirming the retained copy contained all six profile IDs and matching profile metadata. |
| Windows sign-in console source fix | Committed, not released | Commit `a7c4500` routes device-login spawning through the background process helper, uses null stdin and a private SQLite home, and hides the native bridge's Node child. Actual Windows sign-in GUI observation remains pending. |
| Windows console regression | Added, not passed on Windows yet | Native test checks `GetConsoleWindow() == 0` for background helpers. A successful Linux build cannot establish this Windows result. |
| Shortcut refresh after update | Committed, not released | Commit `830427a` refreshes existing app-owned Linux shortcut version metadata after an in-place update. Seven disposable desktop-integration tests passed. Other platform integration requires native acceptance. |
| Backup/migration service | Integrated source, unreleased | Verified closed-profile snapshots, conservative space previews, completion markers, restore staging, JSONC-aware layout journals and rollback are wired to native commands and GUI. Uncovered external links block migration; dependency snapshots and full restore acceptance remain open. |
| Shared extension prototype | Linux fixture passed | A native uninstall through B's private linked catalog preserved A's catalog, running editor, and shared package hashes. A directly writable common root was mutated by native uninstall. |
| Cross-home conversation prototype | Linux fixture passed | Official native backend resumed the same thread ID from a full rollout in B's private home, retained original turns, listed the thread natively after resume, and sent the next fixture request using B's identity. |
| Implemented broker integration | Pinned Linux native fixture passes | Actual production bridge and Rust helper list/read foreign history before resume, retain every native turn item, reject a competing writer, stop the old backend before release, and continue with B's fixture identity. Empty-chat release also passes. The test-only credential adapter avoids host-keychain changes; this is not real sign-in evidence. |
| Native/format coverage gaps | Still open | Full attachment/compaction/subagent/crash matrix, complete migration/restore launch acceptance, Mac/Windows sharing and real-account inference remain unverified. |
| Unsubscribe finding | Verified locally | `thread/unsubscribe` returned `unsubscribed`, but `thread/loaded/list` still contained the thread. Unsubscribe cannot be treated as releasing writable backend state. |
| Three-platform sharing workflow | First run completed; platform corrections prepared | `.github/workflows/shared-storage.yml` and `extension/scripts/shared-storage-probe.mjs` cover production broker/migration and selected feasibility cases, rather than the full acceptance matrix below. |
| Real-account workflow | Account A verified on both platforms; continuation pending | `.github/workflows/authenticated-accounts.yml` and `extension/scripts/authenticated-runner.mjs` prepare encrypted device instructions and production helper/bridge A/B/A checks. Fresh Mac/Windows device-A logins succeeded; account B and switching remain unverified. |
| Premature runner attempts | Cancelled | Runs `38218708218` and `38219650817` were cancelled following the sequencing correction. An overly narrow Mac loopback-host assertion was corrected; these runs are not passing acceptance evidence. |
| Windows download link | Source is correct | `next/lib/site.ts` points to `Multi.Codex_<version>_x64-setup.exe`. Public rendered links, redirects, and installer payload still require final verification. |

The Linux prototype's GUI environment was tightened after its recorded pass to use a fresh private home and remove inherited account/routing variables. Repeat the relevant probe against the revised script before using it as final acceptance evidence.

### 2.2 Implemented source and remaining gaps

Local source implements shared extension generations/private catalogs with Copilot dependencies, desktop-packaged bridge/headless helper, migrated launch selection, verified migration snapshots and layout/settings rollback, isolated native history import/read, full-rollout catalog and asset checkpoints, sidebar routes, kernel writer leases, final publication after confirmed backend shutdown, and shared-storage migration/recovery/accounting UI. Explicit extension commands release chats to another window and recover a stopped writer, including unpublished retained roots. Busy chats open as full read-only snapshots. Native paged history reads and bounded catalog change events keep peer sidebars current without transferring writer ownership. Required package links and registry selections are checked before launch. Originals are retained.

The continuation implements application-owned dependency freezing/rebasing, repeat migration and catalog repair, app-version adoption, durable restore activation with retained displaced data/current credentials, automatic update-time account migration/defer/retry queue with a persistent pause, native launch confirmation, original-extension cleanup with backup-based rollback, library-wide previewed cleanup and tombstones, reachability collection, bounded successful reader/checkpoint/backend staging, loaded-child adoption and stopped-new-chat recovery. Portable installations that override private editor paths are refused; unsupported external targets and unknown rollout records keep the source intact and the account in legacy mode.

Local verification now includes 152 Rust passes (4 native-gated ignores), 106 frontend passes, 51 extension passes (3 environment-gated skips), production frontend/resource builds, 8 packaging/manifest gates, the pinned production broker test, the real-package migration/rollback test, and native private-catalog force-reinstall/uninstall isolation while a peer editor stays open. Restore service regressions additionally verify a missing current auth file preserves the newest keychain credential, deliberate sign-out remains signed out, and missing keychain access blocks restore without displacing newer work. A fresh restore no longer mistakes its own held session guard for an active client. Owned dependency resolution handles Windows verbatim prefixes and macOS aliases while ordinary storage checks continue to reject links and external targets. The complete 93-case native/GUI/failure matrix is not yet established by these results.

The manual three-platform sharing workflow now runs the production bridge/helper and cross-platform Rust migration test as well as the native editor feasibility probe. The authenticated Mac/Windows workflow additionally exercises A/B/A switching, private credential fingerprints, two minimal actual model turns, complete A-to-B shared continuation and confirmed writer-stop handoff. Device login requires the user's browser participation. Real OAuth callback completion and interactive Windows app/browser/terminal observation remain separate pending cases; browser initiation/cancellation is not completion.

Residual engineering/acceptance limits: extra extensions are not globally imported; every retained catalog pins its generation, so package cleanup conservatively keeps those consumers plus current/previous versions; failed unpublished generation staging and verified/displaced backups remain retained for deliberate recovery/cleanup. Unmapped assets, colliding native identities loaded in one backend, unknown descendant relationships and unsupported native methods fail closed. Large-scale latency/memory/CPU and the remaining attachment/compaction/subagent/crash cases require the planned native matrix. No broad native Mac/Windows success is claimed before those jobs finish.

Native run [38278589661](https://github.com/wrestle-R/multi-codex/actions/runs/38278589661), source `14075e4`, passed the Ubuntu production broker/migration suites. macOS native package replacement/uninstall isolation passed, then its temporary `/var` versus `/private/var` fixture-path assertion failed. Windows failed its first editor CLI fixture call. The probe now canonicalizes temporary roots, uses proper file-URI registry paths, and retains bounded inert command diagnostics. Auth run [38278593946](https://github.com/wrestle-R/multi-codex/actions/runs/38278593946) failed before sign-in because its native helper build needed the new managed resource directory. Native workflows now stage complete desktop resources before Rust checks; standalone helper builds bootstrap real bridge resources when absent. No real authentication passed in these runs. Fresh native runs are required after these corrections.

Follow-up run [38280311075](https://github.com/wrestle-R/multi-codex/actions/runs/38280311075), source `4c62173`, again passed Ubuntu. Windows diagnostics identified VS Code1.140.0's versioned CLI resource layout; the production installer and probe now resolve the active CLI from the editor's own `bin/code.cmd`, with an owned-path check and flat-layout fallback. macOS's fresh editor process exited before maintenance in this attempt; its bounded inert stderr is now retained for diagnosis. Auth run [38280314443](https://github.com/wrestle-R/multi-codex/actions/runs/38280314443) built both platforms and reached device-A instructions. The user completed both browser prompts, but each test stopped within device-A before account B. The existing public report cannot distinguish authentication from account saving. The script now reports narrow stages, safe credential-format booleans, verified authentication separately from launcher saving, and accepts account identity from the same signed claims used by the production bridge. Real sign-in and switching success remain unproven until the corrected runs complete.

Routine ready-account queue polling now checks catalog/link structure without repeatedly hashing gigabytes of package payload. Launch still performs full integrity verification and refuses changed bytes; the dedicated local regression test verifies that boundary.

Further native checks passed the Windows editor maintenance, peer preservation and full cross-home feasibility probe in run [38282124211](https://github.com/wrestle-R/multi-codex/actions/runs/38282124211). The job then stopped at Unix installer fixtures invoked on Windows. macOS run [38283235045](https://github.com/wrestle-R/multi-codex/actions/runs/38283235045) passed the same editor/cross-home probe after using a short physical temporary root for the editor's IPC socket; five Rust fixtures then failed on path aliases or shell executable identity. Source `9a13ea7` canonicalizes those fixtures, gives the lease fixture its own executable, and pairs Windows-specific PowerShell installer checks with Unix-specific Bash checks. These partial passes do not establish the complete production acceptance gate.

Auth run [38282128133](https://github.com/wrestle-R/multi-codex/actions/runs/38282128133) timed out awaiting device-A completion; no verified sign-in or switching success is recorded. The user corrected their earlier response to say only Mac had been attempted and requested both fresh codes together. Fresh auth run [38285283637](https://github.com/wrestle-R/multi-codex/actions/runs/38285283637) and targeted native runs [38285327993](https://github.com/wrestle-R/multi-codex/actions/runs/38285327993) (Mac), [38285331294](https://github.com/wrestle-R/multi-codex/actions/runs/38285331294) (Windows) are pending against `9a13ea7`.

Those follow-ups now provide narrower results: auth `38285283637` verified both real account-A logins, then the test accessed `added.metadata.id` even though the production helper returns flattened `added.id`. Source `251fd69` corrects the response handling and runs a disposable native add/list/credential/delete preflight before asking for another sign-in. Both runners passed that preflight in auth [38286924994](https://github.com/wrestle-R/multi-codex/actions/runs/38286924994); fresh account-A/B participation and the switching results are pending. Mac native `38285327993` passed 148 Rust tests with 7 environment ignores, then an extension asset assertion failed on an alias versus canonical target spelling; that assertion is corrected in `2bf4291`. Windows `38285331294` passed 68 Rust tests with 4 ignores and found two failures: an emptied junction stub in the repair fixture and a JSON substring assertion that overlooked escaped backslashes. The fixtures are corrected; catalog repair now explicitly validates and skips shared-generation links already supplied by the new verified generation on both platforms, while refusing unknown/external linked packages.

No new app release has been published for this work. Source and installed application versions remain 1.5.1. The next app version is expected to be 1.5.2, subject to checking the release sequence when preparing publication. Never overwrite the existing 1.5.1 release with changed binaries.

## 3. Data-preservation rules

These are implementation invariants, not optional cleanup suggestions:

1. Preserve the retained current backup. Store backups outside profile deletion trees and shared generation garbage collection.
2. Before modifying an existing profile, prove its relevant editor, CLI, Desktop, bridge, and helper sessions are closed. Acquire the migration/launch guard and recheck. Defer active profiles; do not terminate their sessions automatically.
3. Take a private, verified snapshot of the affected profile and required application metadata. Record source and backup manifests, link targets, byte counts, permissions policy, schema, and completion state. Publish the verified marker only after verification succeeds.
4. A link-preserving backup does not freeze data behind an external link. Inventory external targets; identify required application-owned assets, copy them safely where necessary, and disclose excluded external project content. Reject unresolved dependencies before claiming recoverability.
5. Keep source directories unchanged while preparing destinations. Stage data and configuration, verify them, then commit the profile layout switch as the final step.
6. Journal every transition durably. A crash or restart must distinguish an incomplete snapshot, staged generation, partial import, committed profile switch, and successfully validated launch.
7. Account IDs, credentials, chat IDs/context, archives, attachments, workspaces, unrelated settings/comments, Copilot state, and ordinary VS Code data are preservation targets. Extension optimization does not authorize resetting them.
8. Failure must leave the original profile usable or provide an exact, tested restore route. Report an incomplete import or missing asset as incomplete; do not silently replace a conversation with a summary.
9. Rollback must preserve work created after migration. Restore layout/settings deliberately and retain newly created shared histories; do not overwrite an entire profile with an old backup as a routine rollback operation.
10. Removing legacy copies is a separate, previewed action after successful validation. A path switch alone reclaims no disk space.
11. Profile deletion must not remove shared histories, shared packages, another profile's state, or retained migration backups.
12. Keep credentials, private auth artifacts, backup contents, raw private logs, and sign-in secrets out of Git, website content, runner artifacts, and published evidence. Record safe fingerprints/counts and outcome fields where needed.

Backup lifecycle: `planned → copying → verifying → verified → eligible for migration`. Failed or interrupted copies remain marked incomplete and cannot authorize a switch. Migration lifecycle: `legacy → snapshot verified → staged → validated → switched → launch confirmed → cleanup eligible`. Each profile advances independently; mixed legacy/shared operation must remain supported during migration.

## 4. Production layout and boundaries

Resolve the application root through the existing platform-aware settings code. Linux uses its user data location, macOS Application Support, and Windows the existing application data location. Do not hard-code a developer's home path.

Proposed layout:

```text
multi-codex/
  shared-vscode/
    generations/
      <platform-architecture-version-digest>/
        packages/
          openai.chatgpt-<version>/
          openai.codex-audio-<version>/
          multi-codex-bridge-<version>/
        manifest.json
    staging/
    maintenance-user-data/
    active-generation.json
    operation.lock
  shared-chats/
    catalog.sqlite
    checkpoints/<content-digest>/
    assets/<content-digest>/
    leases/<library-thread-id>.lock
    owners/<library-thread-id>.json
    imports/
    operation.lock
  migrations/
    profiles/<account-id>.json
  profiles/<account-id>/
    codex-home/
    vscode-user-data/
    desktop-data/
    launch-workspaces/
    vscode-extension-catalog/
    vscode-extensions/
    runtime/
    layout.json

multi-codex-backups/
  <retained-current-snapshot>/
  <verified-closed-profile-snapshot>/
```

`catalog.sqlite` is owned by MultiCodex; it is not Codex's native runtime database. Schema names and exact generation naming can change during implementation, but the isolation and ownership boundaries cannot.

The new private extension catalog contains VS Code's per-account extension registry/removal metadata and links to selected shared packages. Use supported Unix directory links and native Windows junctions where verified. Existing Copilot packages/state are preserved separately as needed. Legacy `vscode-extensions` stays available for fallback until explicit cleanup.

Chat checkpoints contain complete native conversation history and supported assets. Each backend works in its own private runtime/home/database. Live backends never append directly to canonical checkpoint files. The bridge publishes committed revisions through the broker.

New accounts reuse a validated shared generation, create their own private catalog and user data, and import no ordinary global editor profile. The bridge needed for managed sharing is an application integration dependency, not a bulk extension import.

## 5. Shared extension service

### 5.1 Storage mechanism

Use **private extension catalogs with versioned shared package payloads**. The directly writable shared `--extensions-dir` approach is rejected because the local native prototype demonstrated cross-instance mutation.

Each launch uses:

```text
<existing VS Code executable>
  --new-window
  --user-data-dir <account>/vscode-user-data
  --extensions-dir <account>/vscode-extension-catalog
  <account's generated workspace descriptor>

CODEX_HOME=<account's private Codex home>
CODEX_SQLITE_HOME=<account's private SQLite location>
```

Preserve inherited-auth sanitization, editor-routing sanitization, process monitoring by private data paths, macOS runtime aliases, per-launch workspaces, and configured editor support. Detect Portable Mode or another configuration that overrides isolation flags; require a compatible editor or keep legacy mode.

A symlink/junction is not write protection. The service must prove that supported native install/update/uninstall/cleanup actions cannot corrupt published shared bytes. Versioning, permissions where practical, integrity checks, private metadata, and replacement behavior all need native tests. If any supported writer follows a link and modifies the payload, revise the mechanism before enabling it.

### 5.2 Initialization and publication

- Use an interprocess operation lock; concurrent first launches initialize one generation.
- Install or stage a pinned compatible Codex package using isolated maintenance user data and a scrubbed environment. Account credentials must never enter the installer environment.
- Validate extension IDs, manifests, platform/architecture, required companions, bundled engine, bridge package, hashes, and activation before publication.
- Determine the supported treatment of Codex's optional audio extension pack companion. Do not treat it as an unrelated extension or silently break audio features.
- Seed from a clean supported installation only when package completeness can be verified. Do not import `.obsolete`, stale versions, arbitrary files, or another account's registry.
- Publish a complete generation atomically. Interrupted staging must not become the active generation.
- Build private catalogs against that generation. Verify registry paths and native discovery before committing an account switch.
- Avoid holding the existing account metadata lock during downloads or large copies.
- Missing editor/network, full disk, invalid package, permission errors, cancellation, and timeouts must leave legacy accounts usable and staging recoverable.

### 5.3 Maintenance policy

The initial implementation uses conservative queued managed maintenance: prepare a replacement in staging, but wait for affected consumers to close before changing their catalog references or collecting generations. Never replace code under a live window or force all windows to reload.

Native extension actions have their actual tested scope. A native install in one private catalog must not be advertised as a globally queued install unless the application truly intercepts it. Provide an explicit managed action for additions intended to become globally available; validate package discovery and preserve private enablement/settings.

Track consumers of each generation. Retain the active generation, referenced legacy generations, and one known-good rollback generation until cleanup conditions are met. Garbage collection requires the lock, consumer recheck, managed-path validation, and a concrete preview. Broken catalog links must produce repair/fallback rather than shared-data deletion.

Sharing package files does not share extension-host processes. Test a failed host separately from a defective package/version affecting several hosts.

## 6. Private settings, Codex startup, and Copilot

For fresh managed profiles, set these minimal defaults where supported by the selected extension/editor:

```json
{
  "chatgpt.openOnStartup": true,
  "extensions.autoUpdate": false,
  "extensions.autoCheckUpdates": false
}
```

For existing profiles, use a JSON-with-comments-aware edit that changes only required managed settings and preserves comments, unrelated keys, formatting where practical, and account-specific configuration. Record the prior values for rollback. Malformed or unexpected settings must block the affected edit and retain legacy behavior; do not replace the entire file.

Disable automatic native extension maintenance only within the managed account environment. Preserve ordinary VS Code settings and sync sessions. Test whether Settings Sync reintroduces packages or changes managed settings; provide a tested policy rather than assuming sync honors app locks.

Preserve Copilot's installed/bundled package discovery, per-account enabled/disabled state, settings, UI, and authentication behavior. Do not add Copilot removal rules, disabling flags, AI-hiding settings, or global user-data imports. If Copilot preservation cannot be verified for an account, leave that account in legacy mode.

Verify `chatgpt.openOnStartup` against the pinned manifest and native UI. Auto-opening must work on cold launch and reload without stealing unrelated global editor configuration.

## 7. Shared chat library and bridge

### 7.1 Import, catalog, and assets

Inventory active and archived chats through a version-supported native reader. Reading a source must not trigger unprotected migrations of its live database: use a verified closed-profile snapshot or an isolated reader copy where initialization can mutate state.

For each source thread, retain source attribution, native ID, logical library ID, archive state, timestamps, project context, complete rollout/checkpoint version, and supported asset references. Import messages, tool calls/results, approvals where represented, compaction history/state, images, and referenced application-owned attachments. Do not copy credentials, source-account config, plugins with secrets, or a raw live runtime database into shared storage.

Imports are resumable and idempotent. Publish catalog references only after checkpoint and asset content is durable and verified. Duplicate scans must not duplicate chats or turns. Conflicting source IDs must preserve both full histories and receive explicit stable mappings; never silently merge unrelated conversations.

Resolve asset references within verified boundaries. Copy supported files by content digest; rewrite supported path fields to valid hydrated locations without altering message content. Preserve project paths when appropriate. Missing assets, unsupported formats, inaccessible external files, or unexpected schemas must be reported and keep the source intact. Do not claim complete continuation when restoration is partial.

Retention distinguishes canonical checkpoints/assets, retained source histories, per-backend native working copies, and backups. Bound temporary hydration/checkpoint growth through tested reachability rules, not arbitrary deletion of source sessions.

### 7.2 Native sidebar integration

Extend the production bridge so the official Codex sidebar can list, open, read, archive, and continue shared chats under the selected account. Listing must work before manually resuming a foreign thread. Merge active/archived pagination and thread metadata consistently; do not offer only a separate read-only library UI.

Map logical and native IDs deterministically. Full continuation remains the same logical conversation, even if an engine version requires a native mapping. A summarized new conversation or silent fork does not satisfy the requirement.

Handle frontend requests, notifications, reconnects, errors, and archive filters using the validated protocol. Unknown mutation methods must not bypass ownership checks. Preserve existing direct/private functionality until the shared route is validated. Publish checkpoint/catalog events so other windows discover completed new chats and revisions without restart.

The current startup code links session directories into private backend homes. Managed shared mode must replace writable session sharing with private hydration from committed broker checkpoints. Review the remaining skills/rules/plugins/memories/prompts links separately; do not broaden their sharing scope as part of chat import.

### 7.3 Identity and account switching

Verify the target account before any continuation or model request. Load its current private credential through the existing account helper and refresh protections. Do not carry the source account's auth, permissions, or secret-bearing tools into the new backend.

Keep source credentials unchanged during A/B/A switching and handle refresh/expiry without crossing profile boundaries. Selecting a different account must affect the next authenticated request, not just the displayed account label.

Use deterministic fixture inference for extensive automation. For real native acceptance, verify authenticated account identity and a real usage/read operation. If a minimal actual model turn is needed to establish identity behavior, keep it explicit in the test procedure and record the limited real-service check separately from fixture coverage.

### 7.4 Ownership and safe handoff

One writer per shared chat is the proposed default. Different chats have independent locks. An account-wide lock must not serialize unrelated work.

Acquire a cross-process per-thread ownership lock before hydration/resume or any mutation. Hold ownership for the full lifetime of loaded writable state, including idle periods, queued turns, approvals, tools, background work, and compaction. Observers read committed checkpoints and cannot append, rename, archive, delete, or otherwise mutate through an unguarded route.

Use kernel-backed locking plus durable owner information sufficient to distinguish process lifetime, including backend PID/start identity and private runtime location. A timer expiring or the helper disconnecting cannot alone authorize a new writer if the old backend remains alive. Account leases and thread ownership serve different purposes.

The safe initial handoff procedure is:

1. Stop accepting new frontend mutations for the old backend and check every loaded thread it owns.
2. Wait for active turns, pending tools/approvals, and queued work to settle; otherwise keep handoff blocked with a clear reason.
3. Publish verified complete checkpoints for the relevant loaded threads.
4. Stop the old native backend and confirm its exit. `thread/unsubscribe` and an idle turn are insufficient.
5. Release ownership only after the old writer is stopped or a separately proven write fence exists.
6. Restart a private backend behind the persistent frontend bridge, initialize it, restore the selected account identity, and hydrate the required latest checkpoint.
7. Let the target backend acquire ownership and continue the same logical chat. Update other loaded-thread state consistently; stopping one backend may affect more than the selected chat.

A bridge refactor must guard against late events from an old process generation and prevent UI requests racing with backend replacement. Preserve the editor window and provide observable progress/failure recovery.

On crash, helper death, orphaned backend, or disconnect, recover from the last durable checkpoint and verify old writer death before releasing ownership. Do not create duplicate turns or accept writes from a stale capability. Decide and implement how unfinished streamed work is surfaced without presenting it as committed history.

### 7.5 Scope of clients

The first shared UI target is desktop-launched VS Code with the managed bridge. Ordinary VS Code windows, CLI, Codex Desktop, remote WSL/SSH/container stores, and externally launched clients must not be silently redirected into broker-owned mutable storage.

Preserve current CLI/Desktop private launch behavior and supported-version restrictions. Any client allowed to mutate shared chats must use the same ownership/checkpoint contract and receive its own acceptance coverage. Account deletion must preserve shared history even when a source client remains outside the broker.

## 8. Migration, launch integration, and recovery

### 8.1 Preview and readiness

Show a per-profile preview with layout state, running consumers, original extension bytes, shared generation bytes, snapshot requirement, retained copy bytes, peak free-space requirement, estimated reclaimable bytes, extra extensions excluded from the minimal managed setup, Copilot preservation, and incomplete chat/asset imports.

Insufficient disk space or a failed backup blocks switching. Do not remove the source to make enough space for its replacement. Test multiple concurrent migrations and account creation against the same locks.

### 8.2 Existing profile migration

1. Inventory profile metadata, credential references, all relevant consumers, extension manifests, settings, workspaces, native histories, and assets.
2. Acquire the operation guard and recheck inactivity and source boundaries.
3. Produce and verify the closed-profile snapshot; preserve the retained current safety backup.
4. Initialize/validate the shared generation once, then stage the profile's private catalog.
5. Import complete histories/assets from safe source copies, with a resumable journal. Preserve original histories.
6. Merge managed settings and bridge configuration in staging. Record changed keys and rollback values without recording secrets.
7. Validate native package discovery, bridge availability, account identity routing, and import integrity.
8. Atomically commit the layout marker and launch path only after required validation. Keep the legacy layout available.
9. Launch with private account paths and confirm successful Codex activation, sidebar/library behavior, Copilot preservation, and expected account identity.
10. Mark cleanup eligibility only after successful launch and preservation checks. Failed launch/activation offers a tested legacy fallback.

Fresh profiles skip legacy-copy migration but still require validated generation/catalog setup and private identity/state initialization. A second or tenth fresh account must not repeat a large package installation.

### 8.3 Recovery and rollback

Every snapshot, install, import, settings edit, layout switch, and launch-confirmation step has an injectable interruption point. Recovery either resumes the staged operation or discards only incomplete owned staging, leaving originals usable.

Before legacy removal, rollback selects legacy extension/catalog paths and restores only migration-owned settings that have not since been deliberately edited. Retain new shared conversations and show their recovery/access route. After legacy removal, exact restoration requires the retained verified snapshot or a complete reinstall route; missing backups must be reported rather than hidden behind a success label.

Provide an explicit restore preview and test recovery of credentials via supported secret-store references, editor state, workspaces, histories, and assets. Do not assume file-copying alone restores platform keychain entries.

### 8.4 Cleanup

“Remove old extension copies” is distinct from migration, rollback, account deletion, and chat cleanup. Preview exact directories and bytes, recheck closed state and generation references under the lock, reject links/escapes, and delete only eligible approved copies. Never delete backups or Codex homes through this action.

Shared chat rename/archive/delete has a library-wide scope. Shared deletion requires a preview, ownership/activity checks, durable catalog changes/tombstones, and asset reachability checks. Imported source scans must not resurrect intentionally deleted shared chats. Preserve the existing private cleanup route for legacy/private clients and label its scope clearly.

Current safe cache cleanup remains restricted to its verified allowlist. `User`, editor backups, databases, `WebStorage`, session history, installed extensions, and rollback copies are not automatically disposable caches.

## 9. Storage and maintenance UI

Expose these categories separately:

| Category | Counting and presentation |
| --- | --- |
| Shared editor packages | Count payload allocation once; show generation/version, consumers, integrity, and queued maintenance. |
| Shared chat library | Count catalog, checkpoints, and assets once; distinguish committed content from staging. |
| Private account state | Show Codex runtime, editor data, Desktop data, workspaces, private catalogs, and supported caches. |
| Retained source/legacy copies | Show original extensions and histories still held for migration/rollback. |
| Retained backups | Show snapshot status, verification, location category, size, and restore availability. |
| Temporary/runtime copies | Show hydration and staging costs separately so deduplication does not hide growth. |
| App metadata | Reserve for actual small metadata files; do not hide shared packages here. |

The current directory-size calculation sums regular file lengths and skips symlinks. Extend it for shared ownership, hard-link identity if used, and Windows reparse points. Distinguish logical bytes from allocated bytes where supported; keep totals internally consistent. Do not follow links into unrelated directories or count a shared generation once per profile.

The earlier six-profile measurement was approximately 6.48 GiB extensions, 3.86 GiB VS Code state, 2.37 GiB Codex homes, and 0.44 GiB Desktop data. A roughly 0.96 GiB shared Codex set suggests about 5.5 GiB of extension savings only after verified legacy cleanup. Copilot preservation, bridge packaging, versions, snapshots, checkpoints, assets, and runtime copies change the measured outcome. This is not a hard cap or a promised immediate reduction.

## 10. Application update, shortcuts, downloads, and auth debugging

### 10.1 Update behavior

Keep quiet background checking and the newer-version-only Update button from 1.5.1. Check startup, every user refresh entry point, visibility/reconnect, and the existing bounded interval. Network/feed failures must not show the rejected “Couldn't update” background popup or erase an already known newer release. Explicit download/install failures need actionable status and retry.

Signed update acceptance covers the actual Tauri SDK download, verification, installation, process exit/restart, version change, profile preservation, and post-update migration state on each target. Validating a manifest or a detached signature alone does not establish successful installation.

Migration must be resumable after restart. Active profiles stay legacy/deferred. Do not force-close their editor sessions during app update. Preserve existing shortcuts and user customizations; avoid duplicate entries or accidentally claiming a different portable installation.

### 10.2 Windows browser sign-in and terminal issue

Audit the complete GUI browser and device-login paths, including app-server startup, CLI login, executable resolution, `.cmd`/shell wrappers, Node helpers, browser opening, cancellation, and error retry. Ensure native console children use the existing background flags. Keep credential/SQLite files in the correct private home.

The committed source fix addresses known direct spawning, but it is not proof that the entire browser sign-in route is fixed. Verify both `GetConsoleWindow()` and actual interactive Windows process/window behavior during browser login. A hosted runner without an observable interactive desktop cannot substitute for a visible local/interactive GUI test.

### 10.3 Browser OAuth versus device code

Browser OAuth initiation/cancellation and completed device-code authentication are different tests. The prepared remote workflow currently performs browser initiation/cancellation and then fresh device-code A/B logins. Do not describe that as completed browser OAuth.

For full remote browser OAuth, provide a working callback route from the user's browser to the temporary runner's localhost listener, using a suitable authenticated tunnel/forwarding arrangement. Protect callback state and private logs. If that cannot be established, record the browser-completion test as blocked and perform it on an interactive native host; do not count device login as its replacement.

When tests are ready, present each platform's account-A URL/code, wait for the user to complete it, then present account B's URL/code. Verify two distinct identities and prompt clearly on expiry/failure. Use the existing encrypted instruction artifacts; never ask for passwords, raw token files, or credentials in chat. Clean up temporary auth, keychain entries, bridge processes, tunnels, and artifact instructions after testing.

### 10.4 Windows public downloads

Every Windows download CTA must resolve to the intended signed NSIS `.exe` installer for the displayed release, with a successful response and correct payload/version. Check home/install/release pages and any README/download navigation that offers a Windows installer. Updater archives may still exist as internal release assets; they must not replace the public installer link.

## 11. Implementation checklist and dependencies

### Completed

- [x] Review the original plan, latest user corrections, current architecture, and existing release behavior.
- [x] Retain the current six-profile live safety backup plus the installed app and desktop entry; preserve originals.
- [x] Hash retained regular file content and verify profile IDs/metadata; document the live-copy limitation.
- [x] Remove the specifically authorized older snapshot while retaining the current backup.
- [x] Commit Windows background-spawn fixes and add a native no-console regression test.
- [x] Commit Linux shortcut metadata refresh and pass seven disposable integration tests.
- [x] Demonstrate the directly writable shared extension-root failure on a disposable Linux setup.
- [x] Demonstrate selected private-catalog uninstall isolation on Linux.
- [x] Demonstrate same-ID/full-turn cross-home resume, native listing after resume, and next-request fixture identity B on Linux.
- [x] Verify unsubscribe does not unload and therefore cannot release writer ownership.
- [x] Add initial managed file/snapshot utilities and pass their targeted local checks.
- [x] Prepare sharing/authentication runner scaffolding; cancel premature runs as requested.
- [x] Rebuild this complete plan with explicit remaining work and test gates.

Implementation checkmarks below record completed source and the specifically stated local evidence. Native/GUI acceptance remains governed by sections 12 and P4; the prepared workflow covers selected cases, not every matrix entry.

### P0 — Preservation and recoverability

- [x] Integrate managed-path/snapshot utilities into a real migration service.
- [x] Harden root/link/reparse checks, source/destination validation, restore boundaries, and Windows junction handling.
- [x] Inventory external links/assets and record the exact backup coverage.
- [x] Add all-client inactivity checks, migration/launch guards, and locked rechecks.
- [x] Implement verified closed-profile snapshots, peak-space checks, completion markers, and private manifests.
- [x] Implement durable per-profile journals, restart recovery, restore preview, and tested rollback.
- [x] Preserve post-migration work and secret-store identity during rollback.

### P1 — Shared packages and launch integration

- [x] Implement generation install/stage/validate/publish, pinned-version policy, integrity, and single initialization.
- [x] Implement private extension catalogs/links, including preserved Copilot dependencies and state.
- [x] Package the managed bridge and native helper for each release target without relying on developer files.
- [x] Wire fresh and migrated launches in `profiles.rs`; preserve private paths, aliases, workspaces, and process detection.
- [x] Merge managed JSONC settings with rollback-aware prior values; verify startup/sidebar support.
- [x] Implement queued maintenance, consumer tracking, repair/fallback, and bounded generation retention.
- [ ] Prove supported native editor writers cannot corrupt shared payloads.

### P2 — Full shared chats and ownership

- [x] Implement catalog/schema, content-addressed full checkpoints/assets, import mappings, and durable publication.
- [x] Implement safe active/archived inventory, idempotent import, conflict handling, and missing-format/asset reporting.
- [x] Replace writable session links with private hydration for managed shared mode.
- [x] Route native sidebar listing/read/resume and metadata events through the bridge before manual resume.
- [ ] Verify identity refresh and next authenticated request use the selected account.
- [x] Implement cross-process per-thread locks, durable owner identity, capability checks, and orphan detection.
- [x] Refactor backend lifecycle for checkpoint/confirmed stop/restart handoff, including all loaded threads and late-event fencing.
- [x] Implement observer behavior, independent-chat concurrency, crash recovery, and no duplicate turns.
- [x] Implement library-wide rename/archive/delete with previews, tombstones, reachability, and account-deletion preservation.

### P3 — Product flows and local implementation tests

- [x] Add native commands, compatible settings schemas, TypeScript API/types, and migration/storage/maintenance dialogs.
- [x] Show measured shared/private/legacy/backup/runtime totals and actual reclaimed bytes.
- [x] Add explicit legacy-copy cleanup and clear private-versus-shared chat cleanup scopes.
- [ ] Complete Windows browser-spawn audit and test the app entry points, not only helper functions.
- [ ] Complete local fixture, failure-injection, regression, and UI checks described below.
- [x] Repeat the hardened Linux prototype and record evidence against the exact implemented source.
- [x] Finish implementation review; only then dispatch GitHub native checks.

### P4 — Native acceptance, after P0–P3

- [x] Extend prepared workflows to exercise the production sharing/migration paths and full test matrix.
- [ ] Run native Linux x64, macOS Apple Silicon, and Windows x64 sharing/migration checks.
- [ ] Complete real Mac/Windows account-A/account-B device logins with user-provided browser completion.
- [ ] Complete A/B/A identity, usage, source-credential, and shared continuation checks.
- [ ] Complete browser OAuth callback sign-in separately on observable native hosts.
- [ ] Observe Windows browser sign-in with no unexpected terminal and verify cancel/retry/relaunch behavior.
- [ ] Run real SDK updater/install/restart and shortcut/data preservation tests on Mac and Windows.
- [ ] Fix failures and rerun affected checks against the revised source; keep unsupported cases explicit.

### P5 — Release and user update-button acceptance

- [ ] Choose the next unused app version and update all applicable version sources together.
- [ ] Build/sign the final native packages and updater assets through the established tag-triggered release process.
- [ ] Verify immutable asset names, signatures, manifest targets, downloads, version consistency, and source fingerprints.
- [ ] Publish complete website release notes and short GitHub links; verify Windows `.exe` CTAs live.
- [ ] Publish only after required acceptance evidence is complete and limitations are accurately documented.
- [ ] Ask the user to launch/refresh the installed 1.5.1 app and observe the new Update button.
- [ ] Let the user perform the update; verify preservation, migration/defer behavior, shortcuts, and cold start afterward.
- [ ] Record measured storage savings only after the user deliberately removes eligible legacy copies.

## 12. Thorough testing plan

All cases below are required planned checks unless marked completed in section 2. Existing prototypes cover selected subcases only. Use temporary application roots and inert credentials for destructive, concurrency, crash, and failure-injection testing. Never run those scenarios on the six real current profiles.

### 12.1 Test environments and evidence

| Environment | Purpose | Required distinction |
| --- | --- | --- |
| Local Linux disposable fixtures | Fast service/protocol/UI/failure checks during implementation | No local app upgrade or real-profile migration. |
| Native GitHub Linux x64 | Packaging, filesystem, sharing, migration, multi-window, and updater regression | Dispatch after implementation; native execution required. |
| Native GitHub macOS Apple Silicon | Package activation, aliases, filesystem, two-account authentication, switching, migration, updater | Record actual OS/editor/extension/engine versions and architecture. |
| Native GitHub Windows x64 | Junctions, paths, process spawning, two-account authentication, switching, NSIS and updater | Cross-compilation and hidden-helper checks alone are insufficient GUI proof. |
| Interactive Mac/Windows hosts | Browser OAuth callback completion, visible sign-in/terminal behavior, sidebar and updater/restart UI | Use a hosted runner only if its interactive desktop and callback route are genuinely observable. |
| User's existing Linux install | Final 1.5.1 → new-release Update-button acceptance | User performs the update; current backup remains retained. |

Prepared sharing probes pin VS Code 1.140.0 and Codex extension 26.1002.51308. The authentication scaffold uses official CLI 0.160.0. Validate availability and compatibility before running, record exact versions, and do not silently substitute a moving latest build. Test the production-bundled bridge/backend combination, plus a controlled editor-upgrade compatibility case. An unsupported upstream Desktop build must remain safely refused unless separately validated; preserve the existing allowlist.

Each case produces: case ID, exact source commit, platform/architecture, app/editor/extension/engine versions, fixture version, outcome, assertions, and sanitized failure stage. Keep screenshots/videos where UI behavior is the assertion. Keep sensitive homes, credentials, raw private logs, and conversation contents out of uploaded evidence. A skipped, blocked, or cancelled case remains visible and is never counted as passed.

### 12.2 Backup, migration, and restore cases

| ID | Procedure | Pass condition |
| --- | --- | --- |
| M01 | Snapshot a stopped fixture with chats, auth references, settings/comments, workspaces, empty directories, and supported links. | Source/copy manifests agree, private permissions hold, completion marker is written last, original remains intact. |
| M02 | Mutate a source during backup. | Verification fails, no layout switch occurs, original remains usable, copy stays incomplete. |
| M03 | Keep VS Code, CLI, Desktop, bridge, or helper active in separate trials. | Migration defers each affected profile and explains the consumer; no session is stopped. |
| M04 | Launch a consumer between preview and locked migration. | Under-lock recheck blocks switching. |
| M05 | Inject disk exhaustion during backup, package staging, asset import, settings write, and journal commit. | Original/committed data remains usable; incomplete staging is not published; retry is recoverable. |
| M06 | Interrupt after each migration lifecycle transition. | Restart resumes or safely rolls back the exact step without duplicate imports or lost state. |
| M07 | Supply linked roots/ancestors, path traversal, relative roots, broken links, external targets, and Windows reparse points. | Invalid boundaries are rejected; approved links are handled explicitly; no unrelated destination is modified. |
| M08 | Migrate valid JSONC settings with comments, nested unrelated keys, and existing managed values. | Required keys are merged; unrelated values/comments and rollback metadata survive. |
| M09 | Migrate malformed JSONC or an unexpected non-object settings root. | No whole-file reset; affected profile retains legacy launch and receives a specific error. |
| M10 | Migrate Unicode/space/long-path profile/workspace fixtures on each OS. | Discovery, copy, launch, journal, and recovery preserve exact supported paths. |
| M11 | Run two migration requests and a fresh-account launch concurrently. | Shared initialization runs once; profile operations serialize correctly without corrupting metadata. |
| M12 | Compare account IDs, credential fingerprints/references, chat/asset inventory, workspaces, editor state, and Copilot state before/after. | All required preservation targets match or have an explicit verified mapping. |
| M13 | Fail new-layout launch/activation after switching. | Tested legacy fallback restores usability without deleting the staged shared data or source. |
| M14 | Roll back after creating a new shared chat and editing unrelated settings. | New work and user edits survive; only migration-owned changes are reversed safely. |
| M15 | Restore a verified snapshot into a stopped temporary profile, including secret-store handling. | Correct account identity, complete chats/assets, and editor/workspace state work through supported APIs. |
| M16 | Remove/alter a backup manifest or corrupt one backup file before restore. | Restore detects the mismatch and does not report success or overwrite a working profile. |
| M17 | Exercise mixed legacy/shared profiles across app restarts and updates. | Each profile follows its recorded layout; active profiles remain deferred and usable. |
| M18 | Preview and remove eligible legacy extension copies, then retry and attempt rollback. | Only approved copies are removed; bytes reflect reality; retry is idempotent; remaining restore route is accurate. |
| M19 | Include an external asset link and a transient socket/lock entry in separate fixtures. | Coverage is explicit; unresolved/restoration-unsafe entries block verified migration rather than being silently skipped. |

### 12.3 Shared extension and editor cases

| ID | Procedure | Pass condition |
| --- | --- | --- |
| E01 | Launch fresh accounts A and B concurrently with no shared generation. | One validated install; both private catalogs discover/activate Codex; private data remains distinct. |
| E02 | Launch accounts 3–10 after initialization. | No repeated large package install/copy; all use the same verified generation. |
| E03 | Native CLI and Extensions UI uninstall in B while A is open. | Shared payload hashes and A's discovery/operation remain intact; B's action has its documented private scope. |
| E04 | Native install, update, VSIX replacement, removal cleanup, and Settings Sync against a linked catalog. | No supported writer corrupts shared payloads or another catalog; unsupported actions are clearly handled. |
| E05 | Queue managed update while consumers are active, then close them and launch again. | No live package replacement; publication/adoption occur under guards; new launches use the intended generation. |
| E06 | Crash/cancel initialization and publication; corrupt a staged package manifest/hash. | Partial generation is not active; legacy/known-good state remains usable; retry completes once. |
| E07 | Fail network/download, remove the editor, deny link creation, or break a catalog link. | Actionable recovery/fallback; no data loss or claim of successful activation. |
| E08 | Test Windows junction deletion/replacement and Unix link deletion. | Removing a catalog entry does not recursively remove its target; GC respects real consumers. |
| E09 | Delete the first account, then launch another. | Shared Codex and bridge still work; generation belongs to the app, not the deleted account. |
| E10 | Test bundled and separately installed Copilot with enabled and disabled states. | Availability, state, settings, UI, and supported authentication behavior are preserved. |
| E11 | Cold launch, ordinary launch, editor reload, and new workspace. | Codex opens as intended with no global editor-setting changes. |
| E12 | Start ten windows, including two accounts on one project. | Correct private homes/SQLite/user data, no route reuse, stable process tracking and workspace descriptors. |
| E13 | Crash one extension host; separately activate a deliberately faulty fixture package. | Unrelated hosts remain usable where expected; bad shared version is detectable and rollback works. |
| E14 | Test configured executable paths, macOS aliases, Windows native executable resolution, and Portable Mode. | Supported launches retain isolation; overriding/unsupported modes are refused or remain legacy. |
| E15 | Audit package writes during normal activity and account auth refresh. | Account-specific writable state stays out of shared payloads. |
| E16 | Attempt generation GC while a consumer starts/exits; include stale and referenced generations. | Locked recheck prevents deleting live/referenced bytes; only previewed eligible generations are removed. |

### 12.4 Complete chat history and sidebar cases

| ID | Procedure | Pass condition |
| --- | --- | --- |
| C01 | Import active and archived chats from multiple profiles, restart, and scan again. | Complete idempotent inventory with stable IDs/attribution and no duplicated chats. |
| C02 | Open B's native sidebar before manually resuming A's chat. | A's imported chats are visible, searchable/filterable as supported, and readable with correct metadata. |
| C03 | Continue A's chat in B and then A again. | Same logical conversation; prior full turns remain intact; each next fixture request uses the selected identity. |
| C04 | Import histories containing tool calls/results, compaction, images, attachments, and project paths. | Full native context and supported assets survive hydration/resume; no summary substitution. |
| C05 | Supply missing assets, unsupported schema/version, inaccessible external paths, and corrupt rollout content. | Specific incomplete/import errors; originals preserved; no false complete-continuation result. |
| C06 | Supply identical source IDs with divergent histories and repeated identical imports. | Divergent full histories remain distinct; identical scans are idempotent; mappings survive restart. |
| C07 | Create/complete a chat in one window while another observes its library. | Other window receives committed catalog/checkpoint updates without an application restart. |
| C08 | Page through large active/archived libraries and reopen after restart. | No missing/duplicate pages, stable ordering/filters, accurate archive metadata. |
| C09 | Change selected account with expired/refreshing credentials. | Correct private refresh; next authenticated action matches selected account; source auth is unchanged. |
| C10 | Delete the originating account after import. | Other accounts retain full shared history/assets and can continue through their own credentials. |
| C11 | Measure repeated hydration and many checkpoint revisions. | Retention policy bounds eligible runtime/staging growth without removing reachable context/assets. |
| C12 | Use private CLI/Desktop histories and ordinary VS Code windows alongside managed sharing. | No silent redirection into shared mutable state; imports remain safe and legacy clients retain private behavior. |

### 12.5 Writer ownership, handoff, and crash cases

| ID | Procedure | Pass condition |
| --- | --- | --- |
| L01 | Two processes attempt first writer acquisition for the same chat simultaneously. | Exactly one writer; the other gets observer/busy behavior with no append. |
| L02 | Two accounts write different chats simultaneously. | Independent progress; no unnecessary global or account-wide serialization. |
| L03 | Attempt handoff during streaming, approval, tool execution, queued work, and compaction in separate trials. | Handoff blocks/drains appropriately; ownership cannot change prematurely. |
| L04 | Leave a chat idle but loaded; call unsubscribe and request a new writer. | Ownership remains protected until confirmed backend stop or proven fencing. |
| L05 | Complete normal A→B handoff while the old backend has several loaded chats. | All affected checkpoints are consistent; old process exits before release; UI recovers under correct identity. |
| L06 | Kill helper/bridge but deliberately leave old backend alive. | New writer is blocked by durable owner/process identity; no timeout-only takeover. |
| L07 | Kill old backend mid-turn, restart the app, and reacquire. | Recovery uses the last durable checkpoint; unfinished work is identified; no duplicate committed turn. |
| L08 | Reuse a PID or present stale owner metadata/capability. | Start identity/runtime checks reject false ownership and stale writers. |
| L09 | Deliver delayed old-backend responses/events after bridge restart. | Process-generation checks reject them; new frontend/backend state is not corrupted. |
| L10 | Issue concurrent resume/turn/archive/delete and unknown mutation requests from observers. | Every mutation route enforces ownership; no protocol bypass. |
| L11 | Deny IPC authentication, use another socket/token, or disconnect during checkpoint publication. | No unauthorized mutation; incomplete publication is invisible; recovery retains committed history. |
| L12 | Crash at checkpoint write, fsync/rename, catalog commit, owner update, backend exit, and ownership release. | Consistent durable checkpoint/catalog pair, no split ownership, restart recovery verified at each boundary. |
| L13 | Fail backend restart/auth reapply during handoff. | Chat remains recoverable and correctly locked/released according to verified process state; failure is visible. |

### 12.6 Shared cleanup and storage cases

| ID | Procedure | Pass condition |
| --- | --- | --- |
| S01 | Measure two and ten linked profiles plus one shared generation. | Shared payload is counted once; private state/catalog bytes are counted per account. |
| S02 | Include hard links if supported, symlinks, junctions, external links, sparse files, backups, and staging. | Totals use the documented logical/allocated rules and never follow unrelated targets. |
| S03 | Compare preview, retained copies, and measured bytes after successful cleanup. | Actual reclaimed bytes reflect deletion; path switch is not reported as reclamation. |
| S04 | Attempt legacy/cache cleanup with an active consumer or a replaced linked path. | Locked recheck/boundary guard blocks unsafe removal; unrelated files remain unchanged. |
| S05 | Rename/archive/unarchive shared chats from an owner and refresh another window. | Library-wide metadata is consistent; observer mutation remains guarded. |
| S06 | Preview delete-for-all with active and idle chats, descendants, and shared assets. | Scope is clear; busy/protected histories survive; only eligible approved content is deleted. |
| S07 | Rescan sources after shared deletion and crash during tombstone/asset GC. | Deleted chats are not resurrected; reachable assets survive; operations recover idempotently. |
| S08 | Delete an account and run private legacy cleanup. | Shared history, generations, other profiles, and current/verified backups are preserved. |
| S09 | Exercise safe cache cleanup against editor history, WebStorage, User databases, and rollback fixtures. | Only the current verified allowlist is cleared; preservation targets remain untouched. |
| S10 | UI-load incomplete migration, deferred account, queued update, broken generation, and retained backups. | Status, counts, recovery actions, and scope are accurate; no misleading success or safe-cache label. |

### 12.7 Real authentication and Windows terminal cases

| ID | Procedure | Pass condition |
| --- | --- | --- |
| A01 | Native Windows background helper launches through all audited spawn paths. | `GetConsoleWindow() == 0` where required; no shell/console wrapper regression. |
| A02 | Start browser OAuth from the actual Windows app GUI and observe process/window activity through completion. | Browser opens correctly, account is verified, and no unexpected terminal appears. |
| A03 | Complete native Mac browser OAuth with a working localhost callback route. | Real completed login and correct private identity; initiation alone does not pass. |
| A04 | Cancel browser login, close the browser, reject consent, expire callback state, and retry. | Pending children/state cleaned safely; no extra terminal, stale login, or cross-account selection. |
| A05 | Complete fresh device-code A and B logins on Mac and Windows. | User receives current URL/code, two distinct identities verified, private homes/secret store isolated. |
| A06 | Production helper/bridge switches A→B→A and reads authenticated usage each time. | Selected identity changes correctly, source credential fingerprints remain unchanged, and normal switching keeps backend PID where applicable. |
| A07 | Perform shared-chat handoff with the two real accounts. | Backend restart is intentional for handoff, full context survives, selected identity is verified, and ownership stays exclusive. |
| A08 | Expire/refresh credentials, sign out B, and keep A active. | A is unaffected; B refresh/sign-out remains private; no token swap. |
| A09 | Expire device code or fail network/auth mid-login, then retry. | Clear recoverable state, fresh instructions, no duplicate profile or stuck process. |
| A10 | Finish or cancel the runner test and inspect its public artifacts. | Temporary credentials/keychain entries/processes/tunnels are cleaned; artifacts contain only approved sanitized results or encrypted instructions. |

A06's same-backend expectation applies to ordinary account switching. A07 deliberately restarts the backend to fence a shared-chat handoff; that change must not be mistaken for a switching regression.

### 12.8 Updater, packaging, shortcuts, and public links

| ID | Procedure | Pass condition |
| --- | --- | --- |
| U01 | Start old app online/offline; trigger account, all-account, menu, and other explicit refreshes. | Quiet bounded checks; no rejected background popup; Update appears only for a newer applicable version. |
| U02 | Discover a newer release, then lose connectivity or return malformed/feed-error responses. | Known availability remains; background errors stay unobtrusive; install failures expose retry accurately. |
| U03 | Send same/older versions, wrong target, missing signed assets, and malformed manifests. | No false Update availability or unsafe installation; state remains coherent. |
| U04 | Native SDK downloads/installs valid signed update on Linux, Mac, and Windows temporary installs. | Actual installation succeeds, app restarts to exact version, and evidence fingerprints match final artifacts. |
| U05 | Corrupt payload/signature and interrupt download/install/restart. | Verification rejects invalid content; supported recovery works; profiles/backups remain intact. |
| U06 | Update with active profile clients and staged/deferred migrations. | Launcher restart does not force-close clients; active profiles remain deferred; journals resume safely. |
| U07 | Test customized Linux shortcuts, missing Desktop shortcut, linked entry, and alternate portable AppImage. | Owned existing entries refresh safely; custom fields survive; no unwanted shortcut or other-install rewrite. |
| U08 | Test Windows NSIS fresh install, upgrade, uninstall/reinstall, spaces in install path, Start Menu/Desktop shortcuts. | Intended app version/path/shortcuts are correct; user data retention behavior is verified; no sign-in console. |
| U09 | Test Mac bundle replacement, application location, aliases/integration, relaunch, and user-data access. | Correct signed bundle launches; integration survives; no wrong install path or profile reset. |
| U10 | Render each public Windows download CTA and follow redirects/download metadata. | Correct release's `.exe` installer, successful response, matching version/digest; no archive-only CTA. |
| U11 | Inspect final release/feed/site version, native target list, asset names, digests, and signatures. | All refer to the same immutable release/source; supported targets complete. |
| U12 | User starts/refreshes installed 1.5.1 and clicks the new-release Update button. | User-observed progress/install/restart succeeds; six profiles preserved; migration status and shortcuts accurate. |
| U13 | Cold-start the updated app, refresh again, launch migrated/deferred accounts, and inspect backups. | No recurring background popup; current-version Update hidden; correct identity/state; retained current backup still available. |

### 12.9 Regression, performance, and UI checks

- Run the repository's existing Rust, frontend, extension, packaging, and relevant installer/manifest suites against the final change. Run native Windows launch/storage tests on Windows rather than treating Linux compilation as coverage.
- Preserve existing credential locks, account leases, busy-switch protection, cancellation, cleanup safeguards, aliases, workspace handling, and supported Desktop refusal behavior.
- Test migration/storage/maintenance dialogs for accurate scopes, disabled busy actions, progress, retry, failure, rollback, keyboard navigation, and meaningful status updates. Do not expose backend implementation detail in routine user flows unless it helps the user decide.
- Measure cold/warm launches for one, two, and ten account windows; package initialization count; bytes copied; startup latency; catalog paging; checkpoint/hydration latency; memory/CPU; and peak migration disk usage. Record observed values and compare to the same-machine private-layout baseline.
- Use a realistically large chat/asset fixture to detect unbounded scans/copies and blocked UI. Avoid copying full histories on every list request or complete package trees on every account launch.
- Distinguish RAM/CPU costs from disk savings: ten windows still have separate editor/extension/backend processes. Do not advertise a hard resource cap without measurements.
- Exercise reload/reconnect and rapid repeated refresh/launch/switch actions. Ensure no duplicate initialization, queued mutation bypass, leaked child process, stale lock, or uncaught promise error.

### 12.10 Test execution gates

| Gate | Required evidence before proceeding |
| --- | --- |
| Implementation ready for GitHub checks | P0–P3 implemented; local preservation/failure/ownership tests pass; source reviewed; no real-profile migration or local app update. |
| Shared feature ready to enable | Native package immutability/discovery, complete sidebar continuation, private identity, writer fencing, migration/rollback, Copilot, and storage scopes pass on every claimed platform. |
| Auth issue resolved | Real completed browser/device flows are separately recorded; two-account Mac/Windows switching passes; visible Windows terminal observation passes. |
| Release ready | Final-source native acceptance, actual updater/install/restart, signed package/feed consistency, download links, and preservation evidence complete. |
| User update accepted | User performs the installed 1.5.1 Update-button test; six-profile preservation and post-update/deferred migration behavior verified. |

If a platform gate fails, keep sharing/migration disabled for that platform or retain legacy mode while fixing it. Do not convert a cancelled runner, helper-only test, fixture identity, signature-only validation, or browser-initiation test into a broader success claim.

## 13. Code locations and work boundaries

| Area | Primary integration points |
| --- | --- |
| Profile launch/layout/activity | `tauri/src-tauri/src/profiles.rs`, `profiles_windows_tests.rs`, platform executable resolution. |
| Snapshot/path safety | `tauri/src-tauri/src/managed_files.rs`; new migration service and journals. |
| Shared generations | New Rust service integrated with profile launch, account deletion, and maintenance commands. |
| Native history reader/cleanup | `chat_cleanup.rs`; private source-reader isolation and separate global cleanup contract. |
| Credentials/helper/process control | `extension_accounts.rs`, `process.rs`, `lib.rs` sign-in handlers. |
| Bridge lifecycle/ownership/sidebar | `extension/src/bridge.ts`, `bridge-main.ts`, `startup.ts`, `extension.ts`, native helper IPC. |
| API/types/settings | `tauri/src-tauri/src/lib.rs`, `settings.rs`, `tauri/src/lib/types.ts`, `desktop-api.ts`. |
| Product UI | Existing app, storage/settings dialogs, new migration/maintenance/status flows. |
| Update/desktop integration | Native updater integration, `desktop_integration.rs`, Windows integration, package/install tests. |
| Native test runners | `.github/workflows/shared-storage.yml`, `authenticated-accounts.yml`, extension probe/auth scripts, existing release/platform workflows. |
| Public links/releases | `next/lib/site.ts`, install/download components, release pages, established release evidence. |

Preserve strict settings compatibility when adding schema fields. Read applicable directory instructions before website changes. Keep ordinary push behavior: website deployment follows pushes; desktop release builds remain tag-triggered and acceptance workflows manual as configured. Do not add duplicate Actions runs on every source push.

No real environment file has been changed for this work. If future authorized work changes one, follow the repository's Varte sync instructions, confirm the correct destination, and record success or the exact pending blocker without secrets. Private project memory remains under ignored `local-docs/` and is never staged or published.

## 14. Decisions still requiring implementation proof

| Decision | Current direction | Proof needed |
| --- | --- | --- |
| Extension sharing | Private catalogs + versioned shared payloads | Native update/install/cleanup cannot mutate published payload or peers; junction/link GC is safe. |
| Full history route | Broker checkpoints + private native hydration/resume | Native sidebar visibility before resume, full assets/compaction, logical ID stability, and final bundled version compatibility. |
| Writer handoff | Checkpoint then confirmed backend stop/restart | Every loaded thread handled; stale events/owners fenced; no surviving writer can append. |
| Copilot preservation | Keep existing bundled/private configuration | All preserved install/state combinations work under the new catalog. |
| Extra extensions | Minimal initial Codex setup, explicit later managed additions | Migration preview is clear; rollback retains excluded extras until explicit cleanup. |
| Browser OAuth on runners | Real callback forwarding or an interactive native host | Successful callback completion and visible Windows behavior; device-code login is separately recorded. |
| Chat retention/cleanup | Content reachability + tombstones + previews | No lost reachable assets/context, no resurrection, bounded eligible temporary growth. |
| Peak disk and savings | Measure before/after; retain backups and sources | Accurate preview includes all staged/snapshot/runtime copies; savings reported only after actual cleanup. |

One writer per chat and conservative queued maintenance remain proposed defaults, not additional user-confirmed requirements. Resolve ordinary engineering choices within the authorized scope; ask the user only when a product tradeoff changes the requested behavior or real sign-in participation is needed.

## 15. Known tradeoffs

Shared extensions reduce repeated package storage, but separate editor windows still consume memory/CPU and private state can grow. A shared bad package version can affect several accounts, so pinning, integrity checks, staged maintenance, and rollback remain necessary. Safe maintenance may wait for long-running windows to close.

Backups and staged migration temporarily increase disk usage. Shared history adds catalog/checkpoint/asset storage and requires careful retention; no additional chat-space savings are promised before measurement. One writer per chat means another account waits for a safe handoff to continue that exact chat. Separate chats can run concurrently, but their edits to the same project can still conflict.

Initial sharing applies to managed local VS Code accounts. Remote development extension stores and direct shared-chat writes by other clients require separate design and testing. Unsupported histories or external assets must remain accessible in their original source while compatibility is resolved.

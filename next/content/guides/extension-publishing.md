# VS Code Marketplace publishing requirements

The owner authorized Marketplace publication under the existing publisher **russeldanielpaul**. [Multi Codex 0.1.1](https://marketplace.visualstudio.com/items?itemName=russeldanielpaul.multi-codex) is published as a free pre-release for Linux x64, macOS Apple Silicon and macOS Intel. All three packages passed native tests and Marketplace validation. Their public downloads match the tested artifacts byte-for-byte; see [release evidence and checksums](extension-marketplace-verification-2026-10-09.json). The [original 0.1.0 evidence](extension-marketplace-verification-2026-10-08.json) remains available.

Version 0.1.1 fixes desktop account discovery when VS Code inherits a managed account's `CODEX_HOME`, and replaces the welcome buttons with a native account list and inline usage. The first account selection attaches automatically. The corrected reader found all six accounts in the owner's existing desktop store without changing its metadata. Native tests on all three platforms verified live attachment, account switching, chat continuation, activity guards and recovery from storage/helper errors without restarting VS Code, either extension or the Codex backend. The website now links to the public Marketplace listing; the publisher management URL is for the owner only.

## Account and publisher

1. A Microsoft account you control. You complete its sign-in and any verification yourself.
2. A publisher created at [Marketplace publisher management](https://marketplace.visualstudio.com/manage/publishers/), with a permanent unique publisher ID and a display name. The ID cannot be changed after creation.
3. Use that registered ID in `extension/package.json`. This release uses `russeldanielpaul`; the integration harness derives the extension ID and storage path from the manifest.

There is no separate “VS Code account.” Marketplace uses your Microsoft account and publisher identity. Standard extension publishing is free; Codex subscriptions or API usage remain separate. A verified-publisher badge/custom domain is optional.

## Package

The extension needs a manifest, version, README, license, a PNG/JPEG Marketplace icon, and the compiled extension plus a native helper for the declared platform. Repository/support links, a changelog and the icon are included. This release targets Linux x64, macOS Apple Silicon and macOS Intel (macOS 26+), with separate `linux-x64`, `darwin-arm64` and `darwin-x64` VSIX packages under one extension ID. Windows, remote, browser and Linux ARM64 are unsupported.

The native package workflow builds and tests the extracted VSIX on each target operating system, including real VS Code/Codex attachment, identity routing, activity guards and startup restoration. It also checks native Mac Keychain access. Real browser sign-in, credential rotation, quotas and cloud permissions remain separate live-validation limits. The attachment uses private Node hooks and experimental Codex authentication, so publish it with an accurate pre-release description and its queue/API-key limitations. No saved account, auth file, token, local report, or development dependency belongs in the VSIX.

From `extension/`, with Node 24, Rust and the Linux desktop build libraries installed:

```bash
npm ci
npm run build:helper
npm test
npm run test:attachment
npm run package -- --target linux-x64 --pre-release
```

Set `MULTI_CODEX_TEST_ENGINE` to the official VS Code extension's bundled engine when running the native storage and engine integration checks. Run `npm run test:vscode` as well to verify the isolated startup wrapper.

## Upload

For a manual upload, sign in to the publisher management page, choose **New extension → Visual Studio Code**, and upload a tested VSIX. Add the other tested platform packages as updates to the same extension and version. A CLI personal access token is unnecessary for this browser upload. Wait for Marketplace validation for every platform before treating the release as available on that platform.

For command-line publishing, use `vsce` with an authorized Marketplace identity. Microsoft's current documentation recommends Microsoft Entra authentication for automation; global Azure DevOps PATs are scheduled to retire on December 1, 2026. If using an eligible PAT before then, its scope is **Marketplace: Manage**, and it must belong to an account authorized for that publisher. Keep credentials in a secret store, never in source or chat.

For this update, Azure CLI device sign-in completed, but the personal-account `consumers` endpoint rejected the Azure DevOps scope with `AADSTS9002332`. Publication therefore used the existing authenticated Marketplace browser, without a PAT. The owner completed the browser CAPTCHA for the Intel upload.

Microsoft's [publishing guide](https://code.visualstudio.com/api/working-with-extensions/publishing-extension) documents account setup, manual upload, authentication, platform targets, validation and version updates. Users can install the published pre-release or a local VSIX without creating a publisher.

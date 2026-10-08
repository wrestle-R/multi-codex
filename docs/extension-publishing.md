# VS Code Marketplace publishing requirements

Publishing is deferred at the owner's request. No Microsoft account or Marketplace publisher has been created, and this extension has not been uploaded.

## Account and publisher

1. A Microsoft account you control. You complete its sign-in and any verification yourself.
2. A publisher created at [Marketplace publisher management](https://marketplace.visualstudio.com/manage/publishers/), with a permanent unique publisher ID and a display name. The ID cannot be changed after creation.
3. Replace the local `multi-codex-local` placeholder in `extension/package.json` with that registered ID. Rebuild the package and update the integration harness's extension ID/storage path when changing it.

There is no separate “VS Code account.” Marketplace uses your Microsoft account and publisher identity. Standard extension publishing is free; Codex subscriptions or API usage remain separate. A verified-publisher badge/custom domain is optional.

## Package

The extension needs a manifest, version, README, license, a PNG/JPEG Marketplace icon, and the compiled extension plus a native helper for the declared platform. Repository/support links and the icon are included in this source. This preview packages Linux x64 only; do not advertise Windows, remote, browser or macOS compatibility from this result.

Before public distribution, validate browser sign-in, real credential refresh, the target Linux runtime/keyring, and additional supported Codex versions. The attachment is a compatibility implementation using private Node hooks and experimental Codex authentication, so publish it with an accurate preview description and its queue/API-key limitations. No saved account, auth file, token, local report, or development dependency belongs in the VSIX.

From `extension/`, with Node 24, Rust and the Linux desktop build libraries installed:

```bash
npm ci
npm run build:helper
npm test
npm run test:attachment
npm run package -- --target linux-x64
```

Set `MULTI_CODEX_TEST_ENGINE` to the official VS Code extension's bundled engine when running the native storage and engine integration checks. Run `npm run test:vscode` as well to verify the isolated startup wrapper.

## Upload

For a manual upload, sign in to the publisher management page, choose **New extension → Visual Studio Code**, and upload the rebuilt VSIX. A CLI personal access token is unnecessary for this browser upload. Wait for Marketplace validation before treating it as available.

For command-line publishing, use `vsce` with an authorized Marketplace identity. Microsoft's current documentation recommends Microsoft Entra authentication for automation; global Azure DevOps PATs are scheduled to retire on December 1, 2026. If using an eligible PAT before then, its scope is **Marketplace: Manage**, and it must belong to an account authorized for that publisher. Keep credentials in a secret store, never in source or chat.

Microsoft's [publishing guide](https://code.visualstudio.com/api/working-with-extensions/publishing-extension) documents account setup, manual upload, authentication, platform targets, validation and version updates. You can install the local VSIX now without creating a publisher.

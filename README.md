# Multi Codex

Your Codex accounts, your choice of VS Code or the Codex app. Keep your default login untouched.

Screenshots from the running Linux desktop app with fictional demo accounts.

![Multi Codex — light theme](tauri/public/light_screenshot.png)

![Multi Codex — dark theme](tauri/public/dark_screenshot.png)

Install and update use the [latest release](https://github.com/wrestle-R/multi-codex/releases/latest), including its matching installer scripts. Current release: **v1.3.5**.

## Linux

AppImage for Linux x86_64. See [platform support](docs/platform-support.md) for distribution requirements and desktop integrations.

### Install

```bash
curl -fsSL https://github.com/wrestle-R/multi-codex/releases/latest/download/install-app.sh -o /tmp/install-multi-codex.sh
bash /tmp/install-multi-codex.sh
```

### Update

```bash
curl -fsSL https://github.com/wrestle-R/multi-codex/releases/latest/download/update-app.sh -o /tmp/update-multi-codex.sh
bash /tmp/update-multi-codex.sh
```

## Mac

Apple Silicon and macOS 26 or newer. This release is not Apple-signed or notarized; the Mac commands explicitly allow the checksum-verified unsigned build. If macOS blocks first launch, follow [Apple's Open Anyway instructions](https://support.apple.com/en-us/102445).

### Install

```bash
curl -fsSL https://github.com/wrestle-R/multi-codex/releases/latest/download/install-app.sh -o /tmp/install-multi-codex.sh
MULTI_CODEX_ALLOW_UNSIGNED_MAC=1 bash /tmp/install-multi-codex.sh
```

### Update

```bash
curl -fsSL https://github.com/wrestle-R/multi-codex/releases/latest/download/update-app.sh -o /tmp/update-multi-codex.sh
MULTI_CODEX_ALLOW_UNSIGNED_MAC=1 bash /tmp/update-multi-codex.sh
```

Updates replace the app while preserving saved accounts and profile data. Close Multi Codex before updating. To update without launching it, add `MULTI_CODEX_NO_LAUNCH=1` to the command.

Standalone launch supports verified Codex desktop version `26.930.51102`. See [tested platforms and remaining validation](docs/platform-support.md).

Install the Codex extension in each isolated VS Code profile before using Codex there.

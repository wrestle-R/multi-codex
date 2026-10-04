# Multi Codex

Your Codex accounts, your choice of VS Code or the Codex app. Keep your default login untouched.

Screenshots from the running Linux desktop app with fictional demo accounts.

![Multi Codex — light theme](tauri/public/light_screenshot.png)

![Multi Codex — dark theme](tauri/public/dark_screenshot.png)

Install and update always download the [latest published stable release](https://github.com/wrestle-R/multi-codex/releases/latest) for your platform. The commands are not pinned to a version.

The screenshots show the v1.3.5 preview, which is still a draft pending [release validation](docs/releases/v1.3.5-readiness.md). The commands below will download it once it is published as the latest stable release.

## Linux

AppImage for Linux x86_64. See [platform support](docs/platform-support.md) for distribution requirements and desktop integrations.

### Install

```bash
curl -fsSL https://raw.githubusercontent.com/wrestle-R/multi-codex/main/scripts/install-app.sh -o /tmp/install-multi-codex.sh
bash /tmp/install-multi-codex.sh
```

### Update

```bash
curl -fsSL https://raw.githubusercontent.com/wrestle-R/multi-codex/main/scripts/update-app.sh -o /tmp/update-multi-codex.sh
bash /tmp/update-multi-codex.sh
```

## Mac

The scripts choose the Mac installer automatically. The 2.4.x candidates require Apple Silicon and macOS 26 or newer.

### Install

```bash
curl -fsSL https://raw.githubusercontent.com/wrestle-R/multi-codex/main/scripts/install-app.sh -o /tmp/install-multi-codex.sh
bash /tmp/install-multi-codex.sh
```

### Update

```bash
curl -fsSL https://raw.githubusercontent.com/wrestle-R/multi-codex/main/scripts/update-app.sh -o /tmp/update-multi-codex.sh
bash /tmp/update-multi-codex.sh
```

Updates replace the app while preserving saved accounts and profile data. Close Multi Codex before updating. To update without launching it, use `MULTI_CODEX_NO_LAUNCH=1 bash /tmp/update-multi-codex.sh`.

Standalone launch in the v1.3.5 preview supports verified Codex desktop version `26.930.51102`.

Install the Codex extension in each isolated VS Code profile before using Codex there.

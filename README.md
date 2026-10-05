# Multi Codex

Your Codex accounts, your choice of VS Code or the Codex app. Keep your default login untouched.

Screenshots from the running Linux desktop app with fictional demo accounts.

![Multi Codex — light theme](tauri/public/light_screenshot.png)

![Multi Codex — dark theme](tauri/public/dark_screenshot.png)

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

The scripts install the latest **public stable release**, currently v1.2.3. The 1.3.5 preview remains a draft pending [release validation](docs/releases/v1.3.5-validation.json). Standalone launch supports verified Codex desktop version `26.930.51102`.

# Multi Codex

Launch multiple Codex accounts in isolated VS Code windows without changing your default Codex login. Re-launching an account opens another VS Code window, including for the same workspace.

![Current Multi Codex light interface](tauri/public/light_screenshot.png)

![Current Multi Codex dark interface](tauri/public/dark_screenshot.png)

## Safe updates

The installer and updater replace only the application binary. They do not delete or recreate your saved profiles, isolated workspaces, or account credentials. Existing Linux installs in `~/.local/bin/multi-codex.AppImage` are updated in place; new installs use that location as well.

## Arch Linux

Install and launch:

```bash
curl -fsSL https://raw.githubusercontent.com/wrestle-R/multi-codex/main/scripts/install-app.sh -o /tmp/install-multi-codex.sh
bash /tmp/install-multi-codex.sh
```

[Download v2.2.6 for Arch Linux](https://github.com/wrestle-R/multi-codex/releases/download/v2.2.6/Multi.Codex_2.2.6_amd64.AppImage)

## macOS

Install and launch:

```bash
curl -fsSL https://raw.githubusercontent.com/wrestle-R/multi-codex/main/scripts/install-app.sh -o /tmp/install-multi-codex.sh
bash /tmp/install-multi-codex.sh
```

[Download v2.2.6 for macOS](https://github.com/wrestle-R/multi-codex/releases/download/v2.2.6/Multi.Codex_2.2.6_universal.dmg)

## Update

```bash
curl -fsSL https://raw.githubusercontent.com/wrestle-R/multi-codex/main/scripts/update-app.sh -o /tmp/update-multi-codex.sh
bash /tmp/update-multi-codex.sh
```

Set `MULTI_CODEX_NO_LAUNCH=1` when updating from a terminal and you only want to replace the binary. Close and reopen Multi Codex afterward to use the new version.

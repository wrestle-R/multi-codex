# Multi Codex

Manage Codex accounts across VS Code, the Codex app, and CLI while keeping your default login untouched.

## Install

### Windows x64

[Download the Windows installer (.exe)](https://github.com/wrestle-R/multi-codex/releases/download/v1.4.0/Multi.Codex_1.4.0_x64-setup.exe), double-click it, and follow setup. Then open Multi Codex from the Start menu.

Or run in PowerShell:

```powershell
Invoke-WebRequest https://github.com/wrestle-R/multi-codex/releases/latest/download/install-app.ps1 -UseBasicParsing -OutFile "$env:TEMP/install-multi-codex.ps1"
powershell -NoProfile -ExecutionPolicy Bypass -File "$env:TEMP/install-multi-codex.ps1"
```

### Linux x64

```bash
curl -fsSL https://github.com/wrestle-R/multi-codex/releases/latest/download/install-app.sh -o /tmp/install-multi-codex.sh
bash /tmp/install-multi-codex.sh
```

### macOS (Apple Silicon, macOS 26+)

The current macOS release is unsigned; this command explicitly allows its installation.

```bash
curl -fsSL https://github.com/wrestle-R/multi-codex/releases/latest/download/install-app.sh -o /tmp/install-multi-codex.sh
MULTI_CODEX_ALLOW_UNSIGNED_MAC=1 bash /tmp/install-multi-codex.sh
```

## Update

Close Multi Codex before updating. Saved accounts and profile data are preserved.

### Windows x64

Download and run the [Windows installer (.exe)](https://github.com/wrestle-R/multi-codex/releases/download/v1.4.0/Multi.Codex_1.4.0_x64-setup.exe) again to update.

Or run in PowerShell:

```powershell
Invoke-WebRequest https://github.com/wrestle-R/multi-codex/releases/latest/download/update-app.ps1 -UseBasicParsing -OutFile "$env:TEMP/update-multi-codex.ps1"
powershell -NoProfile -ExecutionPolicy Bypass -File "$env:TEMP/update-multi-codex.ps1"
```

### Linux x64

```bash
curl -fsSL https://github.com/wrestle-R/multi-codex/releases/latest/download/update-app.sh -o /tmp/update-multi-codex.sh
bash /tmp/update-multi-codex.sh
```

### macOS (Apple Silicon, macOS 26+)

```bash
curl -fsSL https://github.com/wrestle-R/multi-codex/releases/latest/download/update-app.sh -o /tmp/update-multi-codex.sh
MULTI_CODEX_ALLOW_UNSIGNED_MAC=1 bash /tmp/update-multi-codex.sh
```

<img src="tauri/public/accounts_screenshot.png" alt="Multi Codex accounts" width="100%" />

Multi Codex is independent and is not affiliated with or endorsed by OpenAI.

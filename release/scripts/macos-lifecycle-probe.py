#!/usr/bin/env python3
"""Real packaged macOS launcher lifecycle; every account/path is disposable."""

import hashlib
import json
import os
from pathlib import Path
import plistlib
import shlex
import subprocess
import sys
import tempfile
import time
import uuid

application = Path(sys.argv[1]).resolve(strict=True)
variant = os.environ.get("MAC_LIFECYCLE_VARIANT", "candidate")
evidence = Path(os.environ["RUNNER_TEMP"]) / "hud-backend-evidence" / variant
evidence.mkdir(parents=True, exist_ok=True)
with (application / "Contents/Info.plist").open("rb") as source:
    info = plistlib.load(source)
binary = application / "Contents/MacOS" / info["CFBundleExecutable"]
report = {
    "passed": False,
    "variant": variant,
    "appPath": str(application),
    "version": info.get("CFBundleShortVersionString"),
    "build": info.get("CFBundleVersion"),
    "source": os.environ.get("GITHUB_SHA"),
    "os": subprocess.check_output(["sw_vers"], text=True),
    "binarySha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
    "checks": [],
}
(evidence / "identity.json").write_text(json.dumps(report, indent=2) + "\n")
print(
    json.dumps(
        {key: report[key] for key in ("appPath", "version", "build", "variant")}
    ),
    flush=True,
)


def applescript(source):
    result = subprocess.run(
        ["osascript", "-e", source], capture_output=True, text=True, timeout=20
    )
    if result.returncode:
        raise RuntimeError(result.stderr.strip())
    return result.stdout.strip()


def wait_for(label, condition, timeout=30):
    deadline = time.monotonic() + timeout
    last = None
    while time.monotonic() < deadline:
        try:
            if condition():
                return
        except (RuntimeError, ValueError) as error:
            last = str(error)
        time.sleep(0.4)
    raise RuntimeError(f"Timed out: {label}; last native UI error: {last}")


process = None
with tempfile.TemporaryDirectory(prefix="Multi Codex lifecycle 測試 ") as temporary:
    root = Path(temporary)
    data = root / "accounts"
    global_home = root / ".codex"
    workspace = root / "workspace"
    profile_id = str(uuid.uuid4())
    profile = data / "profiles" / profile_id / "codex-home"
    profile.mkdir(parents=True)
    global_home.mkdir()
    workspace.mkdir()
    (profile / "sessions").mkdir()
    (data / "migrations").mkdir()
    fixtures = {
        data
        / "profiles.json": json.dumps(
            [
                {
                    "id": profile_id,
                    "name": "Disposable lifecycle account",
                    "authMode": "apikey",
                    "createdAt": "2026-10-14T00:00:00Z",
                    "updatedAt": "2026-10-14T00:00:00Z",
                }
            ]
        ).encode(),
        data
        / "executables.json": json.dumps(
            {
                "onboardingCompleted": True,
                "launchMode": "both",
                "hideDesktopPicker": True,
                "preferredWorkspace": str(workspace),
            }
        ).encode(),
        profile
        / "auth.json": b'{"auth_mode":"apikey","OPENAI_API_KEY":"inert-lifecycle-fixture"}',
        profile
        / "sessions"
        / "untouched.txt": b"not a conversation; preservation sentinel\n",
        global_home / "auth.json": b'{"testSentinel":"not-a-credential"}\n',
        global_home / "config.toml": b"# untouched global settings sentinel\n",
    }
    for path, content in fixtures.items():
        path.write_bytes(content)
        path.chmod(0o600)
    (data / "migrations/sharing-queue.json").write_text(
        json.dumps({"schema": 1, "enabled": False, "profiles": {}})
    )
    env = {
        key: value
        for key, value in os.environ.items()
        if not key.startswith(("CODEX_", "VSCODE_", "OPENAI_", "MULTI_CODEX_"))
        and key not in ("ELECTRON_RUN_AS_NODE", "NODE_OPTIONS")
    }
    env.update(
        CODEX_HOME=str(global_home),
        MULTI_CODEX_DATA_DIR=str(data),
        XDG_DATA_HOME=str(root / "xdg-data"),
        XDG_CONFIG_HOME=str(root / "config"),
        PATH="/usr/bin:/bin:/usr/sbin:/sbin",
    )
    # Keep the native runner HOME so Keychain Services resolves the user
    # domain correctly; explicit app/Codex/XDG paths remain disposable.
    # Provision a private Keychain for this fixture
    # so account callbacks exercise the real credential backup, not an absent
    # or locked runner keychain. Restore the runner selection before disposal.
    assert os.environ.get("GITHUB_ACTIONS") == "true", "Native Keychain selection is runner-only"
    def security(*arguments, check=True):
        return subprocess.run(
            ["/usr/bin/security", *arguments], check=check,
            capture_output=True, text=True, timeout=20,
        )

    previous_default = security("default-keychain", "-d", "user", check=False)
    previous_search = security("list-keychains", "-d", "user", check=False)
    old_default = shlex.split(previous_default.stdout)
    old_search = shlex.split(previous_search.stdout)
    keychain = root / "fixture.keychain-db"
    fixture_password = "disposable-lifecycle-keychain"
    security("create-keychain", "-p", fixture_password, str(keychain))
    security("set-keychain-settings", "-lut", "3600", str(keychain))
    security("unlock-keychain", "-p", fixture_password, str(keychain))
    security("default-keychain", "-d", "user", "-s", str(keychain))
    security("list-keychains", "-d", "user", "-s", str(keychain), *old_search)
    report["disposableNativeKeychain"] = True
    if probe := os.environ.get("MAC_KEYCHAIN_PROBE"):
        diagnostic = subprocess.run([probe], env=env, capture_output=True, text=True, timeout=20)
        report["nativeKeychainDiagnostic"] = json.loads(diagnostic.stdout)
    log = (evidence / "application.log").open("wb")

    def start():
        global process
        process = subprocess.Popen(
            [str(binary)], cwd=workspace, env=env, stdout=log, stderr=subprocess.STDOUT
        )
        wait_for("one visible launcher", lambda: launcher_count() == 1)
        foreground()
        wait_for("visible menu-bar icon", lambda: menu_icon_count() == 1)
        if variant == "candidate" and tuple(map(int, report["version"].split("."))) >= (1, 7, 0):
            receipt = data / "release-notes.json"
            if not receipt.exists():
                def dismiss_upgrade_notes():
                    return ui('set elements to entire contents of window "Multi Codex"\nrepeat with element in elements\ntry\nif role of element is "AXButton" and name of element is "Got it" then\nperform action "AXPress" of element\nreturn "dismissed"\nend if\nend try\nend repeat\nreturn "waiting"') == "dismissed"
                wait_for("minimal upgrade notes", dismiss_upgrade_notes)
                wait_for("durable upgrade acknowledgement", lambda: receipt.is_file() and json.loads(receipt.read_text())["version"] == report["version"])
                report["checks"].append({"minimalUpgradeNotesAcknowledged": True})
        return process.pid

    def ui(command):
        return applescript(
            f'tell application "System Events" to tell first application process whose unix id is {process.pid}\n{command}\nend tell'
        )

    def launcher_count():
        return int(ui('return count of (windows whose name is "Multi Codex")'))

    def hud_count():
        return int(ui('return count of (windows whose name is "Multi Codex Orbit")'))

    def menu_icon_count():
        return int(ui("return count of menu bar items of menu bar 2"))

    def process_ids():
        listing = subprocess.check_output(
            ["/bin/ps", "-axo", "pid=,command="], text=True
        )
        return [
            int(line.split(maxsplit=1)[0])
            for line in listing.splitlines()
            if len(line.split(maxsplit=1)) == 2
            and line.split(maxsplit=1)[1] == str(binary)
        ]

    def open_menu():
        ui("click menu bar item 1 of menu bar 2")

    def account_menu_once():
        open_menu()
        ui(
            'click menu item "Disposable lifecycle account" of menu 1 of menu bar item 1 of menu bar 2'
        )
        names = ui(
            'return name of every menu item of menu 1 of menu item "Disposable lifecycle account" of menu 1 of menu bar item 1 of menu bar 2'
        )
        for expected in ("Open VS Code", "Open Codex", "Open CLI"):
            if expected not in names:
                raise RuntimeError(f"Missing installed account action: {expected}")
        ui("key code 53")
        ui("key code 53")

    def account_menu():
        def ready():
            account_menu_once()
            return True
        wait_for("native account menu actions", ready)

    def fullscreen(target):
        def settled():
            ui('set value of attribute "AXFullScreen" of window "Multi Codex" to ' + str(target).lower())
            return ui('return value of attribute "AXFullScreen" of window "Multi Codex"') == str(target).lower()
        wait_for("native fullscreen transition", settled)

    def foreground():
        ui('set frontmost to true\nperform action "AXRaise" of window "Multi Codex"')

    def close(kind):
        foreground()
        if kind == "red-button":
            ui(
                'click first button of window "Multi Codex" whose subrole is "AXCloseButton"'
            )
        else:
            ui('keystroke "w" using command down')
        wait_for(f"{kind} hides launcher", lambda: launcher_count() == 0)
        if process.poll() is not None or process_ids() != [process.pid]:
            raise RuntimeError("Closing launcher exited or duplicated its process")
        if menu_icon_count() != 1:
            raise RuntimeError("Closing launcher removed the menu-bar icon")
        account_menu()

    def reopen(kind):
        if kind == "menu-bar":
            open_menu()
            ui(
                'click menu item "Open Multi Codex" of menu 1 of menu bar item 1 of menu bar 2'
            )
        else:
            # LaunchServices is the native Dock reopen route for the registered
            # packaged bundle; also assert no second process/window is created.
            subprocess.run(["open", "-a", str(application)], check=True, timeout=20)
        wait_for(f"{kind} reopens one launcher", lambda: launcher_count() == 1)
        if process_ids() != [process.pid]:
            raise RuntimeError("Reopening created a duplicate launcher process")

    try:
        original = start()
        account_menu()
        if variant == "candidate":
            wait_for("compact native HUD visible", lambda: hud_count() == 1)
            dimensions = [
                int(value.strip())
                for value in ui('return size of window "Multi Codex Orbit"').split(",")
            ]
            if len(dimensions) != 2 or not (
                32 <= dimensions[0] < 400 and 32 <= dimensions[1] < 150
            ):
                raise RuntimeError(f"Unexpected compact HUD size: {dimensions}")
            foreground()
            ui('keystroke "h" using {command down, shift down}')
            wait_for("native shortcut hides HUD", lambda: hud_count() == 0)
            ui('keystroke "h" using {command down, shift down}')
            wait_for("native shortcut restores HUD", lambda: hud_count() == 1)
            fullscreen(True)
            wait_for("fullscreen hides native HUD", lambda: hud_count() == 0)
            fullscreen(False)
            wait_for("ordinary Space restores native HUD", lambda: hud_count() == 1)
            report["checks"].append(
                {
                    "nativeHudDimensions": dimensions,
                    "shortcutHideRestore": True,
                    "fullscreenHideRestore": True,
                }
            )
        for cycle in range(1, 4):
            for kind, via in [("red-button", "menu-bar"), ("command-w", "dock")]:
                close(kind)
                reopen(via)
                report["checks"].append(
                    {
                        "cycle": cycle,
                        "close": kind,
                        "reopen": via,
                        "samePid": process.pid == original,
                        "oneLauncher": True,
                        "menuVisible": True,
                        "accountSubmenuUsable": True,
                    }
                )
        subprocess.run(
            ["/usr/sbin/screencapture", "-x", str(evidence / "launcher-and-menu.png")],
            check=True,
        )
        # Exercise a real account menu callback. It must still create the
        # isolated editor instance after repeated close/reopen cycles.
        open_menu()
        ui(
            'click menu item "Disposable lifecycle account" of menu 1 of menu bar item 1 of menu bar 2'
        )
        ui(
            'click menu item "Open VS Code" of menu 1 of menu item "Disposable lifecycle account" of menu 1 of menu bar item 1 of menu bar 2'
        )

        # The modal traps focus with its close button first and folder choice
        # last. Use native keyboard input because System Events does not always
        # enumerate descendants of WKWebView's accessibility web area.
        time.sleep(1)
        foreground()
        ui("key code 48 using shift down")
        ui("keystroke space")
        wait_for(
            "account menu launches isolated editor",
            lambda: any(
                profile_id in line and "Code" in line
                for line in subprocess.check_output(
                    ["/bin/ps", "-axo", "command="], text=True
                ).splitlines()
            ),
            timeout=60,
        )
        report["checks"].append({"accountLaunchAfterCloseCycles": True})
        for quit_kind in ("menu-bar", "command-q", "dock-quit"):
            if process.poll() is not None:
                start()
            if quit_kind == "menu-bar":
                open_menu()
                ui(
                    'click menu item "Quit Multi Codex" of menu 1 of menu bar item 1 of menu bar 2'
                )
            elif quit_kind == "command-q":
                foreground()
                ui('keystroke "q" using command down')
            else:
                def quit_from_dock():
                    return applescript(
                        'tell application "System Events" to tell process "Dock"\n'
                        'if not (exists menu 1 of UI element "Multi Codex" of list 1) then perform action "AXShowMenu" of UI element "Multi Codex" of list 1\n'
                        'if exists menu item "Quit" of menu 1 of UI element "Multi Codex" of list 1 then\n'
                        'click menu item "Quit" of menu 1 of UI element "Multi Codex" of list 1\nreturn "clicked"\n'
                        'end if\nreturn "waiting"\nend tell'
                    ) == "clicked"
                wait_for("Dock Quit menu available", quit_from_dock)
            if process.wait(timeout=15) != 0 or process_ids():
                raise RuntimeError(f"{quit_kind} did not exit cleanly")
            report["checks"].append(
                {"quit": quit_kind, "cleanExit": True, "noDuplicateProcess": True}
            )
        for path, expected in fixtures.items():
            if path.read_bytes() != expected:
                raise RuntimeError(
                    f"Lifecycle test changed protected fixture: {path.name}"
                )
        report.update(passed=True, fixtureBytesUnchanged=True)
    except Exception as error:
        report["failure"] = str(error)
        report["processAlive"] = process is not None and process.poll() is None
        if report["processAlive"]:
            try:
                report["accessibleWindowNames"] = ui("return name of every window")
                subprocess.run(
                    ["/usr/sbin/screencapture", "-x", str(evidence / "failure.png")],
                    check=True,
                )
            except Exception as diagnostic_error:
                report["diagnosticFailure"] = str(diagnostic_error)
        raise
    finally:
        if process and process.poll() is None:
            process.terminate()
            try:
                process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=5)
        # Stop only editor/engine processes whose command names this private
        # fixture. No global application Quit or account sign-out is used.
        listing = subprocess.check_output(
            ["/bin/ps", "-axo", "pid=,command="], text=True
        )
        for line in listing.splitlines():
            fields = line.split(maxsplit=1)
            if len(fields) == 2 and str(root) in fields[1]:
                pid = int(fields[0])
                if pid != os.getpid():
                    try:
                        os.kill(pid, 15)
                    except ProcessLookupError:
                        pass
        log.close()
        if old_default:
            security("default-keychain", "-d", "user", "-s", *old_default)
        security("list-keychains", "-d", "user", "-s", *old_search)
        security("delete-keychain", str(keychain))
        report["disposableKeychainRemoved"] = not keychain.exists()
        subprocess.run(
            [
                "/usr/bin/log",
                "show",
                "--last",
                "10m",
                "--style",
                "compact",
                "--predicate",
                'process == "multi-codex-desktop" OR process == "Multi Codex"',
            ],
            stdout=(evidence / "lifecycle.log").open("wb"),
            stderr=subprocess.DEVNULL,
            timeout=30,
        )
        reports = Path.home() / "Library/Logs/DiagnosticReports"
        report["crashReports"] = [
            path.name for path in reports.glob("*") if "codex" in path.name.lower()
        ]
        (evidence / "result.json").write_text(json.dumps(report, indent=2) + "\n")

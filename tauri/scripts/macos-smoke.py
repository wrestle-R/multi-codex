#!/usr/bin/env python3
"""Exercise a packaged app and restart in disposable macOS paths; never use runner/user accounts."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time

binary = Path(sys.argv[1]).resolve()
report_dir = Path(os.environ["RUNNER_TEMP"]) / "multi-codex-macos-evidence"
report_dir.mkdir(exist_ok=True)
with tempfile.TemporaryDirectory(prefix="Multi Codex Mac 測試 ") as temporary:
    home = Path(temporary)
    data = home / "Library/Application Support/multi-codex"
    data.mkdir(parents=True)
    (home / ".codex").mkdir()
    # Inert sentinels: do not contain tokens and must remain byte-for-byte intact.
    fixtures = {
        data / "profiles.json": b"[]\n",
        data / "executables.json": b'{"codePath":null,"codexPath":null,"globalCodexHome":null}\n',
        home / ".codex/auth.json": b'{"testSentinel":"not-a-credential"}\n',
        home / ".codex/config.toml": b'# untouched global config\n',
    }
    for path, contents in fixtures.items():
        path.write_bytes(contents)
        path.chmod(0o600)
    env = {key: value for key, value in os.environ.items()
           if not key.startswith(("CODEX_", "VSCODE_", "OPENAI_"))
           and key not in ("ELECTRON_RUN_AS_NODE", "NODE_OPTIONS")}
    env.update(HOME=str(home), XDG_DATA_HOME=str(home / "data"), PATH="/usr/bin:/bin:/usr/sbin:/sbin")
    # Query Quartz window state: process survival alone does not prove window creation.
    inspector = home / "window.swift"
    inspector.write_text('''import CoreGraphics
import Foundation
let pid = Int32(CommandLine.arguments[1])!
let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]] ?? []
let visible = windows.filter { ($0[kCGWindowOwnerPID as String] as? Int32) == pid && ($0[kCGWindowLayer as String] as? Int) == 0 }
guard !visible.isEmpty else { exit(1) }
print("Visible native window confirmed")
''')
    inspector_binary = home / "window-inspector"
    subprocess.run(["/usr/bin/swiftc", str(inspector), "-o", str(inspector_binary)], check=True)
    runs = []
    for attempt in (1, 2):
        log_path = report_dir / f"startup-{attempt}.log"
        with log_path.open("wb") as log:
            process = subprocess.Popen([str(binary)], env=env, stdout=log, stderr=subprocess.STDOUT)
            try:
                deadline = time.monotonic() + 30
                while time.monotonic() < deadline:
                    if process.poll() is not None:
                        raise RuntimeError(f"Packaged app exited at startup ({process.returncode})")
                    inspection = subprocess.run([str(inspector_binary), str(process.pid)], capture_output=True)
                    if inspection.returncode == 0:
                        break
                    time.sleep(.5)
                else:
                    raise RuntimeError("Packaged app did not create a visible window")
                time.sleep(5)
                assert process.poll() is None, "Packaged app exited after window startup"
                # Runner is disposable and contains only the test app's signed-out UI.
                subprocess.run(["/usr/sbin/screencapture", "-x", str(report_dir / f"startup-{attempt}.png")], check=True)
                runs.append({"startup": attempt, "visibleWindow": True, "stayedRunning": True})
            finally:
                if process.poll() is None:
                    process.terminate()
                    try:
                        process.wait(timeout=10)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait(timeout=5)
        for path, expected in fixtures.items():
            assert path.read_bytes() == expected, f"Startup changed fixture: {path.name}"
    (report_dir / "result.json").write_text(json.dumps({
        "runs": runs,
        "fixtureHashesUnchanged": {str(path.relative_to(home)): hashlib.sha256(contents).hexdigest()
                                   for path, contents in fixtures.items()},
        "authenticatedDesktopIsolation": "not tested",
        "signingAndNotarization": "separate release gate",
    }, indent=2) + "\n")
print("Packaged Mac window, restart, empty GUI PATH, Unicode paths and data preservation passed")

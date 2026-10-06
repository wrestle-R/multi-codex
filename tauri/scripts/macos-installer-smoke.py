#!/usr/bin/env python3
"""Test the real released DMG installer with disposable account-data sentinels."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

assets = Path(sys.argv[1]).resolve()
repository = Path(__file__).resolve().parents[2]
tag = sys.argv[2]
version = tag.removeprefix("v")
asset = f"Multi.Codex_{version}_aarch64.dmg"
expected = next(line.split()[0] for line in (assets / "SHA256SUMS").read_text().splitlines()
                if line.split()[1] == asset)
assert hashlib.sha256((assets / asset).read_bytes()).hexdigest() == expected
report_dir = Path(os.environ["RUNNER_TEMP"]) / "multi-codex-macos-installer-evidence"
report_dir.mkdir(exist_ok=True)
results = []
with tempfile.TemporaryDirectory(prefix="Multi Codex installer 測試 ") as temporary:
    task = Path(temporary)
    bin_dir = task / "bin"
    bin_dir.mkdir()
    # Resolve the not-yet-published release locally, using the actual downloaded
    # draft DMG. All install/copy/signing operations below use real macOS tools.
    curl = bin_dir / "curl"
    curl.write_text('''#!/usr/bin/env python3
import os, pathlib, shutil, sys
args = sys.argv[1:]
if any("url_effective" in arg for arg in args):
    print("https://github.com/wrestle-R/multi-codex/releases/tag/" + os.environ["INSTALLER_TEST_TAG"])
else:
    url = next(arg for arg in args if arg.startswith("https://"))
    output = args[args.index("--output") + 1]
    shutil.copyfile(pathlib.Path(os.environ["INSTALLER_TEST_ASSETS"]) / url.rsplit("/", 1)[1], output)
''')
    curl.chmod(0o755)
    for script in ("install-app.sh", "update-app.sh"):
        fixture = task / script
        fixture.mkdir()
        application = fixture / "Applications/Multi Codex.app"
        application.mkdir(parents=True)
        previous = application / "previous-app-sentinel"
        previous.write_bytes(b"previous app")
        data = fixture / "Library/Application Support/multi-codex"
        home = fixture / ".codex"
        home.mkdir()
        profile = data / "profiles/00000000-0000-4000-8000-000000000001/codex-home"
        (profile / "sessions").mkdir(parents=True)
        protected = {
            data / "profiles.json": b"[]\n",
            data / "executables.json": b'{"onboardingCompleted":false}\n',
            profile / "auth.json": b'{"testSentinel":"not-a-credential"}\n',
            profile / "sessions/keep.jsonl": b"test-only conversation sentinel\n",
            home / "auth.json": b'{"testSentinel":"not-a-credential"}\n',
            home / "config.toml": b"# test-only default settings\n",
        }
        for path, contents in protected.items():
            path.write_bytes(contents)
            path.chmod(0o600)
        env = {key: value for key, value in os.environ.items()
               if not key.startswith(("CODEX_", "VSCODE_", "OPENAI_"))
               and key not in ("ELECTRON_RUN_AS_NODE", "NODE_OPTIONS", "MULTI_CODEX_ALLOW_UNSIGNED_MAC")}
        env.update(HOME=str(fixture), PATH=str(bin_dir) + ":/usr/bin:/bin:/usr/sbin:/sbin",
                   MULTI_CODEX_NO_LAUNCH="1", INSTALLER_TEST_ASSETS=str(assets), INSTALLER_TEST_TAG=tag)
        strict = subprocess.run(["/bin/bash", str(repository / "scripts" / script)], env=env,
                                capture_output=True, text=True, timeout=180)
        assert strict.returncode != 0, "Unsigned install must require explicit opt-in"
        assert previous.read_bytes() == b"previous app", "Rejected install replaced the previous app"
        assert not list((fixture / "Applications").glob(".Multi Codex.app.new.*")), "Rejected staging was not cleaned"
        for path, contents in protected.items():
            assert path.read_bytes() == contents, f"Rejected install changed {path.name}"
        env["MULTI_CODEX_ALLOW_UNSIGNED_MAC"] = "1"
        installed = subprocess.run(["/bin/bash", str(repository / "scripts" / script)], env=env,
                                  capture_output=True, text=True, timeout=180)
        if installed.returncode:
            raise RuntimeError(f"{script}: {installed.stdout}\n{installed.stderr}")
        assert not previous.exists(), "Verified install did not replace the previous app"
        installed_version = subprocess.check_output(["/usr/libexec/PlistBuddy", "-c", "Print :CFBundleShortVersionString",
                                                     str(application / "Contents/Info.plist")], text=True).strip()
        assert installed_version == version
        subprocess.run(["/usr/bin/codesign", "--verify", "--deep", "--strict", str(application)], check=True)
        for path, contents in protected.items():
            assert path.read_bytes() == contents, f"Install changed {path.name}"
        assert not list((fixture / "Applications").glob(".Multi Codex.app.*")), "Install staging was not cleaned"
        executable = subprocess.check_output(["/usr/libexec/PlistBuddy", "-c", "Print :CFBundleExecutable",
                                              str(application / "Contents/Info.plist")], text=True).strip()
        subprocess.run([sys.executable, str(repository / "tauri/scripts/macos-smoke.py"),
                        str(application / "Contents/MacOS" / executable)], env=env, check=True, timeout=180)
        results.append({"script": script, "strictUnsignedRejection": True, "explicitUnsignedInstall": True,
                        "installedVersion": installed_version, "signatureIntegrity": True,
                        "accountAndChatSentinelsUnchanged": True, "installedNativeStartupAndRestart": True,
                        "stagingCleaned": True})
(report_dir / "result.json").write_text(json.dumps({"tag": tag, "dmgSha256": expected,
    "checks": results, "notarization": "not available; first-open approval remains controlled by macOS"}, indent=2) + "\n")
print("Actual Mac DMG install, update, startup/restart, rejection and data preservation passed")

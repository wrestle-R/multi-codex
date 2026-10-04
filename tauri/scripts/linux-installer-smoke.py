#!/usr/bin/env python3
"""Install the real released AppImage into temporary homes without launching it."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

assets = Path(sys.argv[1]).resolve()
tag = sys.argv[2]
version = tag.removeprefix("v")
repository = Path(__file__).resolve().parents[2]
name = f"Multi.Codex_{version}_amd64.AppImage"
expected = next(line.split()[0] for line in (assets / "SHA256SUMS").read_text().splitlines()
                if line.split()[1] == name)
assert hashlib.sha256((assets / name).read_bytes()).hexdigest() == expected
checks = []
with tempfile.TemporaryDirectory(prefix="multi-codex-linux-installer-") as temporary:
    task = Path(temporary)
    bin_dir = task / "bin"
    bin_dir.mkdir()
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
        application = fixture / ".local/bin/multi-codex.AppImage"
        application.parent.mkdir(parents=True)
        application.write_bytes(b"previous app")
        profile = fixture / ".local/share/multi-codex/profiles/test/codex-home"
        (profile / "sessions").mkdir(parents=True)
        default = fixture / ".codex"
        default.mkdir()
        protected = {profile / "auth.json": b"test credential sentinel", profile / "sessions/keep.jsonl": b"test chat sentinel",
                     profile.parents[2] / "profiles.json": b"test account sentinel", default / "auth.json": b"default login sentinel"}
        for path, content in protected.items():
            path.write_bytes(content)
        env = {key: value for key, value in os.environ.items() if not key.startswith(("CODEX_", "OPENAI_", "VSCODE_"))}
        env.update(HOME=str(fixture), PATH=str(bin_dir) + ":" + os.environ["PATH"], MULTI_CODEX_NO_LAUNCH="1",
                   INSTALLER_TEST_ASSETS=str(assets), INSTALLER_TEST_TAG=tag)
        result = subprocess.run(["/bin/bash", str(repository / "scripts" / script)], env=env,
                                capture_output=True, text=True, timeout=180)
        if result.returncode:
            raise RuntimeError(f"{script}: {result.stdout}\n{result.stderr}")
        assert hashlib.sha256(application.read_bytes()).hexdigest() == expected
        assert os.access(application, os.X_OK)
        for path, content in protected.items():
            assert path.read_bytes() == content, f"Installer changed {path.name}"
        checks.append({"script": script, "exactReleasedAppImageInstalled": True,
                       "accountAndChatSentinelsUnchanged": True, "defaultLoginUnchanged": True})
report = {"tag": tag, "appImageSha256": expected, "checks": checks,
          "scope": "Actual AppImage replacement without launch; native launch and isolation are separate completed checks"}
output = Path(sys.argv[3])
output.parent.mkdir(parents=True, exist_ok=True)
output.write_text(json.dumps(report, indent=2) + "\n")
print(json.dumps(report, indent=2))

#!/usr/bin/env python3
"""Prove the old event handler fails and the fixed native lifecycle passes."""
import os
from pathlib import Path
import subprocess
import sys
import tempfile

binary = str(Path(sys.argv[1]).resolve())
with tempfile.TemporaryDirectory(prefix="multi-codex-tray-") as temporary:
    env = dict(os.environ, HOME=temporary, XDG_DATA_HOME=temporary)
    before = subprocess.run(
        [binary, "--without-exit-guard"], env=env, capture_output=True, text=True, timeout=45
    )
    assert before.returncode != 0, "Regression check did not detect the missing exit guard"
    assert "automatic last-window exit stopped the tray process" in before.stderr, before.stderr
    print("Confirmed old handler fails: automatic last-window exit stopped the tray process")
    subprocess.run([binary], env=env, check=True, timeout=45)

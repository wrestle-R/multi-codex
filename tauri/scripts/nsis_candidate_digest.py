"""Fingerprint Tauri's exact NSIS payload from the restored raw build binary.

Tauri patches the first bundle-type token before packaging, then restores the
original build binary. Reproduce only that documented fixed-length patch; every
other byte must match. Authenticode signing needs its own packaged reference.
"""
import hashlib
from pathlib import Path
import sys


def expected_nsis_digest(binary):
    data = bytearray(Path(binary).read_bytes())
    token = b"__TAURI_BUNDLE_TYPE_VAR_UNK"
    index = data.find(token)
    if index < 0:
        raise ValueError("Unpatched Tauri bundle marker is missing")
    data[index:index + len(token)] = b"__TAURI_BUNDLE_TYPE_VAR_NSS"
    return hashlib.sha256(data).hexdigest()


if __name__ == "__main__":
    print(expected_nsis_digest(sys.argv[1]))

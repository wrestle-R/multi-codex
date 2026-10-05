#!/usr/bin/env python3
"""Disposable desktop isolation probes. Never import Multi Codex's saved accounts.

smoke exercises signed-out instances only; it cannot approve authenticated isolation.
start leaves two temporary windows open for interactive test-account sign-in.
Only result.json may be uploaded from a probe: logs/state can contain credentials.
"""
import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import struct
import sys
import tempfile
import time

MARKER = "multi-codex-disposable-desktop-v1"


def desktop_version(binary):
    for archive in (binary.parent / "resources/app.asar", binary.parent.parent / "Resources/app.asar"):
        if not archive.is_file():
            continue
        with archive.open("rb") as source:
            header = struct.unpack("<4I", source.read(16))
            if header[3] > 32 * 1024 * 1024:
                raise RuntimeError("Desktop package header is unexpectedly large")
            tree = json.loads(source.read(header[3]))
            entry = tree["files"]["package.json"]
            if entry.get("unpacked") or entry["size"] > 65536:
                raise RuntimeError("Desktop package metadata is unavailable")
            source.seek(8 + header[1] + int(entry["offset"]))
            return json.loads(source.read(entry["size"]))["version"]
    raise RuntimeError("Desktop package metadata was not found")


def write_json(path, data):
    temporary = path.with_suffix(".tmp")
    temporary.write_text(json.dumps(data, indent=2) + "\n")
    temporary.chmod(0o600)
    temporary.replace(path)


def load(root):
    if root.is_symlink():
        raise RuntimeError("Refusing a symlinked probe directory")
    root = root.resolve(strict=True)
    state = json.loads((root / "probe.json").read_text())
    if state.get("marker") != MARKER or state.get("root") != str(root):
        raise RuntimeError("Refusing a directory not created by this disposable probe")
    if root.stat().st_uid != os.getuid() or root.stat().st_mode & 0o077:
        raise RuntimeError("Probe directory ownership is invalid")
    for name in ("a", "b"):
        for path in (root / name, root / name / ".codex", root / name / "desktop-data"):
            if path.is_symlink() or not path.is_dir():
                raise RuntimeError("Probe paths are not private disposable directories")
    return state


def environment(root, name):
    home = root / name
    env = {key: value for key, value in os.environ.items()
           if not key.startswith(("CODEX_", "OPENAI_", "VSCODE_"))
           and key not in ("NODE_OPTIONS", "ELECTRON_RUN_AS_NODE")}
    env.update(HOME=str(home), CODEX_HOME=str(home / ".codex"),
               CODEX_ELECTRON_USER_DATA_PATH=str(home / "desktop-data"),
               CODEX_SQLITE_HOME=str(home / ".codex"),
               XDG_CONFIG_HOME=str(home / "config"), XDG_DATA_HOME=str(home / "data"),
               XDG_CACHE_HOME=str(home / "cache"))
    return env


def launch(root, state, name):
    # Interactive windows must not crash in console.write if a log fills /tmp.
    destination = root / name / "private-startup.log" if state.get("captureLogs", True) else Path(os.devnull)
    with destination.open("ab") as log:
        state.setdefault("logOffsets", {})[name] = log.tell()
        arguments = [state["binary"], "--user-data-dir=" + str(root / name / "desktop-data")]
        if state.get("inspectable", False):
            arguments.extend(["--remote-debugging-port=0", "--remote-debugging-address=127.0.0.1"])
        process = subprocess.Popen(
            arguments,
            env=environment(root, name), cwd=root / name, stdin=subprocess.DEVNULL,
            stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
    state["pids"][name] = process.pid
    write_json(root / "probe.json", state)
    return process


def owns_process(root, state, name):
    pid = state["pids"].get(name)
    if not pid:
        return False
    command = subprocess.run(["ps", "-p", str(pid), "-o", "args="],
                             capture_output=True, text=True, check=False).stdout
    return "--user-data-dir=" + str(root / name / "desktop-data") in command


def stop(root, state, name):
    if owns_process(root, state, name):
        pid = state["pids"][name]
        if os.getpgid(pid) != pid:
            raise RuntimeError("Refusing to stop a process outside the probe's group")
        os.killpg(pid, signal.SIGTERM)
        deadline = time.monotonic() + 10
        while owns_process(root, state, name) and time.monotonic() < deadline:
            time.sleep(.2)
        if owns_process(root, state, name):
            os.killpg(pid, signal.SIGKILL)
    state["pids"].pop(name, None)
    write_json(root / "probe.json", state)


def fingerprint(path):
    return hashlib.sha256(path.read_bytes()).hexdigest() if path.is_file() else None


def status(root, state):
    result = {}
    for name in ("a", "b"):
        auth = root / name / ".codex/auth.json"
        identity = None
        if auth.is_file():
            # Hash identity claims only; do not print tokens, emails or account IDs.
            try:
                data = json.loads(auth.read_text())
                token = data.get("tokens", {}).get("id_token", "")
                payload = token.split(".")[1]
                claims = json.loads(base64.urlsafe_b64decode(payload + "=" * (-len(payload) % 4)))
                identity = hashlib.sha256(str(claims["sub"]).encode()).hexdigest()
            except (ValueError, KeyError, IndexError, TypeError):
                pass
        cookies = list((root / name / "desktop-data").rglob("Cookies"))
        result[name] = {"running": owns_process(root, state, name),
                        "authPresent": auth.is_file(), "authHash": fingerprint(auth),
                        "identityHash": identity,
                        "cookieDatabaseInodes": [p.stat().st_ino for p in cookies if p.is_file()],
                        "sentinelUnchanged": (root / name / "sentinel.txt").read_text() == name}
    return result


def initialized(root, state, name):
    with (root / name / "private-startup.log").open("rb") as source:
        source.seek(state["logOffsets"][name])
        log = source.read().decode(errors="replace")
    return "window" in log.lower() and "reveal" in log.lower()


def wait_ready(root, state, inspector=None):
    deadline = time.monotonic() + 60
    while time.monotonic() < deadline:
        ready = []
        for name in ("a", "b"):
            if not owns_process(root, state, name):
                raise RuntimeError(f"Disposable desktop {name} exited before startup")
            if inspector:
                ready.append(subprocess.run([str(inspector), str(state["pids"][name])],
                                             capture_output=True).returncode == 0)
            else:
                ready.append(initialized(root, state, name))
        if all(ready):
            return
        time.sleep(.5)
    raise RuntimeError("Both disposable desktop windows did not initialize within 60 seconds")


def mac_inspector(root):
    source = root / "window.swift"
    source.write_text('''import CoreGraphics
import Foundation
let pid = Int32(CommandLine.arguments[1])!
let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]] ?? []
let visible = windows.contains { ($0[kCGWindowOwnerPID as String] as? Int32) == pid && ($0[kCGWindowLayer as String] as? Int) == 0 }
exit(visible ? 0 : 1)
''')
    binary = root / "window-inspector"
    subprocess.run(["/usr/bin/swiftc", str(source), "-o", str(binary)], check=True)
    return binary


def create(binary, capture_logs=True, directory=None):
    binary = binary.resolve(strict=True)
    if not os.access(binary, os.X_OK):
        raise RuntimeError("Desktop binary is not executable")
    # Short /tmp roots also avoid macOS Unix socket limits with Unicode paths.
    root = Path(tempfile.mkdtemp(prefix="mc-desktop-測試 ", dir=directory or "/tmp")).resolve()
    root.chmod(0o700)
    state = {"marker": MARKER, "root": str(root), "binary": str(binary), "pids": {},
             "captureLogs": capture_logs, "inspectable": not capture_logs}
    write_json(root / "probe.json", state)
    for name in ("a", "b"):
        for directory in (".codex", "desktop-data", "config", "data", "cache"):
            (root / name / directory).mkdir(parents=True, mode=0o700)
        (root / name / ".codex/config.toml").write_text('cli_auth_credentials_store = "file"\n')
        (root / name / "sentinel.txt").write_text(name)
    try:
        for name in ("a", "b"):
            launch(root, state, name)
    except Exception:
        for name in ("a", "b"):
            stop(root, state, name)
        raise
    return root, state


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    for command in ("start", "smoke"):
        child = commands.add_parser(command)
        child.add_argument("binary", type=Path)
        child.add_argument("--directory", type=Path)
        if command == "smoke":
            child.add_argument("--output", type=Path, required=True)
    for command in ("status", "checkpoint", "check", "restart", "stop"):
        child = commands.add_parser(command)
        child.add_argument("root", type=Path)
        if command in ("check", "restart"):
            child.add_argument("profile", choices=("a", "b"))
    args = parser.parse_args()
    if args.command in ("start", "smoke"):
        root, state = create(args.binary, capture_logs=args.command == "smoke", directory=args.directory)
        if args.command == "start":
            print(root)
            print("Two disposable windows started. Sign into different TEST accounts only.")
            return
        report = {"platform": sys.platform, "authenticatedIsolation": "not tested", "passed": False}
        try:
            inspector = mac_inspector(root) if sys.platform == "darwin" else None
            wait_ready(root, state, inspector)
            first = status(root, state)
            assert all(not v["authPresent"] for v in first.values()), "Smoke test unexpectedly authenticated"
            a_inodes, b_inodes = (set(first[n]["cookieDatabaseInodes"]) for n in ("a", "b"))
            assert a_inodes and b_inodes and a_inodes.isdisjoint(b_inodes), "Cookies are not separate"
            # Persist markers inside each desktop data directory across independent restarts.
            for name in ("a", "b"):
                (root / name / "desktop-data/probe-sentinel").write_text(name)
            restarts = []
            for name, peer in (("a", "b"), ("b", "a")):
                peer_pid = state["pids"][peer]
                stop(root, state, name)
                assert owns_process(root, state, peer), "Stopping one stopped its peer"
                launch(root, state, name)
                wait_ready(root, state, inspector)
                assert state["pids"][peer] == peer_pid and owns_process(root, state, peer)
                for target in ("a", "b"):
                    assert (root / target / "desktop-data/probe-sentinel").read_text() == target
                restarts.append({"restarted": name, "peerRunning": True, "markersPreserved": True})
            final = status(root, state)
            assert all(not v["authPresent"] and v["sentinelUnchanged"] for v in final.values())
            report.update(passed=True, separateCookieDatabases=True, runs=first, restarts=restarts,
                          windowCheck="Quartz visible windows" if inspector else "desktop reveal logs")
        except Exception as error:
            report["error"] = str(error)
            raise
        finally:
            for name in ("a", "b"):
                stop(root, state, name)
            args.output.mkdir(parents=True, exist_ok=True)
            write_json(args.output / "result.json", report)
        print(json.dumps(report, indent=2))
        return
    state = load(args.root)
    root = args.root.resolve(strict=True)
    if args.command == "status":
        print(json.dumps(status(root, state), indent=2))
    elif args.command == "checkpoint":
        write_json(root / "checkpoint.json", status(root, state))
        print("Private checkpoint saved; no credentials printed.")
    elif args.command == "check":
        before = json.loads((root / "checkpoint.json").read_text())[args.profile]
        after = status(root, state)[args.profile]
        assert after["running"], "Peer desktop is no longer running"
        assert before["authHash"] == after["authHash"], "Peer credential changed; inspect refresh timing"
        assert before["identityHash"] == after["identityHash"], "Peer identity changed"
        assert after["sentinelUnchanged"], "Peer storage sentinel changed"
        print(f"Profile {args.profile}: peer credential, identity and storage sentinel unchanged.")
    elif args.command == "restart":
        stop(root, state, args.profile)
        launch(root, state, args.profile)
        print(f"Restarted only disposable desktop {args.profile}.")
    elif args.command == "stop":
        for name in ("a", "b"):
            stop(root, state, name)
        print("Stopped only this probe's processes; temporary data retained privately.")


if __name__ == "__main__":
    os.umask(0o077)
    main()

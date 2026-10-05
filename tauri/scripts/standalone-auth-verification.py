#!/usr/bin/env python3
"""Real disposable desktop auth checks. Never copies production credentials to a runner."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import queue
import subprocess
import threading
import time

spec = importlib.util.spec_from_file_location("probe", Path(__file__).with_name("standalone-isolation.py"))
probe = importlib.util.module_from_spec(spec)
spec.loader.exec_module(probe)


def desktop(root, name, operation="account", refresh=False):
    completed = subprocess.run([
        "node", "--experimental-websocket", "--disable-warning=ExperimentalWarning",
        str(Path(__file__).with_name("standalone-desktop-rpc.mjs")), str(root), name,
        operation, str(refresh).lower(),
    ], capture_output=True, text=True, timeout=40)
    if completed.returncode:
        raise RuntimeError("Disposable desktop inspection is not ready")
    result = json.loads(completed.stdout)
    if operation in ("account", "limits", "logout") and not result.get("ok"):
        raise RuntimeError("Disposable desktop account request failed")
    return result


def wait_account(root, name, expected):
    deadline = time.monotonic() + 90
    while time.monotonic() < deadline:
        try:
            account = desktop(root, name)
            ui = desktop(root, name, "status")
            if account["identityHash"] == expected and (
                (expected is None and ui["signInVisible"]) or
                (expected is not None and ui["profileMenuPresent"] and not ui["signInVisible"])
            ):
                return account
        except (RuntimeError, subprocess.TimeoutExpired, FileNotFoundError):
            pass
        time.sleep(.5)
    raise RuntimeError(f"Desktop {name} did not show the expected authenticated/signed-out state")


class LoginServer:
    def __init__(self, cli, root, name):
        self.process = subprocess.Popen([str(cli), "app-server", "--stdio"],
            env=probe.environment(root, name), cwd=root / name,
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
            text=True, start_new_session=True)
        self.messages = queue.Queue()
        self.next_id = 0
        threading.Thread(target=self.read, daemon=True).start()
        self.request("initialize", {"clientInfo": {"name": "multi_codex_isolation", "version": "1"}})
        self.send({"method": "initialized"})

    def read(self):
        for line in self.process.stdout:
            try:
                self.messages.put(json.loads(line))
            except json.JSONDecodeError:
                continue

    def send(self, message):
        self.process.stdin.write(json.dumps(message) + "\n")
        self.process.stdin.flush()

    def request(self, method, params):
        self.next_id += 1
        self.send({"id": self.next_id, "method": method, "params": params})
        deadline = time.monotonic() + 40
        while time.monotonic() < deadline:
            message = self.messages.get(timeout=max(.1, deadline - time.monotonic()))
            if message.get("id") == self.next_id:
                if "error" in message:
                    raise RuntimeError("Disposable app-server login request failed")
                return message["result"]
        raise RuntimeError("Disposable login server timed out")

    def login(self, name, output=None, public_key=None):
        result = self.request("account/login/start", {"type": "chatgptDeviceCode"})
        assert result.get("type") == "chatgptDeviceCode", "Device-code login was not available"
        url, code = result["verificationUrl"], result["userCode"]
        assert url.startswith("https://auth.openai.com/"), "Unexpected device-code verification site"
        if public_key:
            encrypted = subprocess.run(["openssl", "pkeyutl", "-encrypt", "-pubin", "-inkey", str(public_key),
                "-pkeyopt", "rsa_padding_mode:oaep"], input=json.dumps({"profile": name, "url": url, "code": code}).encode(),
                capture_output=True, check=True).stdout
            (output / f"signin-{name}.encrypted").write_bytes(encrypted)
            print(f"Encrypted device sign-in request {name.upper()} ready for the local operator.", flush=True)
        else:
            print(f"TEST ACCOUNT {name.upper()}: open {url} and enter {code}", flush=True)
            print("Use the matching disposable account. This is a fresh runner login; no local tokens are uploaded.", flush=True)
        deadline = time.monotonic() + 15 * 60
        while time.monotonic() < deadline:
            try:
                message = self.messages.get(timeout=min(30, max(.1, deadline - time.monotonic())))
            except queue.Empty:
                print(f"Waiting for test account {name.upper()} device sign-in...", flush=True)
                continue
            if message.get("method") == "account/login/completed":
                assert message["params"].get("success"), "Disposable device-code sign-in failed"
                print(f"Test account {name.upper()} sign-in completed.", flush=True)
                return
        raise RuntimeError("Disposable device-code sign-in was not completed")

    def close(self):
        self.process.terminate()
        try:
            self.process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            self.process.kill()
            self.process.wait(timeout=5)


def checks(root, state, expected, report):
    identity = {}
    for name in ("a", "b"):
        value = desktop(root, name)
        assert value["accountType"] == "chatgpt" and value["identityHash"], "Two real ChatGPT logins are required"
        identity[name] = value["identityHash"]
        assert expected[name] is None or expected[name] == identity[name], "Test account does not match expected identity"
        wait_account(root, name, identity[name])
    assert identity["a"] != identity["b"], "Test accounts must have different identities"
    report["differentIdentities"] = True
    report["checks"] = []
    # A fresh desktop cache must accept the pre-existing Codex-home credentials.
    for name, peer in (("a", "b"), ("b", "a")):
        peer_hash = probe.fingerprint(root / peer / ".codex/auth.json")
        peer_pid = state["pids"][peer]
        probe.stop(root, state, name)
        data = root / name / "desktop-data"
        data.rename(root / name / f"desktop-data-before-authcheck-{time.time_ns()}")
        data.mkdir(mode=0o700)
        probe.launch(root, state, name)
        wait_account(root, name, identity[name])
        assert state["pids"][peer] == peer_pid and probe.owns_process(root, state, peer)
        assert peer_hash == probe.fingerprint(root / peer / ".codex/auth.json"), "Restart changed peer credentials"
        assert desktop(root, peer)["identityHash"] == identity[peer]
        assert desktop(root, peer, "limits")["ok"]
        report["checks"].append({"profile": name, "coldDesktopReuse": True, "peerUnaffected": True})
    for name, peer in (("a", "b"), ("b", "a")):
        peer_hash = probe.fingerprint(root / peer / ".codex/auth.json")
        own_hash = probe.fingerprint(root / name / ".codex/auth.json")
        assert desktop(root, name, refresh=True)["identityHash"] == identity[name]
        assert probe.fingerprint(root / name / ".codex/auth.json") != own_hash, "Forced refresh did not rotate the test credential"
        assert probe.fingerprint(root / peer / ".codex/auth.json") == peer_hash, "Refresh changed peer credentials"
        assert desktop(root, peer)["identityHash"] == identity[peer]
        assert desktop(root, peer, "limits")["ok"]
        report["checks"].append({"profile": name, "forcedRefreshRotatedCredential": True, "peerUnaffected": True})
    # Keep inert private copies of the latest disposable credentials for later local
    # launch-button validation. They are never committed or uploaded as artifacts.
    for name in ("a", "b"):
        saved = root / name / "test-credential-retained.json"
        saved.write_bytes((root / name / ".codex/auth.json").read_bytes())
        saved.chmod(0o600)
    for name, peer in (("a", "b"), ("b", "a")):
        peer_hash = probe.fingerprint(root / peer / ".codex/auth.json")
        assert desktop(root, name, "logout")["ok"]
        wait_account(root, name, None)
        assert not (root / name / ".codex/auth.json").exists(), "Desktop logout retained an active credential"
        assert probe.fingerprint(root / peer / ".codex/auth.json") == peer_hash, "Logout changed peer credentials"
        assert desktop(root, peer)["identityHash"] == (identity[peer] if peer == "b" else None)
        if peer == "b":
            assert desktop(root, peer, "limits")["ok"]
        probe.stop(root, state, name)
        probe.launch(root, state, name)
        wait_account(root, name, None)
        assert probe.fingerprint(root / peer / ".codex/auth.json") == peer_hash
        report["checks"].append({"profile": name, "desktopLogout": True, "stayedSignedOutAfterRestart": True, "peerUnaffected": True})
    report.update(passed=True, authenticatedIsolation="passed using real native desktop requests and UI states")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--root", type=Path)
    source.add_argument("--binary", type=Path)
    parser.add_argument("--device-login", type=Path)
    parser.add_argument("--device-request-key", type=Path)
    parser.add_argument("--expected-a")
    parser.add_argument("--expected-b")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.root:
        state = probe.load(args.root)
        root = args.root.resolve()
    else:
        root, state = probe.create(args.binary, capture_logs=False)
    report = {"platform": os.uname().sysname, "desktopVersion": probe.desktop_version(Path(state["binary"])),
              "passed": False, "authenticatedIsolation": "not passed"}
    args.output.mkdir(parents=True, exist_ok=True)
    try:
        if args.device_login:
            for name in ("a", "b"):
                server = LoginServer(args.device_login, root, name)
                try:
                    server.login(name, args.output, args.device_request_key)
                finally:
                    server.close()
                probe.stop(root, state, name)
                probe.launch(root, state, name)
                # Login is fresh for this runner. Desktop must pick up that exact home.
                deadline = time.monotonic() + 90
                while time.monotonic() < deadline:
                    try:
                        value = desktop(root, name)
                        if value["identityHash"]:
                            break
                    except (RuntimeError, subprocess.TimeoutExpired):
                        pass
                    time.sleep(.5)
        checks(root, state, {"a": args.expected_a, "b": args.expected_b}, report)
    except Exception as error:
        report["failure"] = str(error)
        raise
    finally:
        probe.write_json(args.output / "result.json", report)
        if args.binary:
            for name in ("a", "b"):
                probe.stop(root, state, name)
    print(json.dumps(report, indent=2), flush=True)


if __name__ == "__main__":
    os.umask(0o077)
    main()

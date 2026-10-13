#!/usr/bin/env python3
"""Exercise the pinned updater SDK on an exact candidate in a disposable root.

Only transport and the runtime's baseline version are test fixtures. Actual
signature validation, platform installation and packaged startup are native.
This does not claim the production Update button or a published version upgrade.
"""
import argparse
import hashlib
import http.server
import json
import os
from pathlib import Path
import platform
import shutil
import signal
import subprocess
import tempfile
import threading
import time
from nsis_candidate_digest import expected_nsis_digest


def digest(path):
    checksum = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            checksum.update(chunk)
    return checksum.hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--harness", required=True, type=Path)
    parser.add_argument("--payload", required=True, type=Path)
    parser.add_argument("--app", type=Path, help="macOS source .app bundle")
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    repository = Path(__file__).resolve().parents[2]
    config = json.loads((repository / "tauri/src-tauri/tauri.conf.json").read_text())
    signature = Path(str(args.payload) + ".sig").read_text().strip()
    system = platform.system()
    report = {"source": os.environ.get("GITHUB_SHA"), "platform": system,
              "candidateSource": os.environ.get("NATIVE_UPDATER_CANDIDATE_SOURCE", os.environ.get("GITHUB_SHA")),
              "architecture": platform.machine(), "candidateVersion": config["version"],
              "payloadSha256": digest(args.payload), "passed": False, "checks": {},
              "baseline": "synthetic 1.0.0 runtime; exact candidate binary with an inert old-install marker",
              "runtime": "mock GUI only; real tauri-plugin-updater 2.12.0",
              "productionUpdateButton": "not exercised by this SDK harness"}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    stage = "prepare-private-install"
    server = None
    app = None
    temporary = None
    install = None
    try:
        temporary = tempfile.TemporaryDirectory(prefix="Multi Codex SDK 測試 ")
        root = Path(temporary.name).resolve()
        env = {key: value for key, value in os.environ.items()
               if not key.startswith(("CODEX_", "OPENAI_", "VSCODE_"))
                   and key not in ("NODE_OPTIONS", "ELECTRON_RUN_AS_NODE", "APPIMAGE", "APPDIR",
                                   "TAURI_SIGNING_PRIVATE_KEY", "TAURI_SIGNING_PRIVATE_KEY_PASSWORD", "GH_TOKEN", "GITHUB_TOKEN")}
        env.update(MULTI_CODEX_UPDATER_TEST_ALLOWED="1", HOME=str(root / "home"),
                   USERPROFILE=str(root / "home"), APPDATA=str(root / "roaming"),
                   LOCALAPPDATA=str(root / "local"), XDG_DATA_HOME=str(root / "data"),
                   XDG_CONFIG_HOME=str(root / "config"), CODEX_HOME=str(root / "codex-home"))
        for key in ("HOME", "APPDATA", "LOCALAPPDATA", "XDG_DATA_HOME", "XDG_CONFIG_HOME", "CODEX_HOME"):
            Path(env[key]).mkdir(parents=True, exist_ok=True)
        if system == "Windows":
            data = Path(env["APPDATA"]) / "multi-codex"
            install = root / "installed app with spaces"
            # NSIS requires /D last and unquoted even when the directory has
            # spaces. An argv list would quote the whole argument on Windows.
            result = subprocess.run(f'"{args.payload.resolve()}" /S /D={install}',
                                    executable=str(args.payload.resolve()),
                                    env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=180)
            assert result.returncode == 0
            binary = install / "multi-codex-desktop.exe"
            source_binary = repository / "tauri/src-tauri/target/release/multi-codex-desktop.exe"
        elif system == "Darwin":
            assert args.app and args.app.suffix == ".app"
            data = Path(env["HOME"]) / "Library/Application Support/multi-codex"
            install = root / "Applications/Multi Codex.app"
            install.parent.mkdir()
            subprocess.run(["ditto", str(args.app.resolve()), str(install)], check=True)
            source_binary = args.app / "Contents/MacOS/multi-codex-desktop"
            binary = install / "Contents/MacOS/multi-codex-desktop"
        elif system == "Linux":
            data = Path(env["XDG_DATA_HOME"]) / "multi-codex"
            install = root / "application"
            install.mkdir()
            source_binary = args.payload.resolve()
            binary = install / "Multi Codex.AppImage"
            shutil.copy2(source_binary, binary)
        else:
            raise RuntimeError("Unsupported native platform")
        expected_binary = expected_nsis_digest(source_binary) if system == "Windows" else digest(source_binary)
        assert digest(binary) == expected_binary
        def verify_signature(label):
            result = subprocess.run(["codesign", "--verify", "--deep", "--strict", str(install)],
                                    stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            report["checks"][label] = result.returncode == 0
            if result.returncode:
                message = result.stderr.decode(errors="replace").lower()
                report["signatureFailure"] = {"exitCode": result.returncode,
                    "missingOrInvalidSealedResource": "a sealed resource is missing or invalid" in message,
                    "unsealedBundleRoot": "unsealed contents present" in message,
                    "unsignedCodeObject": "not signed at all" in message,
                    "invalidMainSignature": "invalid signature" in message,
                    "resourceForkOrFinderInfo": "resource fork" in message or "finder information" in message,
                    "invalidBundleFormat": "bundle format" in message}
            assert result.returncode == 0, "Native bundle signature integrity failed"
        if system == "Darwin":
            stage = "initial-bundle-signature"
            verify_signature("initialBundleSignatureIntegrity")
            bridge = install / "Contents/Resources/managed-vscode/bridge"
            stamp = json.loads((bridge / "desktop-helper.json").read_text())
            assert digest(bridge / "bin/multi-codex-account-helper") == stamp["digest"]
            report["checks"]["sealedBundleHelperFingerprint"] = True
            original_files = {p.relative_to(install).as_posix(): digest(p)
                              for p in install.rglob("*") if p.is_file()}
        # The SDK must replace these changed bytes with the signed candidate.
        with binary.open("ab") as previous:
            previous.write(b"\nINERT PRE-UPDATE INSTALL MARKER\n")
        before = digest(binary)
        data.mkdir(parents=True, exist_ok=True)
        profile = data / "profiles/00000000-0000-4000-8000-000000000001/codex-home"
        (profile / "sessions").mkdir(parents=True)
        fixtures = {data / "profiles.json": b"[]\n",
                    data / "executables.json": b'{"onboardingCompleted":false}\n',
                    profile / "auth.json": b'{"fixture":"not-a-credential"}\n',
                    profile / "sessions/keep.jsonl": b'{"fixture":"retained-chat"}\n',
                    Path(env["CODEX_HOME"]) / "config.toml": b"# retained default settings\n",
                    root / "retained-backup/sentinel": b"retained backup\n"}
        for path, content in fixtures.items():
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(content)
        def preserved():
            assert all(path.read_bytes() == content for path, content in fixtures.items())

        class Transport(http.server.BaseHTTPRequestHandler):
            def log_message(self, *_):
                pass

            def do_GET(self):
                scenario = self.server.scenario
                if self.path == "/manifest.json":
                    version = {"current": "1.0.0", "older": "0.9.0"}.get(scenario, config["version"])
                    manifest = {"version": version, "url": f"http://127.0.0.1:{self.server.server_port}/payload",
                                "signature": signature}
                    if scenario == "malformed":
                        content = b"{invalid-json"
                    elif scenario == "wrong-target":
                        manifest.pop("url")
                        manifest.pop("signature")
                        manifest["platforms"] = {"unsupported-target": {"url": "http://127.0.0.1/payload", "signature": signature}}
                        content = json.dumps(manifest).encode()
                    else:
                        content = json.dumps(manifest).encode()
                    self.send_response(200)
                    self.send_header("Content-Type", "application/json")
                    self.send_header("Content-Length", str(len(content)))
                    self.end_headers()
                    self.wfile.write(content)
                    return
                self.send_response(200)
                self.send_header("Content-Length", str(args.payload.stat().st_size))
                self.end_headers()
                try:
                    with args.payload.open("rb") as source:
                        while chunk := source.read(1024 * 1024):
                            if scenario == "corrupt":
                                chunk = bytes([chunk[0] ^ 1]) + chunk[1:]
                                scenario = "changed-once"
                            self.wfile.write(chunk)
                            if scenario == "truncated":
                                self.close_connection = True
                                return
                except (BrokenPipeError, ConnectionResetError):
                    pass

        server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Transport)
        server.scenario = "current"
        threading.Thread(target=server.serve_forever, daemon=True).start()
        endpoint = f"http://127.0.0.1:{server.server_port}/manifest.json"
        proof = root / "verified-before-installer.txt"
        for scenario, mode in [("current", "current"), ("older", "current"),
                               ("malformed", "unavailable"), ("wrong-target", "unavailable"),
                               ("corrupt", "reject"), ("truncated", "reject"), ("valid", "install")]:
            stage = f"sdk-{scenario}"
            server.scenario = scenario
            result = subprocess.run([str(args.harness.resolve()), endpoint, str(root), str(binary),
                                     config["plugins"]["updater"]["pubkey"], mode, str(proof)], env=env,
                                    stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=300)
            assert result.returncode == 0, "Native SDK harness failed"
            if scenario != "valid":
                assert digest(binary) == before
                assert not proof.exists(), "Invalid update reached the installer"
            else:
                if system == "Windows":
                    assert proof.read_bytes() == b"verified-download-before-native-installer\n"
                deadline = time.monotonic() + 180
                while time.monotonic() < deadline:
                    try:
                        if digest(binary) == expected_binary:
                            break
                    except OSError:
                        pass
                    time.sleep(0.2)
                assert digest(binary) == expected_binary
            preserved()
            report["checks"][scenario] = True
        stage = "native-packaged-restart"
        if system == "Darwin":
            updated_files = {p.relative_to(install).as_posix(): digest(p)
                             for p in install.rglob("*") if p.is_file()}
            report["bundleInventory"] = {
                "missingFiles": len(original_files.keys() - updated_files.keys()),
                "addedFiles": len(updated_files.keys() - original_files.keys()),
                "changedFiles": sum(updated_files.get(name) != value for name, value in original_files.items()),
                "codeResourcesPresent": (install / "Contents/_CodeSignature/CodeResources").is_file()}
            assert updated_files == original_files, "SDK did not restore the entire exact candidate bundle"
            stage = "updated-bundle-signature"
            verify_signature("updatedBundleSignatureIntegrity")
            stage = "native-packaged-restart"
        if system == "Linux":
            env["APPIMAGE_EXTRACT_AND_RUN"] = "1"
        app = subprocess.Popen([str(binary)], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                               start_new_session=system != "Windows")
        time.sleep(5)
        assert app.poll() is None, "Updated candidate failed native startup"
        preserved()
        report["checks"].update(exactSignedCandidateInstalled=True, nativePackagedRestartAlive=True,
                                 privateDataAndRetainedBackupPreserved=True)
        report["passed"] = True
        if system == "Windows":
            subprocess.run(["taskkill.exe", "/PID", str(app.pid), "/T", "/F"],
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            app.wait(timeout=15)
            app = None
        else:
            os.killpg(app.pid, signal.SIGTERM)
            app.wait(timeout=15)
            app = None
    except Exception as error:
        report["failedStage"] = stage
        report["errorClass"] = type(error).__name__
        raise
    finally:
        if app and app.poll() is None:
            if system == "Windows":
                subprocess.run(["taskkill.exe", "/PID", str(app.pid), "/T", "/F"],
                               stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            else:
                os.killpg(app.pid, signal.SIGTERM)
            app.wait(timeout=15)
        if server:
            server.shutdown()
            server.server_close()
        if system == "Windows" and install and (install / "uninstall.exe").is_file():
            subprocess.run([str(install / "uninstall.exe"), "/S"], env=env,
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=120)
            deadline = time.monotonic() + 30
            while (install / "uninstall.exe").exists() and time.monotonic() < deadline:
                time.sleep(0.2)
        if temporary:
            temporary.cleanup()
        args.output.write_text(json.dumps(report, indent=2) + "\n")
        print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()

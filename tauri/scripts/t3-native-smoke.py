"""Download and verify official T3, then boot two credential-free desktop fixtures."""
import hashlib, json, os, pathlib, platform, signal, subprocess, tempfile, urllib.request
root = pathlib.Path(__file__).resolve().parents[2]
version = '0.1.7'
windows = platform.system() == 'Windows'
asset = f'T3-Code-{version}-x64.exe' if windows else f'T3-Code-{version}-amd64.deb'
base = f'https://github.com/pingdotgg/t3code/releases/download/v{version}/'
with tempfile.TemporaryDirectory(prefix='multi-codex-t3-') as directory:
    work = pathlib.Path(directory)
    archive = work / asset
    urllib.request.urlretrieve(base + asset, archive)
    request = urllib.request.Request(f'https://api.github.com/repos/pingdotgg/t3code/releases/tags/v{version}', headers={'Accept':'application/vnd.github+json','User-Agent':'multi-codex-native-validation'})
    release = json.load(urllib.request.urlopen(request, timeout=60))
    expected = next(item['digest'].removeprefix('sha256:') for item in release['assets'] if item['name'] == asset)
    assert len(expected) == 64, 'Official desktop asset digest is required'
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    assert digest == expected, 'Upstream T3 checksum mismatch'
    extracted = work / 'app'
    if windows:
        # The unsuffixed x64 ZIP is a macOS bundle. Extract the Windows NSIS payload.
        installer = work / 'installer'
        subprocess.run(['7z', 'x', str(archive), f'-o{installer}', '-y'], check=True, stdout=subprocess.DEVNULL)
        payload = next(installer.rglob('app-64.7z'))
        subprocess.run(['7z', 'x', str(payload), f'-o{extracted}', '-y'], check=True, stdout=subprocess.DEVNULL)
        binary = next(p for p in extracted.rglob('*.exe') if 't3' in p.name.lower() and (p.parent/'resources/app.asar').is_file())
    else:
        subprocess.run(['dpkg-deb', '-x', str(archive), str(extracted)], check=True)
        binary = next(p for p in extracted.rglob('t3code') if p.is_file() and os.access(p, os.X_OK) and p.stat().st_size > 1000000)
    command = ['cargo', 'run', '--locked', '--manifest-path', str(root/'cli/Cargo.toml'), '-p', 'multi-codex-core', '--example', 't3-launch-probe', '--', str(binary), str(work/'profiles')]
    # The third-party desktop fixture must never inherit CI signing or access tokens.
    fixture_env = {key:value for key,value in os.environ.items() if key not in {'TAURI_SIGNING_PRIVATE_KEY','TAURI_SIGNING_PRIVATE_KEY_PASSWORD','GH_TOKEN','GITHUB_TOKEN','ACTIONS_RUNTIME_TOKEN','ACTIONS_ID_TOKEN_REQUEST_TOKEN'}}
    probe = subprocess.Popen(command, cwd=root, env=fixture_env, start_new_session=not windows)
    try:
        assert probe.wait(timeout=240) == 0, 'T3 native account-isolation smoke failed'
    finally:
        if not windows:
            try: os.killpg(probe.pid, signal.SIGTERM)
            except ProcessLookupError: pass
        elif probe.poll() is None:
            subprocess.run(['taskkill', '/F', '/T', '/PID', str(probe.pid)], capture_output=True)
    print(json.dumps({'upstreamVersion':version, 'asset':asset, 'sha256':digest, 'accounts':2, 'status':'passed', 'scope':'Disposable desktop startup and singleton isolation, no authenticated agent turn'}))

"""Download and verify official T3, then boot two credential-free desktop fixtures."""
import hashlib, json, os, pathlib, platform, signal, subprocess, tempfile, urllib.request, zipfile
root = pathlib.Path(__file__).resolve().parents[2]
version = '0.1.7'
windows = platform.system() == 'Windows'
asset = f'T3-Code-{version}-x64.zip' if windows else f'T3-Code-{version}-amd64.deb'
base = f'https://github.com/pingdotgg/t3code/releases/download/v{version}/'
with tempfile.TemporaryDirectory(prefix='multi-codex-t3-') as directory:
    work = pathlib.Path(directory)
    archive = work / asset
    urllib.request.urlretrieve(base + asset, archive)
    checksums = urllib.request.urlopen(base + 'SHA256SUMS', timeout=60).read().decode()
    expected = next(line.split()[0] for line in checksums.splitlines() if line.split()[-1].lstrip('*./') == asset)
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    assert digest == expected, 'Upstream T3 checksum mismatch'
    extracted = work / 'app'
    if windows:
        with zipfile.ZipFile(archive) as source: source.extractall(extracted)
        binary = next(p for p in extracted.rglob('*.exe') if 't3' in p.name.lower() and p.stat().st_size > 50000000)
    else:
        subprocess.run(['dpkg-deb', '-x', str(archive), str(extracted)], check=True)
        binary = next(p for p in extracted.rglob('t3code') if p.is_file() and os.access(p, os.X_OK) and p.stat().st_size > 1000000)
    command = ['cargo', 'run', '--locked', '--manifest-path', str(root/'cli/Cargo.toml'), '-p', 'multi-codex-core', '--example', 't3-launch-probe', '--', str(binary), str(work/'profiles')]
    probe = subprocess.Popen(command, cwd=root, start_new_session=not windows)
    try:
        assert probe.wait(timeout=240) == 0, 'T3 native account-isolation smoke failed'
    finally:
        if not windows:
            try: os.killpg(probe.pid, signal.SIGTERM)
            except ProcessLookupError: pass
        elif probe.poll() is None:
            subprocess.run(['taskkill', '/F', '/T', '/PID', str(probe.pid)], capture_output=True)
    print(json.dumps({'upstreamVersion':version, 'asset':asset, 'sha256':digest, 'accounts':2, 'status':'passed', 'scope':'Disposable desktop startup and singleton isolation, no authenticated agent turn'}))

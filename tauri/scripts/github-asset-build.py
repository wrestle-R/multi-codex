"""Run a build using Tauri's mirror hook and GitHub's official public asset API."""
import hashlib
import http.server
import json
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile
import threading
import time
import urllib.parse
import urllib.request

ALLOWED_OWNERS = {'tauri-apps', 'AppImage', 'linuxdeploy', 'appimage-community'}


def download_asset(path, destination):
    parts = urllib.parse.urlsplit(path).path.strip('/').split('/')
    if len(parts) != 6 or parts[0] not in ALLOWED_OWNERS or parts[2:4] != ['releases', 'download']:
        raise ValueError('Unsupported bundler asset path')
    owner, repo, _, _, tag, name = parts
    metadata_url = f'https://api.github.com/repos/{owner}/{repo}/releases/tags/{tag}'
    headers = {'User-Agent': 'multi-codex-build', 'Accept': 'application/vnd.github+json'}
    with urllib.request.urlopen(urllib.request.Request(metadata_url, headers=headers), timeout=60) as response:
        release = json.load(response)
    asset = next(item for item in release['assets'] if item['name'] == urllib.parse.unquote(name))
    # Only GitHub's returned asset endpoint is allowed; no arbitrary metadata URL.
    expected_prefix = f'https://api.github.com/repos/{owner}/{repo}/releases/assets/'
    if not asset['url'].startswith(expected_prefix):
        raise ValueError('Unexpected asset endpoint')
    headers['Accept'] = 'application/octet-stream'
    with urllib.request.urlopen(urllib.request.Request(asset['url'], headers=headers), timeout=120) as response, destination.open('wb') as output:
        shutil.copyfileobj(response, output)
    if destination.stat().st_size != asset['size']:
        raise ValueError('Incomplete bundler asset')
    if asset.get('digest'):
        actual = 'sha256:' + hashlib.sha256(destination.read_bytes()).hexdigest()
        if actual != asset['digest']:
            raise ValueError('Bundler asset digest mismatch')
    # Tauri additionally retains its own pinned toolchain hash checks.


class Handler(http.server.BaseHTTPRequestHandler):
    def log_message(self, *_args):
        pass

    def do_GET(self):
        with tempfile.TemporaryDirectory(prefix='multi-codex-bundler-') as directory:
            destination = pathlib.Path(directory) / 'asset'
            for attempt in range(3):
                try:
                    download_asset(self.path, destination)
                    break
                except Exception as error:
                    if attempt == 2:
                        print(f'Official bundler asset download failed: {type(error).__name__}', flush=True)
                        self.send_error(502, 'Official asset unavailable')
                        return
                    time.sleep(attempt + 1)
            self.send_response(200)
            self.send_header('Content-Length', str(destination.stat().st_size))
            self.end_headers()
            with destination.open('rb') as source:
                shutil.copyfileobj(source, self.wfile)


def main():
    command = sys.argv[1:]
    if command[:1] == ['--']:
        command = command[1:]
    if not command:
        raise SystemExit('Expected a build command')
    command[0] = shutil.which(command[0]) or command[0]
    server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Handler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    environment = dict(os.environ)
    environment['TAURI_BUNDLER_TOOLS_GITHUB_MIRROR_TEMPLATE'] = (
        f'http://127.0.0.1:{server.server_port}/<owner>/<repo>/releases/download/<version>/<asset>'
    )
    try:
        return subprocess.call(command, env=environment)
    finally:
        server.shutdown()
        server.server_close()
        thread.join()


if __name__ == '__main__':
    sys.exit(main())

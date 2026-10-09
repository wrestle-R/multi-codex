#!/usr/bin/env python3
"""Verify native menu, close, reopen and Quit using a disposable packaged app."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time

app = Path(sys.argv[1]).resolve()
binary_name = subprocess.check_output([
    '/usr/libexec/PlistBuddy', '-c', 'Print :CFBundleExecutable',
    str(app / 'Contents/Info.plist'),
], text=True).strip()
report_dir = Path(os.environ['RUNNER_TEMP']) / 'multi-codex-menu-evidence'
report_dir.mkdir(exist_ok=True)

def script(source):
    result = subprocess.run(['osascript', '-e', source], capture_output=True, text=True, timeout=20)
    if result.returncode:
        raise RuntimeError(result.stderr.strip())
    return result.stdout.strip()

def wait_for(label, condition):
    deadline = time.monotonic() + 30
    last_error = None
    while time.monotonic() < deadline:
        try:
            if condition():
                return
        except RuntimeError as error:
            last_error = str(error)
        time.sleep(.5)
    raise RuntimeError(f'Timed out: {label}; last UI error: {last_error}')

with tempfile.TemporaryDirectory(prefix='Multi Codex menu 測試 ') as temporary:
    home = Path(temporary)
    data = home / 'Library/Application Support/multi-codex'
    data.mkdir(parents=True)
    profile_id = '00000000-0000-4000-8000-000000000001'
    profiles = [{'id': profile_id, 'name': 'Menu test account', 'authMode': 'apikey',
                 'createdAt': '2026-10-09T00:00:00Z', 'updatedAt': '2026-10-09T00:00:00Z'}]
    metadata = json.dumps(profiles).encode()
    (data / 'profiles.json').write_bytes(metadata)
    (data / 'profiles.json').chmod(0o600)
    env = {key: value for key, value in os.environ.items()
           if not key.startswith(('CODEX_', 'VSCODE_', 'OPENAI_'))
           and key not in ('ELECTRON_RUN_AS_NODE', 'NODE_OPTIONS')}
    env.update(HOME=str(home), XDG_DATA_HOME=str(home / 'data'), PATH='/usr/bin:/bin:/usr/sbin:/sbin')
    with (report_dir / 'application.log').open('wb') as log:
        process = subprocess.Popen([str(app / 'Contents/MacOS' / binary_name)],
                                   env=env, stdout=log, stderr=subprocess.STDOUT)
        prefix = f'tell application "System Events" to tell first application process whose unix id is {process.pid}\n'
        def ui(command):
            return script(prefix + command + '\nend tell')
        def window_count():
            return int(ui('return count of windows'))
        def open_menu():
            ui('click menu bar item 1 of menu bar 2')
        def account_actions():
            open_menu()
            ui('click menu item "Menu test account" of menu 1 of menu bar item 1 of menu bar 2')
            actions = ui('return name of every menu item of menu 1 of menu item "Menu test account" of menu 1 of menu bar item 1 of menu bar 2')
            ui('key code 53')
            ui('key code 53')
            return actions
        def replace_metadata(contents):
            temporary_metadata = data / 'profiles.menu-test.tmp'
            temporary_metadata.write_bytes(contents)
            temporary_metadata.chmod(0o600)
            temporary_metadata.replace(data / 'profiles.json')
        checks = []
        moved_codex = None
        try:
            wait_for('native main window', lambda: window_count() > 0)
            open_menu()
            names = ui('return name of every menu item of menu 1 of menu bar item 1 of menu bar 2')
            for expected in ('Open Multi Codex', 'Menu test account', 'Refresh All Usage', 'Quit Multi Codex'):
                assert expected in names, f'Missing menu action: {expected}: {names}'
            ui('click menu item "Menu test account" of menu 1 of menu bar item 1 of menu bar 2')
            actions = ui('return name of every menu item of menu 1 of menu item "Menu test account" of menu 1 of menu bar item 1 of menu bar 2')
            for expected in ('Open VS Code', 'Open Codex', 'Open CLI'):
                assert expected in actions, f'Missing installed target: {expected}: {actions}'
            ui('key code 53')
            checks.append('Native account menu contains all three installed supported launch targets')
            ui('key code 53')
            profiles[0]['name'] = 'Renamed menu account'
            replace_metadata(json.dumps(profiles).encode())
            def renamed_visible():
                open_menu()
                names = ui('return name of every menu item of menu 1 of menu bar item 1 of menu bar 2')
                ui('key code 53')
                return 'Renamed menu account' in names
            wait_for('menu reflecting account edits', renamed_visible)
            replace_metadata(metadata)
            def original_visible():
                open_menu()
                names = ui('return name of every menu item of menu 1 of menu bar item 1 of menu bar 2')
                ui('key code 53')
                return 'Menu test account' in names
            wait_for('restored account label', original_visible)
            checks.append('Native menu updates account names without restarting')
            codex_app = next((path for path in (Path('/Applications/Codex.app'), Path('/Applications/ChatGPT.app')) if path.exists()), None)
            assert codex_app, 'The disposable runner must have the pinned Codex fixture'
            moved_codex = home / 'Codex-away.app'
            codex_app.rename(moved_codex)
            wait_for('menu hiding an uninstalled Codex target', lambda: 'Open Codex' not in account_actions())
            assert 'Open VS Code' in account_actions(), 'Installed VS Code disappeared'
            moved_codex.rename(codex_app)
            moved_codex = None
            wait_for('menu restoring the supported Codex target', lambda: 'Open Codex' in account_actions())
            checks.append('Native menu removes and restores launch targets when installed apps change')
            ui('click first button of window 1 whose subrole is "AXCloseButton"')
            wait_for('close hiding the main window', lambda: window_count() == 0)
            assert process.poll() is None, 'Close exited the app'
            open_menu()
            ui('click menu item "Open Multi Codex" of menu 1 of menu bar item 1 of menu bar 2')
            wait_for('tray reopening the same main window', lambda: window_count() > 0)
            checks.append('Close hides the window and tray Open restores it in the same process')
            ui('click first button of window 1 whose subrole is "AXCloseButton"')
            wait_for('second close', lambda: window_count() == 0)
            subprocess.run(['open', '-a', str(app)], check=True, timeout=20)
            wait_for('Dock/LaunchServices reopen', lambda: window_count() > 0)
            assert process.poll() is None
            checks.append('Dock/LaunchServices reopen restores the hidden main window')
            open_menu()
            ui('click menu item "Quit Multi Codex" of menu 1 of menu bar item 1 of menu bar 2')
            assert process.wait(timeout=15) == 0, 'Quit did not exit cleanly'
            checks.append('Quit exits the native application cleanly')
            assert (data / 'profiles.json').read_bytes() == metadata
            result = {'passed': True, 'checks': checks, 'accountMetadataUnchanged': True,
                      'metadataSha256': hashlib.sha256(metadata).hexdigest(),
                      'authenticatedLaunches': 'Not tested by this menu probe'}
            (report_dir / 'result.json').write_text(json.dumps(result, indent=2) + '\n')
            print(json.dumps(result, indent=2))
        finally:
            if moved_codex and moved_codex.exists():
                moved_codex.rename(codex_app)
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=5)

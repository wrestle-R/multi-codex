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

from macos_menu_ui import MascotMenu, mascot_pid, stop_mascot

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
    # This probe covers menus, not initial storage migration. Use the product's
    # saved pause so an unrelated extension installer cannot outlive the fixture.
    (data / 'migrations').mkdir()
    queue = data / 'migrations/sharing-queue.json'
    queue.write_text(json.dumps({'schema': 1, 'enabled': False, 'profiles': {}}))
    queue.chmod(0o600)
    env = {key: value for key, value in os.environ.items()
           if not key.startswith(('CODEX_', 'VSCODE_', 'OPENAI_'))
           and key not in ('ELECTRON_RUN_AS_NODE', 'NODE_OPTIONS')}
    env.update(HOME=str(home), XDG_DATA_HOME=str(home / 'data'), PATH='/usr/bin:/bin:/usr/sbin:/sbin')
    with (report_dir / 'application.log').open('wb') as log:
        process = subprocess.Popen([str(app / 'Contents/MacOS' / binary_name)],
                                   env=env, stdout=log, stderr=subprocess.STDOUT)
        def ui(pid, command):
            return script(f'tell application "System Events" to tell first application process whose unix id is {pid}\n'
                          + command + '\nend tell')

        def window_count():
            return int(ui(process.pid, 'return count of windows'))

        binary = app / 'Contents/MacOS' / binary_name
        menu = MascotMenu(binary, script)

        def menu_helper():
            return mascot_pid(binary)

        def expect_buttons(labels):
            for label in labels:
                wait_for(f'menu button {label}', lambda: menu.button(label))

        def replace_metadata(contents):
            temporary_metadata = data / 'profiles.menu-test.tmp'
            temporary_metadata.write_bytes(contents)
            temporary_metadata.chmod(0o600)
            temporary_metadata.replace(data / 'profiles.json')
        checks = []
        helper = None
        try:
            wait_for('native main window', lambda: window_count() > 0)
            wait_for('menu bar helper', menu_helper)
            helper = menu_helper()
            menu.open()
            expect_buttons(('Open Multi Codex', 'Open Codex as Menu test account',
                            'Refresh all usage', 'Quit Multi Codex'))
            assert menu.button('More ways to open Menu test account', press=True)
            expect_buttons(('Open VS Code', 'Open CLI'))
            menu.dismiss()
            checks.append('Mascot opens the account switcher and installed launch targets')

            profiles[0]['name'] = 'Renamed menu account'
            replace_metadata(json.dumps(profiles).encode())
            menu.open()
            expect_buttons(('Open Codex as Renamed menu account',))
            menu.dismiss()
            replace_metadata(metadata)
            checks.append('The dropdown reads saved account names when it opens')

            ui(process.pid, 'click first button of window 1 whose subrole is "AXCloseButton"')
            wait_for('close hiding the main window', lambda: window_count() == 0)
            assert process.poll() is None, 'Close exited the app'
            menu.open()
            expect_buttons(('Open Multi Codex',))
            assert menu.button('Open Multi Codex', press=True)
            wait_for('menu reopening the same main window', lambda: window_count() > 0)
            checks.append('Close hides the window and the mascot reopens it in the same process')

            ui(process.pid, 'click first button of window 1 whose subrole is "AXCloseButton"')
            wait_for('second close', lambda: window_count() == 0)
            subprocess.run(['open', '-a', str(app)], check=True, timeout=20)
            wait_for('Dock/LaunchServices reopen', lambda: window_count() > 0)
            assert process.poll() is None
            checks.append('Dock/LaunchServices reopen restores the hidden main window')

            ui(process.pid, 'keystroke "q" using command down')
            assert process.wait(timeout=15) == 0, 'Command-Q did not quit the launcher'
            assert menu_helper() == helper, 'Command-Q stopped the mascot'
            menu.open()
            expect_buttons(('Open Codex as Menu test account', 'Quit Multi Codex'))
            assert menu.button('Quit Multi Codex', press=True)
            wait_for('explicit quit stopping the menu bar helper', lambda: menu_helper() is None)
            checks.append('Command-Q leaves the mascot available and explicit Quit stops it')
            assert (data / 'profiles.json').read_bytes() == metadata
            result = {'passed': True, 'checks': checks, 'accountMetadataUnchanged': True,
                      'metadataSha256': hashlib.sha256(metadata).hexdigest(),
                      'authenticatedLaunches': 'Not tested by this menu probe'}
        finally:
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=5)
            stop_mascot(binary)
            wait_for('disposable menu helper cleanup', lambda: menu_helper() is None)

# Emit success only after the disposable home has been completely removed.
result['disposableFixtureCleanup'] = True
(report_dir / 'result.json').write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps(result, indent=2))

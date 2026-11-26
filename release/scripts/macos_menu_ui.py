"""Shared AppleScript helpers for the packaged mascot and its dropdown."""

import json
import os
import subprocess


def mascot_pid(binary):
    expected = f"{binary} --menu-bar-avatars"
    listing = subprocess.check_output(["/bin/ps", "-axo", "pid=,command="], text=True)
    for line in listing.splitlines():
        fields = line.split(maxsplit=1)
        if len(fields) == 2 and fields[1] == expected:
            return int(fields[0])
    return None


def stop_mascot(binary):
    if (pid := mascot_pid(binary)) is not None:
        try:
            os.kill(pid, 15)
        except ProcessLookupError:
            pass


class MascotMenu:
    def __init__(self, binary, applescript):
        self.binary = binary
        self.applescript = applescript

    def ui(self, command):
        if (pid := mascot_pid(self.binary)) is None:
            raise RuntimeError("The menu bar helper is not running")
        return self.applescript(
            f'tell application "System Events" to tell first application process whose unix id is {pid}\n'
            + command + '\nend tell'
        )

    def icon_count(self):
        return int(self.ui("""
            set iconCount to 0
            repeat with barRef in menu bars
                set iconCount to iconCount + (count of menu bar items of barRef)
            end repeat
            return iconCount
        """))

    def open(self):
        self.ui("""
            if exists window "Multi Codex Menu" then
                try
                    if visible of window "Multi Codex Menu" then return
                end try
            end if
            repeat with barRef in menu bars
                if (count of menu bar items of barRef) is 1 then
                    click menu bar item 1 of barRef
                    return
                end if
            end repeat
            error "Could not find the mascot"
        """)

    def button(self, label, press=False):
        command = 'perform action "AXPress" of itemRef' if press else 'return true'
        quoted = json.dumps(label, ensure_ascii=False)
        return self.ui(f"""
            repeat with itemRef in (entire contents of window "Multi Codex Menu")
                try
                    if role of itemRef is "AXButton" then
                        if name of itemRef is {quoted} or description of itemRef is {quoted} then
                            {command}
                            return true
                        end if
                    end if
                end try
            end repeat
            return false
        """) == "true"

    def dismiss(self):
        self.ui("key code 53")

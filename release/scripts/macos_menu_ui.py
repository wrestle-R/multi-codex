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
    def __init__(self, binary, applescript, log):
        self.binary = binary
        self.applescript = applescript
        self.log = log

    def ui(self, command):
        if (pid := mascot_pid(self.binary)) is None:
            raise RuntimeError("The menu bar helper is not running")
        return self.applescript(
            f'tell application "System Events" to tell first application process whose unix id is {pid}\n'
            + command + '\nend tell'
        )

    def location(self):
        pid = mascot_pid(self.binary)
        prefix = f"Multi Codex mascot location: {pid} "
        for line in reversed(self.log.read_text(errors="replace").splitlines()):
            if line.startswith(prefix):
                point = line.removeprefix(prefix)
                if point == "unavailable":
                    return None
                return tuple(map(float, point.split()))
        return None

    def icon_count(self):
        return int(self.location() is not None)

    def open(self):
        point = self.location()
        if point is None:
            raise RuntimeError("The helper did not report a visible mascot")
        # Post both mouse events. Tauri opens the popover on mouse-up; the
        # accessibility action on a status item does not guarantee that event.
        code = """
            import CoreGraphics
            let args = CommandLine.arguments
            let point = CGPoint(x: Double(args[1])!, y: Double(args[2])!)
            for type in [CGEventType.leftMouseDown, .leftMouseUp] {
                CGEvent(mouseEventSource: nil, mouseType: type, mouseCursorPosition: point,
                        mouseButton: .left)?.post(tap: .cghidEventTap)
            }
        """
        subprocess.run(["/usr/bin/swift", "-e", code, *map(str, point)], check=True, timeout=20)

    def _query_buttons(self, label, press=False):
        command = 'perform action "AXPress" of itemRef' if press else ''
        quoted = json.dumps(label or "", ensure_ascii=False)
        return self.ui(f"""
            set matches to {{}}
            set elements to entire contents of window "Multi Codex Menu"
            repeat with itemRef in elements
                set itemRole to missing value
                try
                    set itemRole to role of itemRef
                end try
                if itemRole is "AXButton" then
                    -- WebKit may expose an aria-label only as AXDescription.
                    -- Read each attribute independently so a missing title is harmless.
                    repeat with attributeName in {{"AXTitle", "AXDescription"}}
                        set itemLabel to missing value
                        try
                            set itemLabel to value of attribute (contents of attributeName) of itemRef
                        end try
                        if itemLabel is not missing value and itemLabel is not "" then
                            if itemLabel is {quoted} then
                                {command}
                                return "found"
                            end if
                            set end of matches to itemLabel
                        end if
                    end repeat
                end if
            end repeat
            return matches
        """)

    def button(self, label, press=False):
        return self._query_buttons(label, press) == "found"

    def button_labels(self):
        return self._query_buttons(None)

    def dismiss(self):
        self.ui("key code 53")

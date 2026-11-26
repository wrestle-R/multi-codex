"""Native click helpers for the packaged mascot and its dropdown."""

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
        self.button_log_offset = 0

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
        self.button_log_offset = self.log.stat().st_size
        self._click(point)

    @staticmethod
    def _click(point):
        code = """
            import CoreGraphics
            let args = CommandLine.arguments
            let point = CGPoint(x: Double(args[1])!, y: Double(args[2])!)
            for type in [CGEventType.leftMouseDown, .leftMouseUp] {
                let event = CGEvent(mouseEventSource: nil, mouseType: type,
                                    mouseCursorPosition: point, mouseButton: .left)!
                event.setIntegerValueField(.mouseEventClickState, value: 1)
                event.post(tap: .cghidEventTap)
            }
        """
        subprocess.run(["/usr/bin/swift", "-e", code, *map(str, point)], check=True, timeout=20)

    def buttons(self):
        pid = mascot_pid(self.binary)
        prefix = f"Multi Codex mascot buttons: {pid} "
        with self.log.open("rb") as log:
            log.seek(self.button_log_offset)
            lines = log.read().decode(errors="replace").splitlines(keepends=True)
        for line in reversed(lines):
            if line.startswith(prefix) and line.endswith("\n"):
                return json.loads(line.removeprefix(prefix))
        return {}

    def button(self, label, press=False):
        point = self.buttons().get(label)
        if point is None:
            return False
        if press:
            self.button_log_offset = self.log.stat().st_size
            self._click(point)
        return True

    def dismiss(self):
        self.ui("key code 53")

# Orbit and Orbiters

Orbit is the optional desktop companion strip in Multi Codex. Each account has a permanent **Orbiter** with its own appearance. The published v1.7.0 build is a preview with known issues; stable v1.6.1 does not include Orbit. See [all releases](/releases?channel=all) before choosing a preview.

## Turn Orbit on or off

Open **Settings**, change **Enable Orbit**, and save. Turning Orbit off hides the strip and silences its reminders; Codex chats stay open. Open **Orbit** in the launcher or tray for size, display, position, motion and sound preferences. Sound starts off.

## Sessions and usage

Launch Codex Desktop through Multi Codex. Orbit counts the local chat selected in each Desktop window, excluding retained backend chats and saved history. The current repair also observes running CLI sessions and selected conversations through the existing VS Code bridge. Older Desktop windows need their next managed launch to connect UI visibility; unavailable visibility is shown explicitly.

An Orbiter represents an account. Two chats use diagonal mini avatars; three or four use the corners. More than four sessions use an overflow control. Hover for sessions and reported usage. Click to open the exact Desktop chat, or focus the owning CLI or VS Code window. Open accounts animate gently; inactive accounts stay gray and still when the only-open filter is off.

Questions, approvals and possible requests for your help receive a badge and a quiet visual popup. Return to Codex to respond. Existing Desktop instances are not force-restarted to connect visibility.

## Desktop behavior

Orbit stays above ordinary windows and hides during fullscreen. Drag it to reposition it, select a display, or reset its position in Orbit preferences. Use **⌘ / Super + Shift + H** or the tray to hide or show it temporarily. Closing the launcher keeps Orbit and the tray alive. **Quit** exits the launcher and its companions.

Orbit supports Apple Silicon macOS 26+ and Linux Hyprland, GNOME and KDE. GNOME and KDE use the launcher's desktop integration. Unsupported desktops do not show a substitute strip. Windows account launching and shared chats remain supported; Orbit is not available on Windows.

# Orbit and Orbiters

Orbit is the optional desktop companion strip in Multi Codex. Each account has a permanent **Orbiter** with its own appearance. Stable v1.7.2 keeps recent Desktop conversations available independently of live activity. See [release details](/releases/v1.7.2).

## Turn Orbit on or off

Open **Settings**, change **Enable Orbit**, and save. Turning Orbit off hides the strip and silences its reminders; Codex chats stay open. Open **Orbit** in the launcher or tray for size, display, position, motion and sound preferences. Sound starts off.

## Sessions and usage

Orbit keeps the two most recent non-archived conversations from each account’s saved Codex history. They stay visible while idle, on New Chat, or when live activity reporting disconnects. These are recent shortcuts, not a count of currently open conversations. An account with only one saved chat shows one shortcut. Archived chats and background subagent conversations are excluded; if the index is temporarily unavailable, saved shortcuts remain with an availability notice.

Click a recent shortcut to navigate to that exact chat in the account’s existing Desktop app. If the account app is closed, clicking opens it normally. Opening Desktop from the launcher no longer asks for a project folder or opens a project.

VS Code keeps one shortcut per managed editor window, including an empty New Chat composer. Clicking focuses that window. CLI sessions remain available where the terminal window can be identified; ambiguous navigation reports an error instead of selecting another window.

Two shortcuts use diagonal mini avatars; three or four use the corners, with overflow for more. Hover for session details and reported usage. Idle avatars are still. Only a small working-status dot can pulse when enabled; Reduce motion disables that pulse too.

Questions, approvals and possible requests for help receive a badge and a simple static reminder with the account name. Click to return to the actual client to respond. Live activity and attention require a connected managed client; the recent Desktop shortcuts do not. Existing clients are never force-restarted.

## Desktop behavior

Orbit stays above ordinary windows and hides during fullscreen. Drag it to reposition it, select a display, or reset its position in Orbit preferences. Use **⌘ / Super + Shift + H** or the tray to hide or show it temporarily. Closing the launcher keeps Orbit and the tray alive. **Quit** exits the launcher and its companions.

Orbit supports Apple Silicon macOS 26+ and Linux Hyprland, GNOME and KDE. GNOME and KDE use the launcher's desktop integration. Unsupported desktops do not show a substitute strip. Windows account launching and shared chats remain supported; Orbit is not available on Windows.

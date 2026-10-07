// Published GitHub releases, newest first. Dates are GitHub's published_at values.
// Highlights come from release notes or the commits between published tags.
// Draft validation candidates are excluded from this public history.
export const releaseHistory = [
  { tag: "v1.3.8", publishedAt: "2026-10-07T06:27:55Z", title: "A little more room.", highlights: [
    "A cleaner terminal picker shows Automatic and installed terminals, with keyboard navigation and a checkmark for your selection.",
    "Workspace and Launching have their own settings sections. Save and Cancel stay visible while advanced options scroll.",
  ] },
  { tag: "v1.3.7", publishedAt: "2026-10-07T05:26:40Z", title: "Your terminal. Timely reminders.", highlights: [
    "Choose your preferred CLI terminal in Launch settings, including Kitty, Ghostty and the platform’s native terminals.",
    "Grouped reminders show available resets expiring within 48 hours, at most twice a day and at least four hours apart.",
    "The Orange theme preview now matches dark mode.",
  ] },
  { tag: "v1.3.6", publishedAt: "2026-10-07T02:49:39Z", title: "A CLI for every account.", highlights: [
    "Launch the Codex CLI from an account row, using that account’s isolated home and your chosen project folder.",
    "Usage refreshes at startup, accounts follow plan order, and six color palettes work in light and dark mode.",
  ] },
  { tag: "v1.3.5", publishedAt: "2026-10-06T06:28:44Z", title: "More ways to open your workspace.", highlights: [
    "Launch an account in VS Code, the supported Codex desktop app, or both, with separate account credentials and desktop state.",
    "A first-run welcome detects installed apps, and Launch settings adds a preferred folder with a built-in Browse picker.",
  ] },
  { tag: "v1.2.3", publishedAt: "2026-09-28T06:09:04Z", title: "Pick a folder without leaving.", highlights: [
    "A built-in workspace folder browser opens over the app and passes your selection to the existing desktop chooser.",
  ] },
  { tag: "v1.2.1", publishedAt: "2026-09-28T05:29:29Z", title: "The right size on Hyprland.", highlights: [
    "The main app opens full-screen on Hyprland, while folder and desktop selection remain compact dialogs.",
  ] },
  { tag: "v1.2.0", publishedAt: "2026-09-28T05:06:27Z", title: "A place for every workspace.", highlights: [
    "On Hyprland, choose the current desktop or desktops 1–10 for a newly launched VS Code window.",
    "Inspect storage by account and safely clear reclaimable caches while keeping credentials, conversations and settings.",
  ] },
  { tag: "v1.1.7", publishedAt: "2026-09-27T18:02:10Z", title: "Clearer guidance, real context.", highlights: [
    "Updated interface screenshots show account limits, alongside clearer instructions for updating the app.",
  ] },
  { tag: "v1.1.6", publishedAt: "2026-09-14T14:47:22Z", title: "Open another window with confidence.", highlights: [
    "Repeated launches preserve the account’s existing profile while opening another VS Code window.",
  ] },
  { tag: "v1.1.3", publishedAt: "2026-09-14T08:16:03Z", title: "Simpler launches. Fresher limits.", highlights: [
    "Simplified account launches and improved live limit refresh, including clearer handling of expired sessions.",
  ] },
  { tag: "v1.1.2", publishedAt: "2026-09-14T07:05:31Z", title: "Keep each account’s session separate.", highlights: [
    "Credentials, extensions, browser sign-in, workspaces and local history stay isolated so accounts cannot replace one another’s active login.",
  ] },
  { tag: "v1.1.1", publishedAt: "2026-09-05T13:58:17Z", title: "Limits, straight from Codex.", highlights: [
    "Live account limit windows replace manual usage entry, with remaining percentages and reset times reported by Codex.",
  ] },
  { tag: "v1.0.2", publishedAt: "2026-09-05T04:18:48Z", title: "An easier install on Linux and Mac.", highlights: [
    "Added Arch Linux and macOS installers, a verified AppImage update script, and corrected desktop launcher icons.",
  ] },
  { tag: "v1.0.0", publishedAt: "2026-09-04T19:30:11Z", title: "See where your usage stands.", highlights: [
    "Optional account usage tracking adds a way to record and view usage alongside saved accounts.",
  ] },
  { tag: "v0.2.0", publishedAt: "2026-09-04T15:56:00Z", title: "At home in your app launcher.", highlights: [
    "AppImage desktop integration adds a launcher entry and icon, with a more polished account interface.",
  ] },
  { tag: "v0.1.1", publishedAt: "2026-09-04T14:40:22Z", title: "Downloads you can verify.", highlights: [
    "Corrected the published Linux artifact checksums so downloads can be checked against the release’s actual packages.",
  ] },
  { tag: "v0.1.0", publishedAt: "2026-09-04T14:26:30Z", title: "The first workspace.", highlights: [
    "The first Linux release launches multiple Codex accounts in isolated VS Code windows while preserving your current login.",
  ] },
] as const

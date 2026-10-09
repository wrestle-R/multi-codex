// Selected published releases, newest first. Dates are GitHub's published_at values.
// Highlights come from release notes or the commits between published tags.
// Draft validation candidates are excluded from this public history.
export const releaseHistory = [
  { tag: "v1.4.0", publishedAt: "2026-10-09T10:08:39Z", title: "Windows support and reliable account switching", highlights: [
    "Native Windows x64 launcher and extension packages add private account storage, isolated launches and verified install/update scripts.",
    "Mac account menus, all-account usage refresh, connection recovery, long-chat support and persistent Hyprland maximization improve everyday use.",
  ] },
  { tag: "v1.3.8", publishedAt: "2026-10-07T06:27:55Z", title: "Launch settings and terminal picker improvements", highlights: [
    "A cleaner terminal picker shows Automatic and installed terminals, with keyboard navigation and a checkmark for your selection.",
    "Workspace and Launching have their own settings sections. Save and Cancel stay visible while advanced options scroll.",
  ] },
  { tag: "v1.3.7", publishedAt: "2026-10-07T05:26:40Z", title: "Terminal preferences and usage reset reminders", highlights: [
    "Choose your preferred CLI terminal in Launch settings, including Kitty, Ghostty and the platform’s native terminals.",
    "Grouped reminders show available resets expiring within 48 hours, at most twice a day and at least four hours apart.",
    "The Orange theme preview now matches dark mode.",
  ] },
  { tag: "v1.3.6", publishedAt: "2026-10-07T02:49:39Z", title: "CLI launches for saved accounts", highlights: [
    "Launch the Codex CLI from an account row, using that account’s isolated home and your chosen project folder.",
    "Usage refreshes at startup, accounts follow plan order, and six color palettes work in light and dark mode.",
  ] },
  { tag: "v1.3.5", publishedAt: "2026-10-06T06:28:44Z", title: "Desktop launch options and first-run setup", highlights: [
    "Launch an account in VS Code, the supported Codex desktop app, or both, with separate account credentials and desktop state.",
    "A first-run welcome detects installed apps, and Launch settings adds a preferred folder with a built-in Browse picker.",
  ] },
  { tag: "v1.2.3", publishedAt: "2026-09-28T06:09:04Z", title: "Workspace folder picker", highlights: [
    "A built-in workspace folder browser opens over the app and passes your selection to the existing desktop chooser.",
  ] },
  { tag: "v1.2.1", publishedAt: "2026-09-28T05:29:29Z", title: "Hyprland window sizing fix", highlights: [
    "The main app opens full-screen on Hyprland, while folder and desktop selection remain compact dialogs.",
  ] },
  { tag: "v1.2.0", publishedAt: "2026-09-28T05:06:27Z", title: "Hyprland desktop selection and storage controls", highlights: [
    "On Hyprland, choose the current desktop or desktops 1–10 for a newly launched VS Code window.",
    "Inspect storage by account and safely clear reclaimable caches while keeping credentials, conversations and settings.",
  ] },
  { tag: "v1.1.7", publishedAt: "2026-09-27T18:02:10Z", title: "Documentation and screenshot updates", highlights: [
    "Updated interface screenshots show account limits, alongside clearer instructions for updating the app.",
  ] },
  { tag: "v1.1.6", publishedAt: "2026-09-14T14:47:22Z", title: "Repeated launch reliability", highlights: [
    "Repeated launches preserve the account’s existing profile while opening another VS Code window.",
  ] },
  { tag: "v1.1.3", publishedAt: "2026-09-14T08:16:03Z", title: "Launch and usage refresh fixes", highlights: [
    "Simplified account launches and improved live limit refresh, including clearer handling of expired sessions.",
  ] },
  { tag: "v1.1.2", publishedAt: "2026-09-14T07:05:31Z", title: "Account session isolation fixes", highlights: [
    "Credentials, extensions, browser sign-in, workspaces and local history stay isolated so accounts cannot replace one another’s active login.",
  ] },
  { tag: "v1.1.1", publishedAt: "2026-09-05T13:58:17Z", title: "Live account usage limits", highlights: [
    "Live account limit windows replace manual usage entry, with remaining percentages and reset times reported by Codex.",
  ] },
  { tag: "v1.0.2", publishedAt: "2026-09-05T04:18:48Z", title: "Linux and macOS installer improvements", highlights: [
    "Added Arch Linux and macOS installers, a verified AppImage update script, and corrected desktop launcher icons.",
  ] },
  { tag: "v1.0.0", publishedAt: "2026-09-04T19:30:11Z", title: "Manual account usage tracking", highlights: [
    "Optional account usage tracking adds a way to record and view usage alongside saved accounts.",
  ] },
  { tag: "v0.2.0", publishedAt: "2026-09-04T15:56:00Z", title: "Linux desktop launcher integration", highlights: [
    "AppImage desktop integration adds a launcher entry and icon, with a more polished account interface.",
  ] },
  { tag: "v0.1.1", publishedAt: "2026-09-04T14:40:22Z", title: "Linux package checksum correction", highlights: [
    "Corrected the published Linux artifact checksums so downloads can be checked against the release’s actual packages.",
  ] },
  { tag: "v0.1.0", publishedAt: "2026-09-04T14:26:30Z", title: "Initial Linux release", highlights: [
    "The first Linux release launches multiple Codex accounts in isolated VS Code windows while preserving your current login.",
  ] },
] as const


// Group summaries describe the series as a whole; individual entries retain
// their published versions, dates, anchors and links to the original notes.
const releaseSeries = [
  { series: "1.4", title: "Windows support and account switching", description: "Native Windows packages, Mac account menus, all-account usage refresh and connection recovery, with tested platform downloads." },
  { series: "1.3", title: "Desktop and CLI launch options", description: "Launch saved accounts in VS Code, Codex desktop or CLI, with folder preferences, terminal selection and usage reminders." },
  { series: "1.2", title: "Workspace and storage controls", description: "Choose a workspace folder, place VS Code windows on Hyprland desktops and manage account caches." },
  { series: "1.1", title: "Live usage and account isolation", description: "Read account limits from Codex and improve session separation, usage refresh and repeated launches." },
  { series: "1.0", title: "Usage tracking and installation", description: "Record account usage manually and install or update the app with platform-specific scripts." },
  { series: "0.2", title: "Linux desktop integration", description: "Open the AppImage from your application launcher with a desktop entry and icon." },
  { series: "0.1", title: "Initial Linux builds", description: "Launch saved accounts in separate VS Code windows and verify Linux package downloads." },
] as const

export const releaseGroups = releaseSeries.map(group => ({
  ...group,
  releases: releaseHistory.filter(release => release.tag.startsWith(`v${group.series}.`)),
}))

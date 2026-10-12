export type RuntimeStatus = "idle" | "running" | "error"

export interface Profile {
  id: string
  name: string
  authMode: string
  notes?: string | null
  createdAt: string
  updatedAt: string
  accountTier?: string
  status: RuntimeStatus
  error?: string | null
}

export interface SaveProfileInput {
  name: string
  authJson: string
  notes?: string
}

export interface ProfileDetails {
  notes?: string
}

export interface LimitWindow {
  remainingPercent: number
  resetsAt: number | null
}

export interface ProfileLimits {
  fiveHour: LimitWindow | null
  weekly: LimitWindow | null
  monthly: LimitWindow | null
  resetCreditsAvailable: number | null
  resetCredits: ResetCredit[] | null
  checkedAt: string
}

export interface ResetCredit {
  id: string
  status: string
  resetType: string
  grantedAt: number
  expiresAt: number | null
  title?: string | null
  description?: string | null
}

export interface ProfileStorageUsage {
  id: string
  name: string
  bytes: number
  reclaimableBytes: number
  running: boolean
  layout?: "legacy" | "shared" | "needsRecovery" | "needsRestore"
  privateCatalogBytes?: number
  privateRuntimeBytes?: number
  breakdown?: {
    codexBytes: number
    vscodeBytes: number
    extensionsBytes: number
    desktopBytes: number
    chatBytes: number
  }
}

export type ChatScope = "all" | "olderThan30Days"

export interface ChatPreview {
  token: string
  count: number
  transcriptBytes: number
  protectedCount: number
}

export interface ChatCleanupResult {
  deletedCount: number
  transcriptBytes: number
}

export interface UpdateProgress {
  phase: "checking" | "downloading" | "installing" | "restarting" | "current"
  downloaded: number
  total: number | null
  version: string | null
}

export interface StorageUsage {
  bytes: number
  reclaimableBytes: number
  otherBytes: number
  profiles: ProfileStorageUsage[]
  sharedExtensionsBytes?: number
  sharedChatsBytes?: number
  migrationBytes?: number
  backupBytes?: number
}

export interface SharedStorageLayout {
  schema: number
  profileId: string
  operationId: string
  phase: "prepared" | "settingsApplied" | "committed" | "rollbackPending" | "rolledBack"
  snapshot: MigrationSnapshot
  generation: { id: string; platform: string; architecture: string }
  catalogName: string
}

export interface SharedStorageQueue {
  schema: number
  enabled: boolean
  profiles: Record<string, { state: "ready" | "deferred" | "needsAttention"; reason: string | null; version: string }>
}

export interface LaunchEnvironment {
  defaultWorkspace: string
  capabilities: DesktopCapabilities
}

export interface WorkspaceDirectory {
  name: string
  path: string
}

export interface WorkspaceDirectoryListing {
  path: string
  parentPath: string | null
  directories: WorkspaceDirectory[]
}

export interface LimitCheckState {
  loading: boolean
  data?: ProfileLimits
  error?: string
}

export interface DesktopIntegrationStatus {
  available: boolean
  installed: boolean
  desktopShortcut: boolean
  version: string
  source: "appimage" | "package"
}

export interface DeviceLoginEvent {
  id: string
  output?: string
  completed: boolean
  error?: string
}

export interface DesktopCapabilities {
  backend: string
  enumerateDesktops: boolean
  enumerateWindows: boolean
  moveWindows: boolean
  reason: string | null
}

export interface DesktopWindow {
  id: string
  pid: number
  application: string
  title: string
  icon?: string | null
}

export interface Desktop {
  id: string
  name: string
  monitor: string | null
  current: boolean
  windows: DesktopWindow[]
}

export interface DesktopInventory {
  protocolVersion: number
  capabilities: DesktopCapabilities
  desktops: Desktop[]
}

export interface LaunchResult {
  completed: boolean
  error: string | null
  retryToken: string | null
}

export interface ExecutableSettings {
  codePath: string | null
  codexPath: string | null
  globalCodexHome: string | null
  preferredWorkspace?: string | null
  hideDesktopPicker?: boolean
  onboardingCompleted?: boolean
  detectedApps?: LaunchTargets | null
  launchMode?: LaunchMode
  cliTerminal?: TerminalId
}

export type TerminalId = "automatic" | "powerShell" | "windowsTerminal" | "terminal" | "kitty" | "ghostty" | "konsole" | "gnomeTerminal" | "xfceTerminal" | "alacritty" | "foot" | "xterm"
export interface TerminalOption {
  id: TerminalId
  label: string
  available: boolean
}

export type LaunchTarget = "vscode" | "standalone" | "cli"
export type LaunchMode = LaunchTarget | "both"

export interface LaunchTargets {
  platform: string
  vscodeInstalled: boolean
  codexCliAvailable: boolean
  standaloneInstalled: boolean
  standaloneVerified: boolean
  standaloneVersion?: string | null
}

export interface MigrationSnapshot {
  schema: number
  profileId: string
  snapshotId: string
  state: "copying" | "verified"
  sourceDigest: string
  metadataDigest: string
  sourceBytes: number
  entries: number
  createdAt: string
  verifiedAt: string | null
}

export interface MigrationPreview {
  profileId: string
  sourceBytes: number
  entries: number
  requiredBackupBytes: number
  availableBackupBytes: number
  requiredAppBytes?: number
  availableAppBytes?: number
  externalLinks: string[]
  running: boolean
  snapshot: MigrationSnapshot | null
}

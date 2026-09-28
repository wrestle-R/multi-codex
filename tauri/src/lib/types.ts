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
}

export interface StorageUsage {
  bytes: number
  reclaimableBytes: number
  otherBytes: number
  profiles: ProfileStorageUsage[]
}

export interface LaunchEnvironment {
  defaultWorkspace: string
  hyprland: boolean
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

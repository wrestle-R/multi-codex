export type RuntimeStatus = "idle" | "launching" | "running" | "error"

export interface Profile {
  id: string
  name: string
  authMode: string
  notes?: string | null
  createdAt: string
  updatedAt: string
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
  resetCreditsAvailable: number | null
  checkedAt: string
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

export type RuntimeStatus = "idle" | "launching" | "running" | "error"

export interface Profile {
  id: string
  name: string
  authMode: string
  requestsRemaining?: number | null
  notes?: string | null
  resetDate?: string | null
  createdAt: string
  updatedAt: string
  status: RuntimeStatus
  error?: string | null
}

export interface SaveProfileInput {
  name: string
  authJson: string
  requestsRemaining?: number
  notes?: string
  resetDate?: string
}

export interface ProfileDetails {
  requestsRemaining?: number
  notes?: string
  resetDate?: string
}

export interface DesktopIntegrationStatus {
  available: boolean
  installed: boolean
  desktopShortcut: boolean
  version: string
  source: "appimage" | "package"
}

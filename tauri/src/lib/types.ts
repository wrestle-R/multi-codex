export type RuntimeStatus = "idle" | "launching" | "running" | "error"

export interface Profile {
  id: string
  name: string
  authMode: string
  createdAt: string
  updatedAt: string
  status: RuntimeStatus
  error?: string | null
}

export interface SaveProfileInput {
  name: string
  authJson: string
}


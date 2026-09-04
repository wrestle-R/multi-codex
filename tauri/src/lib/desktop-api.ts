import { invoke } from "@tauri-apps/api/core"
import type { DesktopIntegrationStatus, Profile, ProfileDetails, SaveProfileInput } from "./types"

const isTauri = typeof window !== "undefined" && Boolean(window.__TAURI_INTERNALS__)

let demoProfiles: Profile[] = [
  {
    id: "demo-personal",
    name: "Personal",
    authMode: "ChatGPT",
    requestsRemaining: 42,
    notes: "Personal projects and experiments",
    resetDate: "2026-09-12",
    createdAt: "2026-09-01T09:30:00Z",
    updatedAt: "2026-09-04T08:10:00Z",
    status: "idle",
  },
  {
    id: "demo-work",
    name: "Work",
    authMode: "ChatGPT",
    requestsRemaining: 18,
    notes: "Client work",
    resetDate: "2026-09-08",
    createdAt: "2026-09-02T11:20:00Z",
    updatedAt: "2026-09-04T07:45:00Z",
    status: "running",
  },
]

const wait = () => new Promise((resolve) => window.setTimeout(resolve, 120))

export async function listProfiles(): Promise<Profile[]> {
  if (isTauri) return invoke<Profile[]>("list_profiles")
  await wait()
  return structuredClone(demoProfiles)
}

export async function addProfile(input: SaveProfileInput): Promise<Profile> {
  if (isTauri) return invoke<Profile>("add_profile", { input })
  JSON.parse(input.authJson)
  const profile: Profile = {
    id: crypto.randomUUID(),
    name: input.name.trim(),
    authMode: "ChatGPT",
    requestsRemaining: input.requestsRemaining,
    notes: input.notes,
    resetDate: input.resetDate,
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
    status: "idle",
  }
  demoProfiles = [...demoProfiles, profile]
  return profile
}

export async function importCurrentProfile(name: string, details: ProfileDetails): Promise<Profile> {
  if (isTauri) return invoke<Profile>("import_current_profile", { name, ...details })
  return addProfile({
    name,
    authJson: '{"auth_mode":"chatgpt","tokens":{"access_token":"demo"}}',
    ...details,
  })
}

export async function updateProfile(
  id: string,
  name: string,
  authJson?: string,
  details: ProfileDetails = {},
): Promise<Profile> {
  if (isTauri) return invoke<Profile>("update_profile", { id, name, authJson, ...details })
  if (authJson) JSON.parse(authJson)
  const updatedAt = new Date().toISOString()
  demoProfiles = demoProfiles.map((profile) =>
    profile.id === id ? { ...profile, name: name.trim(), ...details, updatedAt } : profile,
  )
  return demoProfiles.find((profile) => profile.id === id)!
}

export async function launchProfile(id: string): Promise<void> {
  if (isTauri) return invoke("launch_profile", { id })
  demoProfiles = demoProfiles.map((profile) =>
    profile.id === id ? { ...profile, status: "running" } : profile,
  )
}

export async function deleteProfile(id: string): Promise<void> {
  if (isTauri) return invoke("delete_profile", { id })
  demoProfiles = demoProfiles.filter((profile) => profile.id !== id)
}

export async function getDesktopIntegrationStatus(): Promise<DesktopIntegrationStatus> {
  if (isTauri) return invoke<DesktopIntegrationStatus>("get_desktop_integration_status")
  return {
    available: false,
    installed: true,
    desktopShortcut: false,
    version: "development",
    source: "package",
  }
}

export async function installDesktopIntegration(
  createDesktopShortcut: boolean,
): Promise<DesktopIntegrationStatus> {
  if (isTauri) {
    return invoke<DesktopIntegrationStatus>("install_desktop_integration", {
      createDesktopShortcut,
    })
  }
  return {
    available: true,
    installed: true,
    desktopShortcut: createDesktopShortcut,
    version: "development",
    source: "appimage",
  }
}

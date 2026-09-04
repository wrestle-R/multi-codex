import { invoke } from "@tauri-apps/api/core"
import type { Profile, SaveProfileInput } from "./types"

const isTauri = typeof window !== "undefined" && Boolean(window.__TAURI_INTERNALS__)

let demoProfiles: Profile[] = [
  {
    id: "demo-personal",
    name: "Personal",
    authMode: "ChatGPT",
    createdAt: "2026-09-01T09:30:00Z",
    updatedAt: "2026-09-04T08:10:00Z",
    status: "idle",
  },
  {
    id: "demo-work",
    name: "Work",
    authMode: "ChatGPT",
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
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
    status: "idle",
  }
  demoProfiles = [...demoProfiles, profile]
  return profile
}

export async function importCurrentProfile(name: string): Promise<Profile> {
  if (isTauri) return invoke<Profile>("import_current_profile", { name })
  return addProfile({
    name,
    authJson: '{"auth_mode":"chatgpt","tokens":{"access_token":"demo"}}',
  })
}

export async function updateProfile(
  id: string,
  name: string,
  authJson?: string,
): Promise<Profile> {
  if (isTauri) return invoke<Profile>("update_profile", { id, name, authJson })
  if (authJson) JSON.parse(authJson)
  const updatedAt = new Date().toISOString()
  demoProfiles = demoProfiles.map((profile) =>
    profile.id === id ? { ...profile, name: name.trim(), updatedAt } : profile,
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


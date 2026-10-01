import { invoke } from "@tauri-apps/api/core"
import type { DesktopInventory, ExecutableSettings, LaunchResult, DesktopIntegrationStatus, DeviceLoginEvent, LaunchEnvironment, Profile, ProfileDetails, ProfileLimits, SaveProfileInput, StorageUsage, WorkspaceDirectoryListing } from "./types"

const isTauri = typeof window !== "undefined" && Boolean(window.__TAURI_INTERNALS__)

let demoProfiles: Profile[] = [
  {
    id: "demo-personal",
    name: "Personal",
    authMode: "ChatGPT",
    notes: "Personal projects and experiments",
    createdAt: "2026-09-01T09:30:00Z",
    updatedAt: "2026-09-04T08:10:00Z",
    accountTier: "Free",
    status: "idle",
  },
  {
    id: "demo-work",
    name: "Work",
    authMode: "ChatGPT",
    notes: "Client work",
    createdAt: "2026-09-02T11:20:00Z",
    updatedAt: "2026-09-04T07:45:00Z",
    accountTier: "Plus",
    status: "running",
  },
  {
    id: "demo-studio",
    name: "Studio",
    authMode: "ChatGPT",
    notes: "Shared projects and experiments",
    createdAt: "2026-09-03T11:20:00Z",
    updatedAt: "2026-09-04T07:30:00Z",
    accountTier: "Go",
    status: "idle",
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
    notes: input.notes,
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

export async function beginDeviceLogin(name: string, details: ProfileDetails, profileId?: string): Promise<string> {
  if (isTauri) return invoke<string>("begin_device_login", { name, ...details, profileId })
  return crypto.randomUUID()
}

export async function cancelDeviceLogin(id: string): Promise<void> {
  if (isTauri) return invoke("cancel_device_login", { id })
}

export async function subscribeDeviceLogin(listener: (event: DeviceLoginEvent) => void): Promise<() => void> {
  if (!isTauri) return () => undefined
  const { listen } = await import("@tauri-apps/api/event")
  return listen<DeviceLoginEvent>("device-login", (event) => listener(event.payload))
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

export async function listWorkspaceDirectories(path: string): Promise<WorkspaceDirectoryListing> {
  if (isTauri) return invoke<WorkspaceDirectoryListing>("list_workspace_directories", { path })
  return {
    path,
    parentPath: path === "/home/rdp/Desktop/code" ? "/home/rdp/Desktop" : null,
    directories: [],
  }
}

export async function launchProfile(id: string, workspace: string, desktop: string | null, retryToken: string | null = null): Promise<LaunchResult> {
  if (isTauri) return invoke("launch_profile", { id, workspace, desktop, retryToken })
  demoProfiles = demoProfiles.map((profile) =>
    profile.id === id ? { ...profile, status: "running" } : profile,
  )
  return { completed: true, error: null, retryToken: null }
}

export async function deleteProfile(id: string): Promise<void> {
  if (isTauri) return invoke("delete_profile", { id })
  demoProfiles = demoProfiles.filter((profile) => profile.id !== id)
}

export async function checkProfileLimits(id: string): Promise<ProfileLimits> {
  if (isTauri) return invoke<ProfileLimits>("check_profile_limits", { id })
  await wait()
  const profile = demoProfiles.find((item) => item.id === id)
  if (!profile) throw new Error("Profile not found")
  if (profile.authMode.toLowerCase() !== "chatgpt") {
    throw new Error("Live limits are available only for ChatGPT accounts")
  }
  const now = Math.floor(Date.now() / 1000)
  return {
    fiveHour: { remainingPercent: 64, resetsAt: now + 2 * 60 * 60 },
    weekly: { remainingPercent: 81, resetsAt: now + 4 * 24 * 60 * 60 },
    monthly: null,
    resetCreditsAvailable: 2,
    resetCredits: [
      { id: "demo-credit-1", status: "available", resetType: "codexRateLimits", grantedAt: now - 3600, expiresAt: now + 2 * 24 * 60 * 60, title: "Usage reset" },
      { id: "demo-credit-2", status: "available", resetType: "codexRateLimits", grantedAt: now - 3600, expiresAt: now + 5 * 24 * 60 * 60, title: "Usage reset" },
    ],
    checkedAt: new Date().toISOString(),
  }
}

export async function getStorageUsage(): Promise<StorageUsage> {
  if (isTauri) return invoke<StorageUsage>("get_storage_usage")
  await wait()
  return {
    bytes: 9_876_543_210,
    reclaimableBytes: 2_143_000_000,
    otherBytes: 12_000,
    profiles: demoProfiles.map((profile, index) => ({
      id: profile.id,
      name: profile.name,
      bytes: index === 0 ? 3_900_000_000 : index === 1 ? 3_100_000_000 : 2_876_531_210,
      reclaimableBytes: index === 0 ? 980_000_000 : index === 1 ? 720_000_000 : 443_000_000,
      running: profile.status === "running",
    })),
  }
}

export async function clearProfileCache(id: string): Promise<number> {
  if (isTauri) return invoke<number>("clear_profile_cache", { id })
  const usage = await getStorageUsage()
  return usage.profiles.find((profile) => profile.id === id)?.reclaimableBytes ?? 0
}

export async function getLaunchEnvironment(): Promise<LaunchEnvironment> {
  if (isTauri) return invoke<LaunchEnvironment>("get_launch_environment")
  return { defaultWorkspace: "/home/rdp/Desktop/code", capabilities: (await getDesktopInventory()).capabilities }
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

export async function getDesktopInventory(): Promise<DesktopInventory> {
  if (isTauri) return invoke("get_desktop_inventory")
  return {
    protocolVersion: 1,
    capabilities: { backend: "preview", enumerateDesktops: true, enumerateWindows: true, moveWindows: true, reason: null },
    desktops: Array.from({ length: 10 }, (_, index) => ({
      id: `preview:${index + 1}`,
      name: `Desktop ${index + 1}`,
      monitor: "Main display",
      current: index === 5,
      windows: index === 0 ? [{ id: "browser", pid: 1, application: "Zen Browser", title: "Documentation" }]
        : index === 3 ? [{ id: "code", pid: 2, application: "Visual Studio Code", title: "Project — Visual Studio Code" }, { id: "reference", pid: 3, application: "Firefox", title: "API reference" }]
        : index === 4 ? [{ id: "code-second", pid: 4, application: "Visual Studio Code", title: "Welcome — Visual Studio Code" }]
        : index === 5 ? [{ id: "launcher", pid: 5, application: "Multi Codex", title: "Accounts" }]
        : [],
    })),
  }
}

export async function discardPlacement(retryToken: string): Promise<void> {
  if (isTauri) await invoke("discard_placement", { retryToken })
}

export async function getExecutableSettings(): Promise<ExecutableSettings> {
  if (isTauri) return invoke("get_executable_settings")
  return JSON.parse(localStorage.getItem("multi-codex-executables") ?? '{"codePath":null,"codexPath":null,"globalCodexHome":null}')
}

export async function saveExecutableSettings(settings: ExecutableSettings): Promise<ExecutableSettings> {
  if (isTauri) return invoke("save_executable_settings", { settings })
  localStorage.setItem("multi-codex-executables", JSON.stringify(settings))
  return settings
}

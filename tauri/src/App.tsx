import { orderProfilesByPlan } from "./lib/profile-order"
import { useProfileLimits } from "./components/use-profile-limits"
import { ThemePicker } from "./components/theme-picker"
import { initialColorTheme, type ColorTheme } from "./lib/themes"
import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import { flushSync } from "react-dom"
import {
  Add01Icon,
  DownloadSquare01Icon,
  HardDriveIcon,
  Moon02Icon,
  Refresh01Icon,
  Sun03Icon,
} from "@hugeicons/core-free-icons"
import { HugeiconsIcon } from "@hugeicons/react"
import "./App.css"
import { BrandMark } from "./components/brand-mark"
import { CacheConfirmDialog } from "./components/cache-confirm-dialog"
import { ConfirmDialog } from "./components/confirm-dialog"
import { LaunchSettingsDialog } from "./components/launch-settings-dialog"
import { DesktopIntegrationDialog } from "./components/desktop-integration-dialog"
import { ProfileDialog } from "./components/profile-dialog"
import { ProfileRow } from "./components/profile-row"
import { ResetExpiryReminder } from "./components/reset-expiry-reminder"
import { ResetCreditsDialog } from "./components/reset-credits-dialog"
import { StorageDialog } from "./components/storage-dialog"
import { WelcomeScreen } from "./components/welcome-screen"
import { WorkspaceDialog } from "./components/workspace-dialog"
import { WorkspacePickerDialog } from "./components/workspace-picker-dialog"
import {
  addProfile,
  beginDeviceLogin,
  cancelDeviceLogin,
  clearProfileCache,
  deleteProfile,
  getDesktopIntegrationStatus,
  discardPlacement,
  getLaunchEnvironment,
  getExecutableSettings,
  getLaunchTargets,
  saveExecutableSettings,
  getStorageUsage,
  importCurrentProfile,
  installDesktopIntegration,
  launchProfile,
  launchCliProfile,
  launchStandaloneProfile,
  listProfiles,
  subscribeDeviceLogin,
  updateProfile,
} from "./lib/desktop-api"
import type { LaunchMode, LaunchTarget, LaunchTargets, DesktopIntegrationStatus, DeviceLoginEvent, LaunchEnvironment, Profile, ProfileDetails, ProfileLimits, ProfileStorageUsage, StorageUsage } from "./lib/types"
import { formatStorage } from "./lib/formatters"

export { formatStorage } from "./lib/formatters"

type Theme = "light" | "dark"

function enabledLaunchTargets(apps: LaunchTargets | null, mode: LaunchMode): LaunchTarget[] {
  if (!apps) return ['vscode']
  const targets: LaunchTarget[] = []
  if (mode !== 'cli') {
    if (apps.vscodeInstalled && (mode !== 'standalone' || !apps.standaloneInstalled)) targets.push('vscode')
    if (apps.standaloneVerified && (mode !== 'vscode' || !apps.vscodeInstalled)) targets.push('standalone')
  }
  if (apps.codexCliAvailable) targets.push('cli')
  return targets
}

function initialTheme(): Theme {
  const saved = localStorage.getItem("multi-codex-theme")
  if (saved === "light" || saved === "dark") return saved
  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light"
}

export default function App() {
  const [profiles, setProfiles] = useState<Profile[]>([])
  const [loading, setLoading] = useState(true)
  const [pageError, setPageError] = useState<string | null>(null)
  const [dialogProfile, setDialogProfile] = useState<Profile | null | undefined>(undefined)
  const [deleteTarget, setDeleteTarget] = useState<Profile | null>(null)
  const [dialogError, setDialogError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const [theme, setTheme] = useState<Theme>(initialTheme)
  const [colorTheme, setColorTheme] = useState<ColorTheme>(initialColorTheme)
  const explicitMode = useRef(["light", "dark"].includes(localStorage.getItem("multi-codex-theme") ?? ""))
  const [integrationStatus, setIntegrationStatus] = useState<DesktopIntegrationStatus | null>(null)
  const [showIntegration, setShowIntegration] = useState(false)
  const [integrationBusy, setIntegrationBusy] = useState(false)
  const [integrationError, setIntegrationError] = useState<string | null>(null)
  const [storageUsage, setStorageUsage] = useState<StorageUsage | null>(null)
  const [storageLoading, setStorageLoading] = useState(false)
  const [storageError, setStorageError] = useState<string | null>(null)
  const [showStorage, setShowStorage] = useState(false)
  const [cacheTarget, setCacheTarget] = useState<ProfileStorageUsage | null>(null)
  const [cacheBusy, setCacheBusy] = useState(false)
  const [cacheError, setCacheError] = useState<string | null>(null)
  const [launchEnvironment, setLaunchEnvironment] = useState<LaunchEnvironment | null>(null)
  const [launchRequest, setLaunchRequest] = useState<{ profile: Profile; workspace: string; target: LaunchTarget } | null>(null)
  const [folderRequest, setFolderRequest] = useState<{ profile: Profile; initialPath: string; target: LaunchTarget } | null>(null)
  const [showLaunchSettings, setShowLaunchSettings] = useState(false)
  const [launchTargets, setLaunchTargets] = useState<LaunchTargets | null>(null)
  const [launchMode, setLaunchMode] = useState<LaunchMode>('vscode')
  const rowLaunchTargets = enabledLaunchTargets(launchTargets, launchMode)
  const [welcomeNeeded, setWelcomeNeeded] = useState(true)
  const [welcomeReady, setWelcomeReady] = useState(false)
  const [welcomeBusy, setWelcomeBusy] = useState(false)
  const [welcomeError, setWelcomeError] = useState<string | null>(null)
  const [hideDesktopPicker, setHideDesktopPicker] = useState(false)
  const [launchBusy, setLaunchBusy] = useState(false)
  const [launchPreparationError, setLaunchPreparationError] = useState<string | null>(null)
  const [launchError, setLaunchError] = useState<string | null>(null)
  const [placementToken, setPlacementToken] = useState<string | null>(null)
  const launchLock = useRef(false)
  const [creditsTarget, setCreditsTarget] = useState<{ profile: Profile; limits: ProfileLimits } | null>(null)
  const { limitChecks, refreshingAllLimits, handleCheckLimits, handleRefreshAllLimits } = useProfileLimits(profiles, !loading && !pageError)
  const [deviceLogin, setDeviceLogin] = useState<{ id: string; output: string[] } | null>(null)
  const deviceLoginRef = useRef<typeof deviceLogin>(null)
  const refreshPromiseRef = useRef<Promise<void> | null>(null)
  const nextThemeRevealRef = useRef<"expand" | "contract">("expand")

  const refresh = useCallback(() => {
    if (refreshPromiseRef.current) return refreshPromiseRef.current
    const request = (async () => {
      try {
        const next = await listProfiles()
        setProfiles(next)
        setPageError(null)
      } catch (error) {
        setPageError(error instanceof Error ? error.message : String(error))
      } finally {
        setLoading(false)
      }
    })()
    refreshPromiseRef.current = request
    void request.finally(() => {
      if (refreshPromiseRef.current === request) refreshPromiseRef.current = null
    })
    return request
  }, [])

  useEffect(() => {
    let cancelled = false
    let timeout: number | undefined
    const poll = async () => {
      await refresh()
      if (!cancelled) timeout = window.setTimeout(() => void poll(), 2000)
    }
    void poll()
    return () => {
      cancelled = true
      if (timeout !== undefined) window.clearTimeout(timeout)
    }
  }, [refresh])

  useEffect(() => {
    void getDesktopIntegrationStatus().then((status) => {
      setIntegrationStatus(status)
      const dismissedVersion = localStorage.getItem("multi-codex-desktop-dismissed")
      if (status.available && !status.installed && dismissedVersion !== status.version) {
        setShowIntegration(true)
      }
    }).catch(() => undefined)
  }, [])

  useEffect(() => {
    void getLaunchEnvironment().then(setLaunchEnvironment).catch(() => undefined)
  }, [])

  useEffect(() => {
    let active = true
    void Promise.all([getExecutableSettings(), getLaunchTargets()]).then(([settings, targets]) => {
      if (!active) return
      setLaunchTargets(targets)
      setLaunchMode(settings.launchMode ?? 'vscode')
      setWelcomeNeeded(!settings.onboardingCompleted)
    }).catch(error => { if (active) setWelcomeError(String(error)) }).finally(() => { if (active) setWelcomeReady(true) })
    return () => { active = false }
  }, [])

  async function completeWelcome() {
    setWelcomeBusy(true)
    setWelcomeError(null)
    try {
      const [settings, detectedApps] = await Promise.all([getExecutableSettings(), getLaunchTargets()])
      const mode = detectedApps.vscodeInstalled && detectedApps.standaloneVerified ? 'both' : settings.launchMode ?? 'vscode'
      await saveExecutableSettings({ ...settings, onboardingCompleted: true, detectedApps, launchMode: mode })
      setLaunchTargets(detectedApps)
      setLaunchMode(mode)
      setWelcomeNeeded(false)
    } catch (error) {
      setWelcomeError(error instanceof Error ? error.message : String(error))
    } finally { setWelcomeBusy(false) }
  }

  const refreshStorage = useCallback(async () => {
    setStorageLoading(true)
    try {
      const usage = await getStorageUsage()
      setStorageUsage(usage)
      setStorageError(null)
    } catch (error) {
      setStorageError(error instanceof Error ? error.message : String(error))
    } finally {
      setStorageLoading(false)
    }
  }, [])

  useEffect(() => {
    let cancelled = false
    let timeout: number | undefined
    const poll = async () => {
      await refreshStorage()
      if (!cancelled) timeout = window.setTimeout(() => void poll(), 5 * 60_000)
    }
    void poll()
    return () => {
      cancelled = true
      if (timeout !== undefined) window.clearTimeout(timeout)
    }
  }, [refreshStorage])

  const applyTheme = useCallback((next: Theme) => {
    explicitMode.current = true
    setTheme(next)
    document.documentElement.classList.toggle("dark", next === "dark")
    document.documentElement.dataset.theme = next
    localStorage.setItem("multi-codex-theme", next)
  }, [])

  useEffect(() => {
    document.documentElement.classList.toggle("dark", theme === "dark")
    document.documentElement.dataset.theme = theme
  }, [theme])
  useEffect(() => {
    const system = window.matchMedia("(prefers-color-scheme: dark)")
    const followSystem = (event: MediaQueryListEvent) => {
      if (!explicitMode.current) setTheme(event.matches ? "dark" : "light")
    }
    system.addEventListener("change", followSystem)
    return () => system.removeEventListener("change", followSystem)
  }, [])
  useEffect(() => {
    document.documentElement.dataset.palette = colorTheme
    localStorage.setItem("multi-codex-color-theme", colorTheme)
  }, [colorTheme])

  const toggleTheme = useCallback((source: HTMLElement | null = null) => {
    const next: Theme = theme === "dark" ? "light" : "dark"
    const root = document.documentElement
    if (!source || typeof document.startViewTransition !== "function" || window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
      applyTheme(next)
      return
    }
    if (root.classList.contains("theme-transition")) return
    const rect = source.getBoundingClientRect()
    const x = rect.left + rect.width / 2
    const y = rect.top + rect.height / 2
    const radius = Math.hypot(Math.max(x, window.innerWidth - x), Math.max(y, window.innerHeight - y))
    root.style.setProperty("--theme-transition-x", `${x}px`)
    root.style.setProperty("--theme-transition-y", `${y}px`)
    root.style.setProperty("--theme-transition-radius", `${radius}px`)
    root.dataset.themeReveal = nextThemeRevealRef.current
    root.classList.add("theme-transition")
    const cleanup = () => {
      root.classList.remove("theme-transition")
      delete root.dataset.themeReveal
      root.style.removeProperty("--theme-transition-x")
      root.style.removeProperty("--theme-transition-y")
      root.style.removeProperty("--theme-transition-radius")
    }
    try {
      const transition = document.startViewTransition(() => flushSync(() => applyTheme(next)))
      transition.finished.then(cleanup, cleanup)
      transition.ready.then(() => {
        nextThemeRevealRef.current = nextThemeRevealRef.current === "expand" ? "contract" : "expand"
      }, () => undefined)
    } catch {
      cleanup()
      applyTheme(next)
    }
  }, [applyTheme, theme])

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.repeat || event.key.toLowerCase() !== "d") return
      const target = event.target
      if (target instanceof HTMLElement && (target.isContentEditable || ["input", "textarea", "select"].includes(target.tagName.toLowerCase()))) return
      event.preventDefault()
      toggleTheme()
    }
    window.addEventListener("keydown", onKeyDown)
    return () => window.removeEventListener("keydown", onKeyDown)
  }, [toggleTheme])

  useEffect(() => {
    deviceLoginRef.current = deviceLogin
  }, [deviceLogin])

  useEffect(() => {
    let unlisten: (() => void) | undefined
    void subscribeDeviceLogin((event: DeviceLoginEvent) => {
      setDeviceLogin((current) => {
        if (!current || current.id !== event.id) return current
        return event.output ? { ...current, output: [...current.output, event.output].slice(-12) } : current
      })
      const activeLogin = deviceLoginRef.current
      if (!event.completed || !activeLogin || event.id !== activeLogin.id) return
      setBusy(false)
      if (event.error) setDialogError(event.error)
      else {
        void refresh()
        setDialogProfile(undefined)
        deviceLoginRef.current = null
        setDeviceLogin(null)
      }
    }).then((stop) => { unlisten = stop })
    return () => unlisten?.()
  }, [refresh])

  const orderedProfiles = useMemo(() => orderProfilesByPlan(profiles), [profiles])

  const chatGptProfiles = useMemo(
    () => profiles.filter((profile) => profile.authMode.toLowerCase() === "chatgpt"),
    [profiles],
  )

  async function handleSave(name: string, authJson: string | undefined, details: ProfileDetails) {
    setBusy(true)
    setDialogError(null)
    try {
      if (dialogProfile) await updateProfile(dialogProfile.id, name, authJson, details)
      else if (authJson) await addProfile({ name, authJson, ...details })
      await refresh()
      setDialogProfile(undefined)
    } catch (error) {
      setDialogError(error instanceof Error ? error.message : String(error))
    } finally {
      setBusy(false)
    }
  }

  async function handleImport(name: string, details: ProfileDetails) {
    setBusy(true)
    setDialogError(null)
    try {
      await importCurrentProfile(name, details)
      await refresh()
      setDialogProfile(undefined)
    } catch (error) {
      setDialogError(error instanceof Error ? error.message : String(error))
    } finally {
      setBusy(false)
    }
  }

  async function handleDeviceLogin(name: string, details: ProfileDetails) {
    setBusy(true)
    setDialogError(null)
    try {
      const id = await beginDeviceLogin(name, details, dialogProfile?.id)
      const login = { id, output: [] }
      deviceLoginRef.current = login
      setDeviceLogin(login)
    } catch (error) {
      setDialogError(error instanceof Error ? error.message : String(error))
      setBusy(false)
    }
  }

  async function closeProfileDialog() {
    const activeLogin = deviceLoginRef.current
    deviceLoginRef.current = null
    setDeviceLogin(null)
    setBusy(false)
    setDialogProfile(undefined)
    if (activeLogin) {
      try {
        await cancelDeviceLogin(activeLogin.id)
      } catch {
        // The login may already have completed; the dialog is still safely closed.
      }
    }
  }

  async function handleLaunch(profile: Profile, target: LaunchTarget) {
    setLaunchPreparationError(null)
    try {
      const environment = launchEnvironment ?? await getLaunchEnvironment()
      setLaunchEnvironment(environment)
      const targets = await getLaunchTargets()
      setLaunchTargets(targets)
      if (target === 'vscode' && !targets.vscodeInstalled) throw new Error("VS Code was not found. Install it or set its executable path in Launch settings.")
      if (target === 'standalone' && (!targets.standaloneInstalled || !targets.standaloneVerified)) throw new Error("The verified Codex desktop app is unavailable. Reopen Launch settings to check the installed apps.")
      if (target === 'cli' && !targets.codexCliAvailable) throw new Error("Codex CLI was not found. Set its executable path in Launch settings.")
      const settings = await getExecutableSettings()
      setLaunchMode(settings.launchMode ?? 'vscode')
      setHideDesktopPicker(environment.capabilities.backend === "macos" || Boolean(settings.hideDesktopPicker))
      setFolderRequest({ profile, target, initialPath: settings.preferredWorkspace || environment.defaultWorkspace })
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error)
      setLaunchPreparationError(message)
      setProfiles((current) => current.map((item) => item.id === profile.id ? { ...item, status: "error", error: message } : item))
    }
    await refresh()
  }

  async function completeFolderSelection(workspace: string) {
    if (!folderRequest) return
    const request = folderRequest
    setFolderRequest(null)
    setLaunchError(null)
    setPlacementToken(null)
    const launch = { profile: request.profile, target: request.target, workspace }
    setLaunchRequest(launch)
    if (hideDesktopPicker || request.target === "cli") await completeLaunch(null, launch)
  }

  function cancelLaunch() {
    if (launchLock.current) return
    if (placementToken) void discardPlacement(placementToken).catch(() => undefined)
    setLaunchRequest(null)
    setPlacementToken(null)
    setLaunchError(null)
  }

  async function completeLaunch(desktop: string | null, request = launchRequest) {
    if (!request || launchLock.current) return
    launchLock.current = true
    setLaunchBusy(true)
    setLaunchError(null)
    try {
      const result = request.target === 'cli' ? await launchCliProfile(request.profile.id, request.workspace) : await (request.target === 'standalone' ? launchStandaloneProfile : launchProfile)(request.profile.id, request.workspace, desktop, placementToken)
      if (result.completed) {
        setLaunchRequest(null)
        setPlacementToken(null)
      } else {
        setPlacementToken(result.retryToken)
        setLaunchError(result.error ?? "Could not verify desktop placement")
      }
    } catch (error) {
      setLaunchError(error instanceof Error ? error.message : String(error))
    } finally {
      launchLock.current = false
      setLaunchBusy(false)
      await refresh()
    }
  }

  async function handleDelete() {
    if (!deleteTarget) return
    setBusy(true)
    setDialogError(null)
    setDeviceLogin(null)
    try {
      await deleteProfile(deleteTarget.id)
      await refresh()
      await refreshStorage()
      setDeleteTarget(null)
    } catch (error) {
      setDialogError(error instanceof Error ? error.message : String(error))
    } finally {
      setBusy(false)
    }
  }

  async function handleClearCache() {
    if (!cacheTarget || cacheBusy) return
    setCacheBusy(true)
    setCacheError(null)
    try {
      await clearProfileCache(cacheTarget.id)
      await refreshStorage()
      setCacheTarget(null)
    } catch (error) {
      setCacheError(error instanceof Error ? error.message : String(error))
    } finally {
      setCacheBusy(false)
    }
  }

  function openAdd() {
    setDialogError(null)
    setDialogProfile(null)
  }

  function dismissIntegration() {
    if (integrationStatus) {
      localStorage.setItem("multi-codex-desktop-dismissed", integrationStatus.version)
    }
    setIntegrationError(null)
    setShowIntegration(false)
  }

  async function handleInstallIntegration(createDesktopShortcut: boolean) {
    setIntegrationBusy(true)
    setIntegrationError(null)
    try {
      const status = await installDesktopIntegration(createDesktopShortcut)
      setIntegrationStatus(status)
      localStorage.removeItem("multi-codex-desktop-dismissed")
      setShowIntegration(false)
    } catch (error) {
      setIntegrationError(error instanceof Error ? error.message : String(error))
    } finally {
      setIntegrationBusy(false)
    }
  }

  if (!loading && !pageError && profiles.length === 0 && welcomeReady && welcomeNeeded) return <WelcomeScreen platform={launchTargets?.platform} busy={welcomeBusy} error={welcomeError} onContinue={() => void completeWelcome()} />

  return (
    <main className="app-shell">
      <ResetExpiryReminder profiles={profiles} checks={limitChecks} ready={!loading && !pageError && !refreshingAllLimits} onView={group => setCreditsTarget({ profile: group.profile, limits: group.limits })} />
      <header className="topbar">
        <div className="topbar-inner">
          <div className="brand-block">
            <BrandMark />
            <div>
              <strong>Multi Codex</strong>
              <span>Isolated account launcher</span>
            </div>
          </div>
          <div className="topbar-actions">
            <ThemePicker value={colorTheme} onChange={setColorTheme} />
            <button className="button secondary" type="button" onClick={() => setShowLaunchSettings(true)}>Launch settings</button>
            <button className="storage-status" type="button" title="View storage used by isolated Multi Codex profiles" aria-label={storageUsage == null ? "Calculating app storage" : `${formatStorage(storageUsage.bytes)} used by Multi Codex. View breakdown`} onClick={() => { setShowStorage(true); void refreshStorage() }}>
              <HugeiconsIcon icon={HardDriveIcon} size={18} strokeWidth={1.8} />
              <span>{storageUsage == null ? "Calculating" : formatStorage(storageUsage.bytes)}</span>
            </button>
            {integrationStatus?.available && !integrationStatus.installed ? (
              <button
                className="icon-button integration-button"
                type="button"
                title="Install desktop integration"
                aria-label="Install desktop integration"
                onClick={() => { setIntegrationError(null); setShowIntegration(true) }}
              >
                <HugeiconsIcon icon={DownloadSquare01Icon} size={20} strokeWidth={1.8} />
              </button>
            ) : null}
            <button
              className="icon-button"
              type="button"
              title={`Use ${theme === "dark" ? "light" : "dark"} theme`}
              aria-label={`Theme: ${theme}. Use ${theme === "dark" ? "light" : "dark"} theme`}
              onClick={(event) => toggleTheme(event.currentTarget)}
            >
              <HugeiconsIcon icon={theme === "dark" ? Sun03Icon : Moon02Icon} size={21} strokeWidth={1.8} />
            </button>
            <button
              className="button secondary refresh-all-button"
              type="button"
              disabled={refreshingAllLimits || chatGptProfiles.length === 0}
              onClick={() => void handleRefreshAllLimits()}
            >
              <HugeiconsIcon icon={Refresh01Icon} size={18} strokeWidth={1.8} />
              {refreshingAllLimits ? "Refreshing all" : "Refresh all"}
            </button>
          </div>
          <button className="button primary header-button" type="button" onClick={openAdd}>
            <HugeiconsIcon icon={Add01Icon} size={19} strokeWidth={1.8} />
            Add account
          </button>
        </div>
      </header>

      <section className="content-area">
        <div className="content-inner">
          <div className="intro-panel">
            <div>
              <span className="eyebrow">Codex profiles</span>
              <h1>One account per workspace.</h1>
              <p>Choose an account, then open your workspace. Your default login stays untouched.</p>
            </div>
          </div>

          {launchPreparationError ? <p className="form-error" role="alert">{launchPreparationError}</p> : null}

          {launchTargets && rowLaunchTargets.length === 0 ? <div className="state-panel launch-requirements" role="status"><p>{launchTargets.standaloneInstalled ? "Isolated standalone launch is awaiting verification for this installed version. Use VS Code from Launch settings." : "Install VS Code or the supported Codex desktop app to launch your accounts."}</p></div> : null}

          {loading ? (
            <div className="profile-list" aria-label="Loading accounts">
              {[0, 1].map((item) => <div className="profile-row skeleton-row" key={item} />)}
            </div>
          ) : pageError ? (
            <div className="state-panel error-state" role="alert"><h2>Could not load accounts</h2><p>{pageError}</p><button className="button secondary" type="button" onClick={() => void refresh()}>Try again</button></div>
          ) : profiles.length === 0 ? (
            <div className="state-panel empty-state"><BrandMark /><h2>No accounts yet</h2><p>Add an auth JSON or import your current Codex account.</p><button className="button primary" type="button" onClick={openAdd}>Add your first account</button></div>
          ) : (
            <div className="profile-list">
              {orderedProfiles.map((profile) => (
                <ProfileRow
                  key={profile.id}
                  profile={profile}
                  limits={limitChecks[profile.id]}
                  onCheckLimits={handleCheckLimits}
                  onLaunch={handleLaunch}
                  launchTargets={rowLaunchTargets}
                  onEdit={(selected) => { setDialogError(null); setDialogProfile(selected) }}
                  onDelete={(selected) => { setDialogError(null); setDeleteTarget(selected) }}
                  onShowResetCredits={(selected, limits) => setCreditsTarget({ profile: selected, limits })}
                />
              ))}
            </div>
          )}

          <footer>Credentials stay in your system keyring. Multi Codex never edits your default auth file.</footer>
        </div>
      </section>

      {dialogProfile !== undefined ? (
        <ProfileDialog
          profile={dialogProfile}
          busy={busy}
          error={dialogError}
          loginOutput={deviceLogin?.output.join("\n")}
          deviceLoginActive={Boolean(deviceLogin)}
          onClose={() => void closeProfileDialog()}
          onSave={handleSave}
          onImportCurrent={handleImport}
          onDeviceLogin={handleDeviceLogin}
        />
      ) : null}
      {deleteTarget ? (
        <ConfirmDialog
          profile={deleteTarget}
          busy={busy}
          error={dialogError}
          onCancel={() => setDeleteTarget(null)}
          onConfirm={() => void handleDelete()}
        />
      ) : null}
      {showIntegration ? (
        <DesktopIntegrationDialog
          busy={integrationBusy}
          error={integrationError}
          onDismiss={dismissIntegration}
          onInstall={handleInstallIntegration}
        />
      ) : null}
      {showLaunchSettings ? <LaunchSettingsDialog onClose={() => {
        setShowLaunchSettings(false)
        void Promise.all([getExecutableSettings(), getLaunchTargets()]).then(([settings, apps]) => {
          setLaunchMode(settings.launchMode ?? 'vscode'); setLaunchTargets(apps)
        }).catch(error => setLaunchPreparationError(String(error)))
      }} /> : null}
      {launchRequest ? (
        <WorkspaceDialog
          showDesktopPicker={!hideDesktopPicker && launchRequest.target !== "cli"}
          appName={launchRequest.target === 'cli' ? 'CLI' : launchRequest.target === 'standalone' ? 'Codex' : 'VS Code'}
          profile={launchRequest.profile}
          workspace={launchRequest.workspace}
          busy={launchBusy}
          error={launchError}
          alreadyOpened={placementToken !== null}
          onCancel={cancelLaunch}
          onChoose={(desktop) => void completeLaunch(desktop)}
        />
      ) : null}
      {folderRequest ? (
        <WorkspacePickerDialog
          initialPath={folderRequest.initialPath}
          busy={false}
          onCancel={() => setFolderRequest(null)}
          onChoose={(path) => void completeFolderSelection(path)}
        />
      ) : null}
      {showStorage ? (
        <StorageDialog
          usage={storageUsage}
          loading={storageLoading}
          error={storageError}
          profiles={profiles}
          onClose={() => setShowStorage(false)}
          onRefresh={() => void refreshStorage()}
          onClear={(profile) => { setCacheError(null); setCacheTarget(profile) }}
          onDelete={(profile) => { setShowStorage(false); setDialogError(null); setDeleteTarget(profile) }}
        />
      ) : null}
      {cacheTarget ? (
        <CacheConfirmDialog
          profile={cacheTarget}
          busy={cacheBusy}
          error={cacheError}
          onCancel={() => { setCacheError(null); setCacheTarget(null) }}
          onConfirm={() => void handleClearCache()}
        />
      ) : null}
      {creditsTarget ? (
        <ResetCreditsDialog profileName={creditsTarget.profile.name} limits={creditsTarget.limits} onClose={() => setCreditsTarget(null)} />
      ) : null}
    </main>
  )
}

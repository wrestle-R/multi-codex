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
import { DesktopIntegrationDialog } from "./components/desktop-integration-dialog"
import { ProfileDialog } from "./components/profile-dialog"
import { ProfileRow } from "./components/profile-row"
import { ResetCreditsDialog } from "./components/reset-credits-dialog"
import { StorageDialog } from "./components/storage-dialog"
import { WorkspaceDialog } from "./components/workspace-dialog"
import {
  addProfile,
  beginDeviceLogin,
  cancelDeviceLogin,
  checkProfileLimits,
  chooseWorkspace,
  clearProfileCache,
  deleteProfile,
  getDesktopIntegrationStatus,
  getLaunchEnvironment,
  getStorageUsage,
  importCurrentProfile,
  installDesktopIntegration,
  launchProfile,
  listProfiles,
  subscribeDeviceLogin,
  updateProfile,
} from "./lib/desktop-api"
import type { DesktopIntegrationStatus, DeviceLoginEvent, LaunchEnvironment, LimitCheckState, Profile, ProfileDetails, ProfileLimits, ProfileStorageUsage, StorageUsage } from "./lib/types"
import { formatStorage } from "./lib/formatters"

export { formatStorage } from "./lib/formatters"

type Theme = "light" | "dark"

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
  const [launchRequest, setLaunchRequest] = useState<{ profile: Profile; workspace: string } | null>(null)
  const [launchBusy, setLaunchBusy] = useState(false)
  const [creditsTarget, setCreditsTarget] = useState<{ profile: Profile; limits: ProfileLimits } | null>(null)
  const [limitChecks, setLimitChecks] = useState<Record<string, LimitCheckState>>({})
  const [refreshingAllLimits, setRefreshingAllLimits] = useState(false)
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
    setTheme(next)
    document.documentElement.classList.toggle("dark", next === "dark")
    document.documentElement.dataset.theme = next
    localStorage.setItem("multi-codex-theme", next)
  }, [])

  useEffect(() => { applyTheme(theme) }, [applyTheme, theme])

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

  async function handleLaunch(profile: Profile) {
    try {
      const environment = launchEnvironment ?? await getLaunchEnvironment()
      setLaunchEnvironment(environment)
      const workspace = await chooseWorkspace(environment.defaultWorkspace)
      if (!workspace) return
      if (environment.hyprland) {
        setLaunchRequest({ profile, workspace })
        return
      }
      await launchProfile(profile.id, workspace, null)
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error)
      setProfiles((current) => current.map((item) => item.id === profile.id ? { ...item, status: "error", error: message } : item))
    }
    await refresh()
  }

  async function completeLaunch(desktop: number | null) {
    if (!launchRequest || launchBusy) return
    const request = launchRequest
    setLaunchBusy(true)
    setLaunchRequest(null)
    try {
      await launchProfile(request.profile.id, request.workspace, desktop)
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error)
      setProfiles((current) => current.map((item) => item.id === request.profile.id ? { ...item, status: "error", error: message } : item))
    } finally {
      setLaunchBusy(false)
      await refresh()
    }
  }

  async function handleCheckLimits(profile: Profile) {
    if (limitChecks[profile.id]?.loading) return
    setLimitChecks((current) => ({
      ...current,
      [profile.id]: { ...current[profile.id], loading: true, error: undefined },
    }))
    try {
      const data = await checkProfileLimits(profile.id)
      setLimitChecks((current) => ({ ...current, [profile.id]: { loading: false, data } }))
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error)
      setLimitChecks((current) => ({
        ...current,
        [profile.id]: { ...current[profile.id], loading: false, error: message },
      }))
    }
  }

  async function handleRefreshAllLimits() {
    if (refreshingAllLimits || chatGptProfiles.length === 0) return
    setRefreshingAllLimits(true)
    try {
      await Promise.all(chatGptProfiles.map((profile) => handleCheckLimits(profile)))
    } finally {
      setRefreshingAllLimits(false)
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

  return (
    <main className="app-shell">
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
            <button className="button primary header-button" type="button" onClick={openAdd}>
              <HugeiconsIcon icon={Add01Icon} size={19} strokeWidth={1.8} />
              Add account
            </button>
          </div>
        </div>
      </header>

      <section className="content-area">
        <div className="content-inner">
          <div className="intro-panel">
            <div>
              <span className="eyebrow">Codex profiles</span>
              <h1>One account per workspace.</h1>
              <p>Open separate VS Code windows without changing your main Codex login.</p>
            </div>
          </div>

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
              {profiles.map((profile) => (
                <ProfileRow
                  key={profile.id}
                  profile={profile}
                  limits={limitChecks[profile.id]}
                  onCheckLimits={handleCheckLimits}
                  onLaunch={handleLaunch}
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
      {launchRequest ? (
        <WorkspaceDialog
          profile={launchRequest.profile}
          workspace={launchRequest.workspace}
          busy={launchBusy}
          onCancel={() => setLaunchRequest(null)}
          onChoose={(desktop) => void completeLaunch(desktop)}
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

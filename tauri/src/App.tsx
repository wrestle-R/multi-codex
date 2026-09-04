import { useCallback, useEffect, useMemo, useState } from "react"
import {
  Add01Icon,
  ComputerIcon,
  Moon02Icon,
  ShieldKeyIcon,
  Sun03Icon,
} from "@hugeicons/core-free-icons"
import { HugeiconsIcon } from "@hugeicons/react"
import "./App.css"
import { BrandMark } from "./components/brand-mark"
import { ConfirmDialog } from "./components/confirm-dialog"
import { ProfileDialog } from "./components/profile-dialog"
import { ProfileRow } from "./components/profile-row"
import {
  addProfile,
  deleteProfile,
  importCurrentProfile,
  launchProfile,
  listProfiles,
  updateProfile,
} from "./lib/desktop-api"
import type { Profile } from "./lib/types"

type Theme = "system" | "light" | "dark"

function initialTheme(): Theme {
  const saved = localStorage.getItem("multi-codex-theme")
  if (saved === "system" || saved === "light" || saved === "dark") return saved
  return "system"
}

const nextTheme: Record<Theme, Theme> = { system: "light", light: "dark", dark: "system" }

export default function App() {
  const [profiles, setProfiles] = useState<Profile[]>([])
  const [loading, setLoading] = useState(true)
  const [pageError, setPageError] = useState<string | null>(null)
  const [dialogProfile, setDialogProfile] = useState<Profile | null | undefined>(undefined)
  const [deleteTarget, setDeleteTarget] = useState<Profile | null>(null)
  const [dialogError, setDialogError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const [theme, setTheme] = useState<Theme>(initialTheme)

  const refresh = useCallback(async () => {
    try {
      const next = await listProfiles()
      setProfiles(next)
      setPageError(null)
    } catch (error) {
      setPageError(error instanceof Error ? error.message : String(error))
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    void refresh()
    const interval = window.setInterval(() => void refresh(), 2000)
    return () => window.clearInterval(interval)
  }, [refresh])

  useEffect(() => {
    const media = window.matchMedia("(prefers-color-scheme: dark)")
    const apply = () => document.documentElement.classList.toggle("dark", theme === "dark" || (theme === "system" && media.matches))
    apply()
    localStorage.setItem("multi-codex-theme", theme)
    media.addEventListener("change", apply)
    return () => media.removeEventListener("change", apply)
  }, [theme])

  const runningCount = useMemo(
    () => profiles.filter((profile) => profile.status === "running" || profile.status === "launching").length,
    [profiles],
  )

  async function handleSave(name: string, authJson?: string) {
    setBusy(true)
    setDialogError(null)
    try {
      if (dialogProfile) await updateProfile(dialogProfile.id, name, authJson)
      else if (authJson) await addProfile({ name, authJson })
      await refresh()
      setDialogProfile(undefined)
    } catch (error) {
      setDialogError(error instanceof Error ? error.message : String(error))
    } finally {
      setBusy(false)
    }
  }

  async function handleImport(name: string) {
    setBusy(true)
    setDialogError(null)
    try {
      await importCurrentProfile(name)
      await refresh()
      setDialogProfile(undefined)
    } catch (error) {
      setDialogError(error instanceof Error ? error.message : String(error))
    } finally {
      setBusy(false)
    }
  }

  async function handleLaunch(profile: Profile) {
    setProfiles((current) => current.map((item) => item.id === profile.id ? { ...item, status: "launching", error: null } : item))
    try {
      await launchProfile(profile.id)
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error)
      setProfiles((current) => current.map((item) => item.id === profile.id ? { ...item, status: "error", error: message } : item))
    }
    await refresh()
  }

  async function handleDelete() {
    if (!deleteTarget) return
    setBusy(true)
    setDialogError(null)
    try {
      await deleteProfile(deleteTarget.id)
      await refresh()
      setDeleteTarget(null)
    } catch (error) {
      setDialogError(error instanceof Error ? error.message : String(error))
    } finally {
      setBusy(false)
    }
  }

  function openAdd() {
    setDialogError(null)
    setDialogProfile(null)
  }

  return (
    <main className="app-shell">
      <header className="topbar">
        <div className="brand-block">
          <BrandMark />
          <div>
            <strong>Multi Codex</strong>
            <span>Isolated account launcher</span>
          </div>
        </div>
        <div className="topbar-actions">
          <button
            className="icon-button"
            type="button"
            title={`Theme: ${theme}. Use ${nextTheme[theme]} theme`}
            aria-label={`Theme: ${theme}. Use ${nextTheme[theme]} theme`}
            onClick={() => setTheme((current) => nextTheme[current])}
          >
            <HugeiconsIcon icon={theme === "system" ? ComputerIcon : theme === "dark" ? Sun03Icon : Moon02Icon} size={21} strokeWidth={1.8} />
          </button>
          <button className="button primary header-button" type="button" onClick={openAdd}>
            <HugeiconsIcon icon={Add01Icon} size={19} strokeWidth={1.8} />
            Add account
          </button>
        </div>
      </header>

      <section className="content-area">
        <div className="intro-panel">
          <div>
            <span className="eyebrow">Codex profiles</span>
            <h1>One account per workspace.</h1>
            <p>Open separate VS Code windows without changing your main Codex login.</p>
          </div>
          <div className="runtime-summary">
            <HugeiconsIcon icon={ShieldKeyIcon} size={24} strokeWidth={1.7} />
            <div><span>Protected locally</span><strong>{runningCount} running</strong></div>
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
                onLaunch={handleLaunch}
                onEdit={(selected) => { setDialogError(null); setDialogProfile(selected) }}
                onDelete={(selected) => { setDialogError(null); setDeleteTarget(selected) }}
              />
            ))}
          </div>
        )}

        <footer>Credentials stay in your system keyring. Multi Codex never edits your default auth file.</footer>
      </section>

      {dialogProfile !== undefined ? (
        <ProfileDialog
          profile={dialogProfile}
          busy={busy}
          error={dialogError}
          onClose={() => setDialogProfile(undefined)}
          onSave={handleSave}
          onImportCurrent={handleImport}
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
    </main>
  )
}

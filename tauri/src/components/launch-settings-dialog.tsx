import { useEffect, useId, useState } from "react"
import { getExecutableSettings, getLaunchEnvironment, getLaunchTargets, getTerminalOptions, saveExecutableSettings } from "../lib/desktop-api"
import type { ExecutableSettings, LaunchTargets, TerminalId, TerminalOption } from "../lib/types"
import { useDialogFocus } from "./use-dialog-focus"
import { WorkspacePickerDialog } from "./workspace-picker-dialog"

export function LaunchSettingsDialog({ onClose }: { onClose: () => void }) {
  const titleId = useId()
  const [settings, setSettings] = useState<ExecutableSettings>({ codePath: null, codexPath: null, globalCodexHome: null })
  const [targets, setTargets] = useState<LaunchTargets | null>(null)
  const [terminals, setTerminals] = useState<TerminalOption[]>([])
  const [defaultFolder, setDefaultFolder] = useState("")
  const [loading, setLoading] = useState(true)
  const [busy, setBusy] = useState(false)
  const [browsing, setBrowsing] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const dialogRef = useDialogFocus(onClose, busy, !browsing)
  useEffect(() => {
    let active = true
    void Promise.all([getExecutableSettings(), getLaunchTargets(), getLaunchEnvironment(), getTerminalOptions()])
      .then(([value, apps, environment, terminalOptions]) => {
        if (!active) return
        setSettings(value)
        setTargets(apps)
        setTerminals(terminalOptions)
        setDefaultFolder(environment.defaultWorkspace)
      })
      .catch(error => { if (active) setError(String(error)) })
      .finally(() => { if (active) setLoading(false) })
    return () => { active = false }
  }, [])
  async function save(event: React.FormEvent) {
    event.preventDefault()
    setBusy(true)
    setError(null)
    try {
      await saveExecutableSettings(settings)
      onClose()
    } catch (error) {
      setError(error instanceof Error ? error.message : String(error))
    } finally { setBusy(false) }
  }
  if (browsing) return <WorkspacePickerDialog purpose="preference" initialPath={settings.preferredWorkspace || defaultFolder} busy={false} onCancel={() => setBrowsing(false)} onChoose={path => { setSettings(value => ({ ...value, preferredWorkspace: path })); setBrowsing(false) }} />
  return (
    <div className="dialog-layer" role="presentation" onMouseDown={event => event.target === event.currentTarget && !busy && onClose()}>
      <section ref={dialogRef} className="dialog launch-settings-dialog" role="dialog" aria-modal="true" aria-labelledby={titleId}>
        <span className="eyebrow">Launch settings</span>
        <h2 id={titleId}>Folders and launching</h2>
        <p className="settings-intro">A few preferences, just for you.</p>
        <form onSubmit={event => void save(event)} className="launch-settings-form">
          <div className="preferred-folder-control">
            <label>Preferred folder<input value={settings.preferredWorkspace ?? ""} disabled={loading || busy} placeholder={defaultFolder || "Default workspace folder"} onChange={event => setSettings(value => ({ ...value, preferredWorkspace: event.target.value || null }))} /></label>
            <button type="button" className="button secondary" disabled={loading || busy || !defaultFolder} onClick={() => setBrowsing(true)}>Browse</button>
          </div>
          <p className="desktop-notice">The folder picker will start here.</p>
          {targets ? <fieldset className="launch-app-options">
            <legend>Open accounts with</legend>
            <div className="launch-app-choices">
              {([['vscode', 'VS Code'], ['standalone', 'Codex'], ['both', 'All available'], ['cli', 'CLI only']] as const).map(([mode, label]) => <label key={mode}><input type="radio" name="launch-app" checked={(settings.launchMode ?? 'vscode') === mode} disabled={loading || busy || (mode === 'standalone' && !targets.standaloneVerified) || (mode === 'cli' && !targets.codexCliAvailable) || (mode === 'vscode' && !targets.vscodeInstalled)} onChange={() => setSettings(value => ({ ...value, launchMode: mode }))} /> {label}</label>)}
            </div>
            <p className="desktop-notice">{!targets.standaloneInstalled || targets.standaloneVerified ? "CLI appears alongside your chosen apps when installed. Each uses your isolated account." : `Codex ${targets.standaloneVersion ?? 'version'} has not passed account-isolation verification. Choose an available option below.`}</p>
          </fieldset> : null}
          <label>CLI terminal
            <select value={settings.cliTerminal ?? "automatic"} disabled={loading || busy} onChange={event => setSettings(value => ({ ...value, cliTerminal: event.target.value as TerminalId }))}>
              {terminals.map(terminal => <option key={terminal.id} value={terminal.id} disabled={!terminal.available && terminal.id !== (settings.cliTerminal ?? "automatic")}>{terminal.label}{terminal.available ? "" : " (not installed)"}</option>)}
              {settings.cliTerminal && !terminals.some(terminal => terminal.id === settings.cliTerminal) ? <option value={settings.cliTerminal}>{settings.cliTerminal} (unavailable on this platform)</option> : null}
            </select>
          </label>
          <p className="desktop-notice">Choose an installed terminal for CLI launches. Uses your selected account’s saved sign-in. Automatic uses the first supported terminal found.</p>
          {targets?.platform === "linux" ? <label className="desktop-picker-setting"><input type="checkbox" checked={!(settings.hideDesktopPicker ?? false)} disabled={loading || busy} onChange={event => setSettings(value => ({ ...value, hideDesktopPicker: !event.target.checked }))} aria-label="Show desktop picker before launching" /><span className="desktop-setting-copy">Show desktop picker<small>Choose a desktop before launching.</small></span><span className="desktop-switch-track" aria-hidden="true" /></label> : null}
          <details className="advanced-settings">
            <summary>Advanced <span>Executable paths and Codex home</span></summary>
            <div className="advanced-settings-fields">
              <p className="desktop-notice">Leave paths empty for automatic discovery. Use full paths without quotes or command arguments.</p>
              <label>VS Code executable<input value={settings.codePath ?? ""} disabled={loading || busy} placeholder="Automatic discovery" onChange={event => setSettings(value => ({ ...value, codePath: event.target.value || null }))} /></label>
              <label>Codex executable<input value={settings.codexPath ?? ""} disabled={loading || busy} placeholder="Automatic discovery" onChange={event => setSettings(value => ({ ...value, codexPath: event.target.value || null }))} /></label>
              <label>Global Codex home<input value={settings.globalCodexHome ?? ""} disabled={loading || busy} placeholder="CODEX_HOME or ~/.codex" onChange={event => setSettings(value => ({ ...value, globalCodexHome: event.target.value || null }))} /></label>
              <p className="desktop-notice">Executable changes apply to the next launch. Restart after changing the global home. An inherited CODEX_HOME takes precedence. Existing profiles stay in their current location.</p>
            </div>
          </details>
          {error ? <p className="form-error" role="alert">{error}</p> : null}
          <div className="dialog-actions"><button type="button" className="button secondary" disabled={busy} onClick={onClose}>Cancel</button><button type="submit" className="button primary" disabled={loading || busy || !targets}>{busy ? "Saving…" : "Save settings"}</button></div>
        </form>
      </section>
    </div>
  )
}

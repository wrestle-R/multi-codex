"use client"
import { useState } from "react"
import { installCommand, version, windowsInstallerDownload } from "@/lib/site"

export function InstallBox({ update = false }: { update?: boolean }) {
  const [platform, setPlatform] = useState<"linux" | "mac" | "windows">("linux")
  const [copied, setCopied] = useState(false)
  const [error, setError] = useState(false)
  const command = installCommand(platform, update)
  async function copy() {
    try { await navigator.clipboard.writeText(command); setCopied(true); setError(false) }
    catch { setError(true) }
  }
  return <div className="install-box"><div className="install-toolbar"><div className="platform-tabs" role="group" aria-label="Installation platform">
    {(["linux", "mac", "windows"] as const).map(value => <button type="button" key={value} aria-pressed={platform === value} onClick={() => { setPlatform(value); setCopied(false); setError(false) }}>{value === "linux" ? "Linux" : value === "mac" ? "macOS" : "Windows"}</button>)}
  </div>{platform !== "windows" ? <button className="copy-button" type="button" onClick={() => void copy()}>{copied ? "Copied ✓" : "Copy command"}</button> : null}</div>
    {platform === "windows" ? <>
      <a className="button primary" href={windowsInstallerDownload}>Download Windows .exe <span aria-hidden="true">↓</span></a>
      <p className="windows-install-note">{update ? "Close Multi Codex, then run the downloaded installer. Your saved accounts and conversations stay in place." : "Download the installer, double-click it and follow setup. Then open Multi Codex from the Start menu."}</p>
      <p className="install-caption">Windows x64 · v{version} · Per-user installer</p>
      <details className="windows-command"><summary>{update ? "Update with PowerShell instead" : "Install with PowerShell instead"}</summary>
        <div className="windows-command-toolbar"><button className="copy-button" type="button" onClick={() => void copy()}>{copied ? "Copied ✓" : "Copy command"}</button></div>
        <pre><code>{command}</code></pre>
      </details>
    </> : <>
      <pre><code>{command}</code></pre>
      <p className="install-caption">{platform === "linux" ? "Linux x86_64 · AppImage" : "Apple Silicon · macOS 26+ · Unsigned build, explicit opt-in"}</p>
    </>}
    {platform === "mac" ? <p className="mac-note">If macOS blocks the first open, use System Settings → Privacy & Security → Open Anyway. The installer preserves Gatekeeper protections.</p> : null}
    <span className="sr-only" role="status">{copied ? "Command copied to clipboard." : ""}</span>
    {error ? <p role="alert">Clipboard access is unavailable. Select and copy the command above.</p> : null}
  </div>
}

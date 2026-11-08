import type { Metadata } from "next"
import { headers } from "next/headers"
import Link from "next/link"
import { detectInstallPlatform, isInstallPlatform } from "@/lib/install-platform"
import { linuxInstallerDownload, macInstallerDownload, macVersion, version, windowsInstallerDownload } from "@/lib/site"

export const metadata: Metadata = {
  title: "Install",
  description: "One install link for Multi Codex on Linux, Apple Silicon Mac and Windows. Get the right download and setup steps for your computer.",
  alternates: { canonical: "https://multi-codex.vercel.app/install" },
}

const downloads = {
  windows: { label: "Windows x64", format: ".exe installer", url: windowsInstallerDownload, version },
  mac: { label: "Apple Silicon Mac", format: ".dmg installer", url: macInstallerDownload, version: macVersion },
  linux: { label: "Linux x64", format: ".AppImage", url: linuxInstallerDownload, version },
} as const

export default async function InstallPage({ searchParams }: { searchParams: Promise<{ platform?: string | string[] }> }) {
  const [requestHeaders, query] = await Promise.all([headers(), searchParams])
  const detected = detectInstallPlatform(requestHeaders)
  const chosen = isInstallPlatform(query.platform) ? query.platform : null
  const platform = chosen || detected.platform
  const download = platform ? downloads[platform] : null
  return <main id="main" className="install-page page-width">
    <p className="eyebrow">MULTI CODEX DESKTOP</p>
    <h1>One link.<br />Your computer.</h1>
    <p className="install-intro">Get the latest stable launcher and keep each account in its own workspace.</p>
    <p className="install-detection">{chosen ? "Downloading for the computer you selected." : detected.message}</p>
    <form action="/install" method="get" className="install-device-form">
      <label htmlFor="install-platform">Install on</label>
      <select id="install-platform" name="platform" defaultValue={platform || ""} required>
        <option value="" disabled>Choose a computer</option>
        <option value="windows">Windows x64</option>
        <option value="mac">Mac · Apple Silicon, macOS 26+</option>
        <option value="linux">Linux x64</option>
      </select>
      <button className="button secondary" type="submit">Use this device</button>
    </form>
    {download && platform ? <section className="install-download" aria-labelledby="download-title">
      <p className="eyebrow">STABLE v{download.version} · {download.format}</p>
      <h2 id="download-title">Install on {download.label}.</h2>
      {platform === "mac" ? <p>Requires an Apple Silicon Mac with macOS 26 or newer. Check <strong>Apple menu → About This Mac</strong> first; Intel Macs aren’t supported.</p> : <p>Requires a supported 64-bit {platform === "windows" ? "Windows" : "Linux"} computer with an Intel or AMD processor.</p>}
      <a className="button primary" href={download.url}>Download for {download.label} <span aria-hidden="true">↓</span></a>
      <ol className="install-steps">
        {platform === "windows" ? <>
          <li>Open the downloaded .exe and follow setup.</li>
          <li>Open Multi Codex from the Start menu, then add an account.</li>
        </> : platform === "mac" ? <>
          <li>Open the .dmg and drag Multi Codex into Applications.</li>
          <li>Open Multi Codex from Applications. If macOS blocks this unsigned build, open <strong>System Settings → Privacy &amp; Security → Open Anyway</strong> after the first attempt.</li>
          <li>Add an account. The Mac launcher includes Codex, VS Code, CLI and Orbit; T3 launching is available on Linux and Windows.</li>
        </> : <>
          <li>Keep the AppImage in a permanent folder, such as <strong>Applications</strong> in your home folder.</li>
          <li>In your file manager’s Properties or Permissions, allow the file to run as a program. Then open it.</li>
          <li>Add an account. If your desktop cannot open AppImages, follow the Linux setup guide below.</li>
        </>}
      </ol>
      {platform === "linux" && <details className="install-fallback"><summary>AppImage won’t open?</summary><p>Open a terminal in the folder containing the download and run:</p><pre><code>{`chmod +x Multi.Codex_${version}_amd64.AppImage\n./Multi.Codex_${version}_amd64.AppImage --appimage-extract-and-run`}</code></pre><p>This starts the AppImage without requiring a FUSE mount. A working desktop session and the native Linux dependencies are still required.</p></details>}
      <p className="install-caption">Already installed? Use Update in Multi Codex. For a manual update, close the app before running the installer or replacing the AppImage. Saved accounts and chats stay in place.</p>
    </section> : null}
    <p className="install-help"><Link className="text-link" href="/docs/platform-support">Platform requirements and help →</Link></p>
  </main>
}

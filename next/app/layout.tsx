import type { Metadata } from "next"
import localFont from "next/font/local"
import Link from "next/link"
import { latestRelease, repository } from "@/lib/site"
import "./globals.css"

const body = localFont({ src: [{ path: "../public/fonts/outfit-regular.woff2", weight: "400" }, { path: "../public/fonts/outfit-medium.woff2", weight: "500" }], variable: "--font-body", display: "swap" })
const display = localFont({ src: [{ path: "../public/fonts/bricolage-semibold.woff2", weight: "600" }, { path: "../public/fonts/bricolage-bold.woff2", weight: "700" }], variable: "--font-display", display: "swap" })
export const metadata: Metadata = {
  title: { default: "Multi Codex — An account for every workspace", template: "%s · Multi Codex" },
  description: "Launch isolated Codex accounts in VS Code, the Codex app, or your favorite terminal. Open-source launcher for Linux and Apple Silicon Mac.",
  openGraph: { title: "Multi Codex", description: "An account for every workspace.", type: "website" },
}
export default function RootLayout({ children }: { children: React.ReactNode }) {
  return <html lang="en"><body className={`${body.variable} ${display.variable}`}>
    <a href="#main" className="skip-link">Skip to content</a>
    <header className="site-header"><nav className="nav-shell" aria-label="Main navigation">
      <Link className="brand" href="/" aria-label="Multi Codex home"><span className="brand-icon" aria-hidden="true">M<span>+</span></span><span>Multi Codex</span></Link>
      <div className="nav-links"><Link href="/docs">Docs</Link><Link href="/releases">Changelog</Link><a href={repository}>GitHub <span aria-hidden="true">↗</span></a></div>
      <a className="nav-download" href={latestRelease}>Download <span aria-hidden="true">↓</span></a>
    </nav></header>
    {children}
    <footer className="site-footer"><Link className="brand" href="/"><span className="brand-icon small" aria-hidden="true">M<span>+</span></span>Multi Codex</Link><span>Built for the way you work.</span><div><a href={`${repository}/blob/main/LICENSE`}>MIT license</a><a href={repository}>Source ↗</a></div><p>An independent open-source project. Not affiliated with OpenAI.</p></footer>
  </body></html>
}

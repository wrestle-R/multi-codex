import { ThemedScreenshot } from "@/components/themed-screenshot"
import Link from "next/link"
import { InstallBox } from "@/components/install-box"
import { latestRelease, repository, version } from "@/lib/site"
import accountsLight from "@/public/images/accounts-light.png"
import accountsDark from "@/public/images/accounts-dark.png"

export default function Home() {
  return <main id="main">
    <section className="hero page-width"><div className="hero-copy"><Link href="/releases" className="release-pill"><span className="status-dot" />New in v{version}<span aria-hidden="true">↗</span></Link>
      <p className="eyebrow">YOUR ACCOUNTS. YOUR WORKSPACES.</p><h1>A little order<br />for all your<br /><span>Codex accounts.</span></h1>
      <p className="hero-description">Personal projects. Work accounts. One calm place to launch them all, with a separate workspace for every login.</p>
      <div className="hero-actions"><a className="button primary" href={latestRelease}>Get Multi Codex <span aria-hidden="true">↓</span></a><Link className="text-link" href="/docs">Read the docs <span aria-hidden="true">→</span></Link></div>
      <p className="platform-line"><span>Linux</span><span>Apple Silicon Mac</span><span>Open source</span></p>
    </div><div className="hero-visual"><div className="visual-label"><span>ONE LAUNCHER, YOUR WHOLE LINEUP</span><span aria-hidden="true">ACCOUNT DASHBOARD</span></div><div className="screenshot-frame"><ThemedScreenshot light={accountsLight} dark={accountsDark} alt="Multi Codex account dashboard showing separate accounts, usage limits and launch buttons for VS Code, Codex and CLI" priority sizes="(max-width: 900px) 100vw, 60vw" /></div><div className="visual-footnote"><span className="small-mark" aria-hidden="true">↳</span>Everything in its own space.<span className="caption-end">Your default login stays yours.</span></div></div></section>
    <section className="feature-strip page-width" aria-label="Features"><div><span className="feature-number">01</span><h2>Accounts stay separate.</h2><p>Each account has its own saved sign-in, conversations and app data.</p></div><div><span className="feature-number">02</span><h2>Launch your way.</h2><p>VS Code, Codex desktop, or CLI in your favorite installed terminal.</p></div><div><span className="feature-number">03</span><h2>Keep an eye on usage.</h2><p>Limits at a glance, with quiet reminders before your usage resets expire.</p></div></section>
    <section className="getting-started page-width"><div><p className="eyebrow">SMALL SETUP. MORE ROOM TO WORK.</p><h2>Up and running<br />in a few minutes.</h2><p>Install the launcher. Add an account. Choose a folder and make it your own.</p><Link className="text-link" href="/docs#first-account">Follow the quick start <span aria-hidden="true">→</span></Link></div><InstallBox /></section>
    <section className="notes-section page-width"><div><p className="eyebrow">A FEW THOUGHTFUL DETAILS</p><h2>Fits into your day.</h2></div><div className="detail-list"><article><span aria-hidden="true">⌘</span><div><h3>Your terminal, your choice</h3><p>Choose Kitty, Ghostty, Terminal, Konsole and other supported terminals in Launch settings.</p></div></article><article><span aria-hidden="true">◷</span><div><h3>A heads-up before resets expire</h3><p>Available resets expiring within 48 hours appear together. At most twice a day, at least four hours apart.</p></div></article><article><span aria-hidden="true">◐</span><div><h3>Six palettes. Light and dark.</h3><p>Sage, Ocean, Sand, Rose, Plum and Orange. A quieter home for your accounts, whatever the hour.</p></div></article></div></section>
    <section className="source-banner page-width"><div><p className="eyebrow">OPEN SOURCE, THROUGH AND THROUGH</p><h2>Have a look under the hood.</h2></div><a className="button secondary" href={repository}>View on GitHub <span aria-hidden="true">↗</span></a></section>
  </main>
}

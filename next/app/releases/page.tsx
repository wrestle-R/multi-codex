import type { Metadata } from "next"
import { latestRelease, repository, version } from "../../lib/site"
export const metadata: Metadata = { title: "Releases" }
export default function Releases() {
  return <main id="main" className="release-page page-width">
    <p className="eyebrow">A LITTLE BETTER, EVERY RELEASE</p><h1>What’s new.</h1><p className="page-description">Small improvements that make room for your work.</p>
    <article className="release-entry">
      <div className="release-meta"><span className="release-pill"><span className="status-dot" />v{version}</span><time dateTime="2026-10-07">October 7, 2026</time></div>
      <div><h2>A little more room.</h2><ul>
        <li><strong>A cleaner terminal picker.</strong> Automatic and installed terminals appear in a menu that matches your theme, with a checkmark for your selection.</li>
        <li><strong>Settings that breathe.</strong> Workspace and Launching have their own sections. Save and Cancel stay visible while advanced options scroll.</li>
        <li><strong>Keyboard friendly.</strong> Use arrows, Home/End, typing, Enter and Escape to choose a terminal.</li>
        <li><strong>Your preferences stay yours.</strong> Removed terminals are explained without silently changing a saved choice. Account data and existing paths stay intact.</li>
      </ul><div className="release-actions"><a className="button primary" href={latestRelease}>Download latest ↓</a><a className="text-link" href={`${repository}/releases/tag/v${version}`}>Full release notes ↗</a></div></div>
    </article>
    <article className="release-entry"><div className="release-meta"><span>v1.3.7</span><time dateTime="2026-10-07">October 7, 2026</time></div><div><h2>Your terminal. Timely reminders.</h2><p>Choose a CLI terminal, launch a complete CLI with your selected account, and see grouped reminders for resets expiring within 48 hours. The Orange preview matches dark mode.</p><a className="text-link" href={`${repository}/releases/tag/v1.3.7`}>Read the release notes ↗</a></div></article>
    <article className="release-entry"><div className="release-meta"><span>v1.3.6</span><time dateTime="2026-10-07">October 7, 2026</time></div><div><h2>A CLI for every account.</h2><p>Account-row CLI launch, isolated account homes, automatic usage refresh, plan ordering and six color palettes in light and dark.</p><a className="text-link" href={`${repository}/releases/tag/v1.3.6`}>Read the release notes ↗</a></div></article>
    <div className="doc-note"><strong>Release verification</strong><p>Mac/Linux builds and installer checks are tracked separately from broader platform validation. The Mac package remains unsigned and unnotarized.</p><a href={`${repository}/blob/main/docs/releases/v${version}-validation.json`}>View the validation manifest →</a></div>
  </main>
}

import type { Metadata } from "next"
import { releaseHistory } from "../../lib/releases"
import { latestRelease, repository, version } from "../../lib/site"

export const metadata: Metadata = {
  title: "Releases",
  description: "Follow Multi Codex from its first release to today. Every published version, with the improvements it brought.",
}

const releaseDate = new Intl.DateTimeFormat("en-US", { month: "long", day: "numeric", year: "numeric", timeZone: "UTC" })

export default function Releases() {
  return <main id="main" className="release-page page-width">
    <p className="eyebrow">A LITTLE BETTER, EVERY RELEASE</p>
    <h1>What’s new.</h1>
    <p className="page-description">From the first workspace to the one you use today. Every published release, and what it made better.</p>
    <div className="release-history-summary">
      <span>{releaseHistory.length} published releases</span>
      <span>Newest first · Since September 2026</span>
      <a className="text-link" href={`${repository}/releases`}>Browse on GitHub ↗</a>
    </div>
    <ol className="release-timeline" role="list" aria-label="Release history">
      {releaseHistory.map((release, index) => <li className={`release-entry${index === 0 ? " release-current" : ""}`} id={release.tag} key={release.tag}>
        <div className="release-meta">
          <a className="release-version" href={`#${release.tag}`}>{release.tag}<span className="sr-only"> permalink</span></a>
          <time dateTime={release.publishedAt}>{releaseDate.format(new Date(release.publishedAt))}</time>
          {index === 0 && <span className="release-pill"><span className="status-dot" />Latest release</span>}
          {index === releaseHistory.length - 1 && <span className="release-origin">The beginning</span>}
        </div>
        <article aria-labelledby={`${release.tag}-title`}>
          <h2 id={`${release.tag}-title`}>{release.title}</h2>
          <ul>{release.highlights.map(highlight => <li key={highlight}>{highlight}</li>)}</ul>
          <div className="release-actions">
            {index === 0 && <a className="button primary" href={latestRelease}>Download latest ↓</a>}
            <a className="text-link" href={`${repository}/releases/tag/${release.tag}`}>Full release notes<span className="sr-only"> for {release.tag}</span> ↗</a>
          </div>
        </article>
      </li>)}
    </ol>
    <div className="doc-note"><strong>Release verification</strong><p>Mac/Linux builds and installer checks are tracked separately from broader platform validation. The Mac package remains unsigned and unnotarized.</p><a href={`${repository}/blob/main/docs/releases/v${version}-validation.json`}>View the validation manifest →</a></div>
  </main>
}

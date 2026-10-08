import type { Metadata } from "next"
import { releaseGroups } from "../../lib/releases"
import { latestRelease, repository, version } from "../../lib/site"

export const metadata: Metadata = {
  title: "Changelog",
  description: "Multi Codex updates grouped by release series, with version-specific changes, dates and release notes.",
}

const releaseDate = new Intl.DateTimeFormat("en-US", { month: "long", day: "numeric", year: "numeric", timeZone: "UTC" })

export default function Releases() {
  return <main id="main" className="release-page page-width">
    <p className="eyebrow">MULTI CODEX CHANGELOG</p>
    <h1>What’s new.</h1>
    <p className="page-description">Features and fixes, grouped by release series. Open the full release notes for package details and validation.</p>
    <div className="release-history-summary">
      <span>Latest: v{version}</span>
      <span>Newest first · Selected updates</span>
      <a className="text-link" href={`${repository}/releases`}>Browse on GitHub ↗</a>
    </div>
    {releaseGroups.map(group => <section className="release-group" key={group.series} aria-labelledby={`series-${group.series}`}>
      <div className="release-group-heading">
        <p className="eyebrow">{group.series}.x</p>
        <h2 id={`series-${group.series}`}>{group.title}</h2>
        <p>{group.description}</p>
      </div>
      <ol className="release-timeline" role="list" aria-label={`${group.series}.x release history`}>
      {group.releases.map(release => <li className={`release-entry${release.tag === `v${version}` ? " release-current" : ""}`} id={release.tag} key={release.tag}>
        <div className="release-meta">
          <a className="release-version" href={`#${release.tag}`}>{release.tag}<span className="sr-only"> permalink</span></a>
          <time dateTime={release.publishedAt}>{releaseDate.format(new Date(release.publishedAt))}</time>
          {release.tag === `v${version}` && <span className="release-pill"><span className="status-dot" />Latest release</span>}
        </div>
        <article aria-labelledby={`${release.tag}-title`}>
          <h3 id={`${release.tag}-title`}>{release.title}</h3>
          <ul>{release.highlights.map(highlight => <li key={highlight}>{highlight}</li>)}</ul>
          <div className="release-actions">
            {release.tag === `v${version}` && <a className="button primary" href={latestRelease}>Download latest ↓</a>}
            <a className="text-link" href={`${repository}/releases/tag/${release.tag}`}>Full release notes<span className="sr-only"> for {release.tag}</span> ↗</a>
          </div>
        </article>
      </li>)}
      </ol>
    </section>)}
    <div className="doc-note"><strong>Release verification</strong><p>Mac/Linux builds and installer checks are tracked separately from broader platform validation. The Mac package remains unsigned and unnotarized.</p><a href={`${repository}/blob/main/docs/releases/v${version}-validation.json`}>View the validation manifest →</a></div>
  </main>
}

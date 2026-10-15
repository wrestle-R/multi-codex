import type { Metadata } from "next"
import Link from "next/link"
import { releaseGroups } from "../../lib/releases"
import { latestRelease, repository, version } from "../../lib/site"
import { stabilityOf } from "../../lib/update-channels"

export const metadata: Metadata = {
  title: "Changelog",
  description: "Multi Codex updates grouped by release series, with version-specific changes, dates and release notes.",
}

const releaseDate = new Intl.DateTimeFormat("en-US", { month: "long", day: "numeric", year: "numeric", timeZone: "UTC" })

export default async function Releases({ searchParams }: { searchParams: Promise<{ channel?: string }> }) {
  const channel = (await searchParams).channel === "all" ? "all" : "stable"
  const groups = releaseGroups.map(group => ({ ...group, releases: group.releases.filter(release => channel === "all" || stabilityOf(release) === "stable") })).filter(group => group.releases.length)
  return <main id="main" className="release-page page-width">
    <p className="eyebrow">MULTI CODEX CHANGELOG</p>
    <h1>What’s new.</h1>
    <p className="page-description">Features and fixes, grouped by release series. Full release details, upgrade instructions and validation live here. GitHub hosts the downloads.</p>
    <div className="release-history-summary">
      <span>Latest stable: v{version}</span>
      <span>Newest first · Selected updates</span>
      <a className="text-link" href={`${repository}/releases`}>Download archive ↗</a>
    </div>
    <nav className="release-channel-options" aria-label="Release channel"><Link href="/releases" aria-current={channel === "stable" ? "page" : undefined}>Stable updates</Link><Link href="/releases?channel=all" aria-current={channel === "all" ? "page" : undefined}>All updates, including previews</Link></nav>
    {groups.map(group => <section className="release-group" key={group.series} aria-labelledby={`series-${group.series}`}>
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
          <span className="release-pill">{stabilityOf(release) === "preview" ? "Preview" : "Stable"}</span>
          {release.tag === `v${version}` && <span className="release-pill"><span className="status-dot" />Latest stable</span>}
        </div>
        <article aria-labelledby={`${release.tag}-title`}>
          <h3 id={`${release.tag}-title`}>{release.title}</h3>
          <ul>{release.highlights.map(highlight => <li key={highlight}>{highlight}</li>)}</ul>
          <div className="release-actions">
            {release.tag === `v${version}` && <a className="button primary" href={latestRelease}>Download latest ↓</a>}
            <Link className="text-link" href={`/releases/${release.tag}`}>Full release details<span className="sr-only"> for {release.tag}</span> →</Link>
          </div>
        </article>
      </li>)}
      </ol>
    </section>)}
    <div className="doc-note"><strong>Release verification</strong><p>Each release page explains the tested packages, known limitations and available validation evidence. The current Mac package remains unsigned and unnotarized.</p><Link href={`/releases/v${version}`}>Read the latest release details →</Link></div>
  </main>
}

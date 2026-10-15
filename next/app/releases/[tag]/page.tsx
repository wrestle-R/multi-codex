import { readFile } from "node:fs/promises"
import { join } from "node:path"
import type { Metadata } from "next"
import Link from "next/link"
import { notFound } from "next/navigation"
import Markdown from "react-markdown"
import remarkGfm from "remark-gfm"
import { releaseHistory } from "../../../lib/releases"
import { repository } from "../../../lib/site"
import { stabilityOf } from "../../../lib/update-channels"

export const dynamicParams = false
export function generateStaticParams() { return releaseHistory.map(release => ({ tag: release.tag })) }

export async function generateMetadata({ params }: { params: Promise<{ tag: string }> }): Promise<Metadata> {
  const { tag } = await params
  const release = releaseHistory.find(release => release.tag === tag)
  if (!release) notFound()
  return { title: `${tag} — ${release.title}`, description: release.highlights.join(" ") }
}

const date = new Intl.DateTimeFormat("en-US", { month: "long", day: "numeric", year: "numeric", timeZone: "UTC" })

export default async function ReleaseDetails({ params }: { params: Promise<{ tag: string }> }) {
  const { tag } = await params
  const release = releaseHistory.find(release => release.tag === tag)
  if (!release) notFound()
  const content = await readFile(join(process.cwd(), "content/releases", `${release.tag}.md`), "utf8")
  return <main id="main" className="page-width docs-layout">
    <aside className="docs-sidebar"><Link href={`/releases#${tag}`}>← All releases</Link><nav aria-label="Release history">{releaseHistory.map(item => <Link key={item.tag} href={`/releases/${item.tag}`} aria-current={item.tag === tag ? "page" : undefined}>{item.tag}</Link>)}</nav></aside>
    <article className="docs-content markdown-guide release-detail">
      <p className="eyebrow">PUBLISHED RELEASE · <time dateTime={release.publishedAt}>{date.format(new Date(release.publishedAt))}</time></p>
      <p className="release-pill">{stabilityOf(release) === "preview" ? "Preview release" : "Stable release"}</p>
      {stabilityOf(release) === "preview" && <div className="doc-note"><strong>Preview</strong><p>This release has known Orbit issues and is awaiting further local testing. Choose Stable updates in the app to receive stable releases only.</p></div>}
      <Markdown remarkPlugins={[remarkGfm]}>{content}</Markdown>
      <div className="release-actions"><Link className="text-link" href={`/releases#${tag}`}>Back to changelog →</Link><a className="button primary" href={`${repository}/releases/tag/${tag}`}>Download {tag} ↓</a></div>
    </article>
  </main>
}

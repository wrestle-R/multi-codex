import { readFile } from "node:fs/promises"
import { join } from "node:path"
import { notFound } from "next/navigation"
import Link from "next/link"
import Markdown from "react-markdown"
import remarkGfm from "remark-gfm"

const guides = ["platform-support", "extension", "extension-publishing", "release-guidelines"]
export function generateStaticParams() { return guides.map(guide => ({ guide })) }
export const dynamicParams = false
export default async function Guide({ params }: { params: Promise<{ guide: string }> }) {
  const { guide } = await params
  if (!guides.includes(guide)) notFound()
  const content = await readFile(join(process.cwd(), "content/guides", `${guide}.md`), "utf8")
  return <main id="main" className="page-width docs-layout"><aside className="docs-sidebar"><Link href="/docs">← Documentation</Link><nav aria-label="Detailed guides">{guides.map(slug => <Link key={slug} href={`/docs/${slug}`}>{slug.replaceAll("-", " ")}</Link>)}</nav></aside><article className="docs-content markdown-guide"><Markdown remarkPlugins={[remarkGfm]}>{content}</Markdown></article></main>
}

export type UpdateChannel = "stable" | "all"
export type Stability = "stable" | "preview"
export interface PublishedRelease {
  tag_name: string
  draft: boolean
  prerelease: boolean
  published_at: string | null
  assets: { name: string; browser_download_url: string }[]
}

export function stabilityOf(release: { tag: string; stability?: Stability }) {
  return release.stability ?? (release.tag.includes("-") ? "preview" : "stable")
}

function version(tag: string) {
  const match = /^v?(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-([\da-zA-Z-]+(?:\.[\da-zA-Z-]+)*))?(?:\+[\da-zA-Z.-]+)?$/.exec(tag)
  if (!match || match.slice(1, 4).some(n => !Number.isSafeInteger(Number(n)))) return null
  const pre = match[4]?.split(".") ?? []
  if (pre.some(n => /^\d+$/.test(n) && n.length > 1 && n.startsWith("0"))) return null
  return { core: match.slice(1, 4).map(Number), pre }
}
export function compareVersions(left: string, right: string) {
  const a = version(left); const b = version(right)
  if (!a || !b) throw new Error("Invalid release version")
  for (let i = 0; i < 3; i++) if (a.core[i] !== b.core[i]) return a.core[i] > b.core[i] ? 1 : -1
  if (!a.pre.length || !b.pre.length) return Number(!a.pre.length) - Number(!b.pre.length)
  for (let i = 0; i < Math.max(a.pre.length, b.pre.length); i++) {
    if (a.pre[i] === b.pre[i]) continue
    if (a.pre[i] == null || b.pre[i] == null) return a.pre[i] == null ? -1 : 1
    const numericA = /^\d+$/.test(a.pre[i]); const numericB = /^\d+$/.test(b.pre[i])
    if (numericA !== numericB) return numericA ? -1 : 1
    if (numericA && a.pre[i].length !== b.pre[i].length) return a.pre[i].length > b.pre[i].length ? 1 : -1
    return a.pre[i] > b.pre[i] ? 1 : -1
  }
  return 0
}
export function selectUpdate(releases: PublishedRelease[], channel: UpdateChannel, previewTags: Set<string>) {
  return releases.filter(release => !release.draft && release.published_at && version(release.tag_name)
    && (channel === "all" || (!release.prerelease && !previewTags.has(release.tag_name) && !version(release.tag_name)!.pre.length))
    && release.assets.some(asset => asset.name === "latest.json"))
    .sort((a, b) => compareVersions(b.tag_name, a.tag_name))[0] ?? null
}

export function manifestURL(release: PublishedRelease) {
  const url = new URL(release.assets.find(asset => asset.name === "latest.json")!.browser_download_url)
  const expected = `/wrestle-R/multi-codex/releases/download/${release.tag_name}/latest.json`
  if (url.protocol !== "https:" || url.hostname !== "github.com" || decodeURIComponent(url.pathname) !== expected || url.username || url.password || url.search || url.hash) throw new Error("Invalid release manifest location")
  return url
}

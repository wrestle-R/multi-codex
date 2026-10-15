import { releaseHistory } from "./releases"
import { manifestURL, selectUpdate, stabilityOf, type PublishedRelease } from "./update-channels"

export function createUpdateHandler(fetchRelease: typeof fetch = fetch) {
  return async (channel: string) => {
    if (channel !== "all" && channel !== "stable") return Response.json({ error: "Unknown update channel" }, { status: 404 })
    try {
      const response = await fetchRelease("https://api.github.com/repos/wrestle-R/multi-codex/releases?per_page=100", { headers: { Accept: "application/vnd.github+json" }, next: { revalidate: 120 }, signal: AbortSignal.timeout(15000) })
      if (!response.ok) throw new Error("Release service unavailable")
      const releases = await response.json() as PublishedRelease[]
      if (!Array.isArray(releases)) throw new Error("Invalid release inventory")
      const previewTags = new Set(releaseHistory.filter(release => stabilityOf(release) === "preview").map(release => release.tag))
      const release = selectUpdate(releases, channel, previewTags)
      if (!release) return new Response(null, { status: 204 })
      const payload = await fetchRelease(manifestURL(release), { next: { revalidate: 120 }, signal: AbortSignal.timeout(15000) })
      if (!payload.ok) throw new Error("Manifest unavailable")
      const text = await payload.text()
      if (text.length > 1024 * 1024) throw new Error("Invalid manifest size")
      const manifest = JSON.parse(text)
      if (manifest.version !== release.tag_name.replace(/^v/, "") || !manifest.platforms || typeof manifest.platforms !== "object" || Array.isArray(manifest.platforms) || !Object.keys(manifest.platforms).length) throw new Error("Release manifest does not match its version")
      for (const platform of Object.values(manifest.platforms) as { signature?: unknown; url?: unknown }[]) {
        if (!platform || typeof platform.signature !== "string" || !platform.signature.trim() || typeof platform.url !== "string" || new URL(platform.url).protocol !== "https:") throw new Error("Invalid signed update package")
      }
      // The native SDK still verifies the platform payload's embedded signature.
      return Response.json(manifest, { headers: { "Cache-Control": "public, max-age=60, s-maxage=120", "X-Multi-Codex-Channel": channel } })
    } catch {
      return Response.json({ error: "The update service is temporarily unavailable" }, { status: 503, headers: { "Retry-After": "60", "Cache-Control": "no-store" } })
    }
  }
}

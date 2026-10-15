import { expect, test } from "@playwright/test"
import { compareVersions, manifestURL, selectUpdate, type PublishedRelease } from "../lib/update-channels"
const release = (tag: string, prerelease = false): PublishedRelease => ({ tag_name: tag, draft: false, prerelease, published_at: "2026-10-15T00:00:00Z", assets: [{ name: "latest.json", browser_download_url: `https://github.com/wrestle-R/multi-codex/releases/download/${tag}/latest.json` }] })
test("stable excludes owner-reported unstable versions, previews, drafts and missing packages", () => {
  const releases = [release("v1.8.0-beta.1", true), release("v1.7.0"), release("v1.6.1"), { ...release("v2.0.0"), draft: true }, { ...release("v3.0.0"), assets: [] }]
  expect(selectUpdate(releases, "stable", new Set(["v1.7.0"]))?.tag_name).toBe("v1.6.1")
  expect(selectUpdate(releases, "all", new Set(["v1.7.0"]))?.tag_name).toBe("v1.8.0-beta.1")
})
test("numeric version ordering and stable promotion do not depend on publication order", () => {
  expect(selectUpdate([release("v1.9.0"), release("v1.10.0")], "stable", new Set())?.tag_name).toBe("v1.10.0")
  for (const [a,b] of [["1.8.0","1.8.0-beta.9"],["1.8.0-beta.10","1.8.0-beta.2"],["2.0.0","1.99.99"]]) expect(compareVersions(a,b)).toBeGreaterThan(0)
  expect(compareVersions("1.8.0+build.1","1.8.0+build.2")).toBe(0)
})
test("unpublished and malformed versions are excluded", () => {
  expect(selectUpdate([release("v1.08.0"), { ...release("v1.9.0"), published_at: null }], "all", new Set())).toBeNull()
  expect(() => compareVersions("1.8.0-beta.01", "1.8.0")).toThrow()
})
test("manifest redirects stay scoped to this repository and exact release", () => {
  expect(manifestURL(release("v1.6.1")).href).toContain("/download/v1.6.1/latest.json")
  for (const url of ["https://evil.example/latest.json", "http://github.com/wrestle-R/multi-codex/releases/download/v1.6.1/latest.json", "https://github.com/other/project/releases/download/v1.6.1/latest.json"]) {
    const r = release("v1.6.1"); r.assets[0].browser_download_url = url; expect(() => manifestURL(r)).toThrow()
  }
})

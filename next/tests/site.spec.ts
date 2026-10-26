import { test, expect } from "@playwright/test"

test("extension installation links use the public Marketplace and explain shared accounts", async ({ page }) => {
  const publicUrl = "https://marketplace.visualstudio.com/items?itemName=russeldanielpaul.multi-codex"
  await page.goto("/")
  await expect(page.getByRole("link", { name: "Get VS Code extension" })).toHaveAttribute("href", publicUrl)
  await expect(page.getByRole("link", { name: "Install from Marketplace" })).toHaveAttribute("href", publicUrl)
  await expect(page.locator("#vscode")).toContainText("Intel and Apple Silicon")
  await page.getByRole("link", { name: "Extension setup and compatibility" }).click()
  await expect(page).toHaveURL(/\/docs#vscode$/)
  await expect(page.locator("#vscode")).toContainText("saved accounts appear automatically")
  await expect(page.locator("#vscode")).toContainText("connects switching automatically")
  await expect(page.getByRole("link", { name: "Install the pre-release" })).toHaveAttribute("href", publicUrl)
  await expect(page.locator('a[href*="/manage/publishers/"]')).toHaveCount(0)
  for (const width of [320, 768, 1440]) {
    await page.setViewportSize({ width, height: 900 })
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(width)
  }
})

test("changelog groups selected updates by series and preserves version links and permalinks", async ({ page }) => {
  const tags = ["v1.7.4", "v1.7.3", "v1.7.2", "v1.7.1", "v1.7.0", "v1.6.1", "v1.5.3", "v1.5.2", "v1.5.1", "v1.5.0", "v1.4.1", "v1.4.0", "v1.3.8", "v1.3.7", "v1.3.6", "v1.3.5", "v1.2.3", "v1.2.1", "v1.2.0", "v1.1.7", "v1.1.6", "v1.1.3", "v1.1.2", "v1.1.1", "v1.0.2", "v1.0.0", "v0.2.0", "v0.1.1", "v0.1.0"]
  await page.goto("/releases?channel=all")
  const groups = page.locator(".release-group")
  await expect(groups).toHaveCount(10)
  expect(await groups.locator(".release-group-heading .eyebrow").allTextContents()).toEqual(["1.7.x", "1.6.x", "1.5.x", "1.4.x", "1.3.x", "1.2.x", "1.1.x", "1.0.x", "0.2.x", "0.1.x"])
  await expect(page.locator(".release-history-summary")).toContainText("Latest stable: v1.7.4")
  await expect(page.locator(".release-history-summary")).toContainText("Selected updates")
  const timeline = page.locator(".release-timeline")
  const entries = timeline.locator(":scope > li")
  await expect(entries).toHaveCount(tags.length)
  expect(await entries.evaluateAll(elements => elements.map(element => element.id))).toEqual(tags)
  let previous = Infinity
  for (const [index, tag] of tags.entries()) {
    const entry = entries.nth(index)
    const published = Date.parse((await entry.locator("time").getAttribute("datetime"))!)
    expect(Number.isFinite(published) && published <= previous).toBe(true)
    previous = published
    await expect(entry.getByRole("heading", { level: 3 })).not.toHaveText("")
    expect(await entry.locator("article li").count()).toBeGreaterThanOrEqual(1)
    await expect(entry.getByRole("link", { name: `Full release details for ${tag} →` })).toHaveAttribute("href", `/releases/${tag}`)
  }
  await expect(page.locator("#v1\\.7\\.4")).toContainText("Latest stable")
  await expect(page.getByText("Latest stable", { exact: true })).toHaveCount(1)
  await timeline.getByRole("link", { name: "v0.1.0 permalink", exact: true }).click()
  await expect(page).toHaveURL(/#v0\.1\.0$/)
  for (const theme of ["dark", "light"]) {
    await page.getByRole("button", { name: "Toggle color theme" }).click()
    await expect(page.locator("html")).toHaveAttribute("data-theme", theme)
    await page.setViewportSize({ width: 320, height: 800 })
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(320)
  }
})

test("full release details stay on the website and preserve package downloads", async ({ page }) => {
  await page.goto("/releases")
  await page.getByRole("link", { name: "Full release details for v1.7.4 →" }).click()
  await expect(page).toHaveURL(/\/releases\/v1\.7\.4$/)
  await expect(page.getByRole("heading", { level: 1 })).toHaveText("Multi Codex v1.7.4")
  await expect(page.getByRole("heading", { name: "Updating and compatibility", exact: true })).toBeVisible()
  await expect(page.getByRole("link", { name: "Download v1.7.4 ↓" })).toHaveAttribute("href", "https://github.com/wrestle-R/multi-codex/releases/tag/v1.7.4")
  for (const width of [320, 768, 1440]) {
    await page.setViewportSize({ width, height: 900 })
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(width)
  }
  const response = await page.goto("/releases/v1.3.4")
  expect(response?.status()).toBe(404)
})

test("showcase, docs and changelog navigate with working release links and no horizontal overflow", async ({ page }) => {
  const errors: string[] = []
  page.on("pageerror", error => errors.push(error.message))
  for (const [url, heading] of [["/", /A little order/], ["/docs", /A workspace/], ["/releases", "What’s new."]] as const) {
    await page.goto(url)
    await expect(page.getByRole("heading", { level: 1, name: heading })).toBeVisible()
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(page.viewportSize()!.width)
    await expect(page.getByRole("navigation", { name: "Main navigation" }).getByRole("link", { name: "Download" })).toHaveAttribute("href", "https://github.com/wrestle-R/multi-codex/releases/tag/v1.7.4")
  }
  await page.getByRole("navigation", { name: "Main navigation" }).getByRole("link", { name: "Docs", exact: true }).click()
  await expect(page).toHaveURL(/\/docs$/)
  await page.getByRole("navigation", { name: "Documentation sections" }).getByRole("link", { name: "Choose a terminal" }).click()
  await expect(page).toHaveURL(/#terminals$/)
  await expect(page.getByRole("heading", { name: "The terminal you already like." })).toBeVisible()
  expect(errors).toEqual([])
})

test("platform controls copy the exact install and update commands, including the Mac unsigned opt-in", async ({ page }) => {
  await page.addInitScript(() => Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText: async (text: string) => { document.documentElement.dataset.copied = text } } }))
  await page.goto("/docs")
  const install = page.locator("#installation .install-box")
  const update = page.locator("#updates .install-box")
  await install.getByRole("button", { name: "macOS", exact: true }).click()
  await expect(install.getByRole("button", { name: "macOS", exact: true })).toHaveAttribute("aria-pressed", "true")
  await expect(install.locator("code")).toContainText("MULTI_CODEX_ALLOW_UNSIGNED_MAC=1 bash /tmp/install-multi-codex.sh")
  await install.getByRole("button", { name: "Copy command" }).click()
  await expect(install.getByRole("button", { name: "Copied ✓" })).toBeVisible()
  expect(await page.locator("html").getAttribute("data-copied")).toBe(await install.locator("code").textContent())
  await update.getByRole("button", { name: "Copy command" }).click()
  expect(await page.locator("html").getAttribute("data-copied")).toBe(await update.locator("code").textContent())
  await expect(update.locator("code")).toContainText("update-app.sh")
  await install.getByRole("button", { name: "Linux", exact: true }).click()
  await expect(install.locator("code")).not.toContainText("MULTI_CODEX_ALLOW_UNSIGNED_MAC")
})

test("screenshots load and troubleshooting answers and missing pages are usable", async ({ page }) => {
  await page.goto("/")
  const image = page.getByRole("img", { name: /account dashboard/ })
  await expect(image).toBeVisible()
  await expect.poll(() => image.evaluate(element => (element as HTMLImageElement).naturalWidth)).toBeGreaterThan(0)
  await page.goto("/docs#troubleshooting")
  await page.getByText("Why does the CLI ask me to sign in?", { exact: true }).click()
  await expect(page.getByText(/Running codex separately in a terminal/)).toBeVisible()
  const response = await page.goto("/missing-page")
  expect(response?.status()).toBe(404)
  await expect(page.getByRole("heading", { name: "This workspace is empty." })).toBeVisible()
  await page.getByRole("link", { name: /Back to Multi Codex/ }).click()
  await expect(page).toHaveURL(/\/$/)
})


test("Windows commands, menu-bar behavior and migrated detailed guides are accessible", async ({ page }) => {
  await page.goto("/docs")
  const install = page.locator("#installation .install-box")
  await install.getByRole("button", { name: "Windows", exact: true }).click()
  const installerUrl = "https://github.com/wrestle-R/multi-codex/releases/download/v1.7.4/Multi.Codex_1.7.4_x64-setup.exe"
  await expect(install.getByRole("link", { name: "Download Windows .exe" })).toHaveAttribute("href", installerUrl)
  await expect(install).toContainText("double-click")
  await expect(install.getByRole("button", { name: "Copy command" })).not.toBeVisible()
  await install.locator("summary").click()
  await expect(install.getByRole("button", { name: "Copy command" })).toBeVisible()
  await expect(install.locator("code")).toContainText("install-app.ps1")
  await expect(install.locator("code")).toContainText("$env:TEMP")
  await expect(install.locator("code")).toContainText("-UseBasicParsing")
  const update = page.locator("#updates .install-box")
  await update.getByRole("button", { name: "Windows", exact: true }).click()
  await expect(update.getByRole("link", { name: "Download Windows .exe" })).toHaveAttribute("href", installerUrl)
  await expect(update).toContainText("saved accounts and conversations stay in place")
  for (const width of [320, 768, 1440]) {
    await page.setViewportSize({ width, height: 900 })
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(width)
  }
  await expect(page.locator("#menu-bar")).toContainText("Quit Multi Codex")
  await expect(page.locator("#vscode")).toContainText("every saved account")
  await page.goto("/")
  await page.getByRole("button", { name: "Windows", exact: true }).click()
  await expect(page.getByRole("link", { name: "Download Windows .exe" })).toHaveAttribute("href", installerUrl)
  for (const guide of ["platform-support", "extension", "extension-publishing", "release-guidelines", "storage-and-updates"]) {
    await page.goto(`/docs/${guide}`)
    await expect(page.getByRole("heading", { level: 1 })).toBeVisible()
    await expect(page.getByRole("navigation", { name: "Detailed guides" })).toBeVisible()
    for (const width of [320,768,1440]) {
      await page.setViewportSize({ width, height: 900 })
      expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(width)
    }
  }
})


test("CLI guide explains package limits, shared deletion and recovery without release claims", async ({ page }) => {
  await page.goto("/")
  await page.getByRole("link", { name: "Use Multi Codex from a terminal" }).click()
  await expect(page).toHaveURL(/\/docs\/cli$/)
  await expect(page.getByRole("heading", { name: "Multi Codex CLI", exact: true })).toBeVisible()
  await expect(page.locator("article")).toContainText("glibc 2.39+")
  await expect(page.locator("article")).toContainText("Shared conversations remain available")
  await expect(page.locator("article")).toContainText("storage backups restore")
  await expect(page.locator("article")).toContainText("native verification")
  for (const width of [320, 768, 1440]) {
    await page.setViewportSize({ width, height: 900 })
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(width)
  }
})


test("Orbit is documented with its screenshot and an off switch on narrow screens", async ({ page }) => {
  await page.goto("/")
  const orbit = page.locator("#orbit")
  await expect(orbit).toContainText("Orbiters")
  await expect(orbit.getByRole("img")).toBeVisible()
  await orbit.getByRole("link", { name: /Set up Orbit/ }).click()
  await expect(page).toHaveURL(/\/docs\/orbit$/)
  await expect(page.locator("article")).toContainText("Enable Orbit")
  await expect(page.locator("article")).toContainText("two most recent non-archived conversations")
  await expect(page.locator("article")).toContainText("recent Desktop shortcuts do not")
  for (const width of [360, 768, 1440]) {
    await page.setViewportSize({ width, height: 900 })
    await page.goto("/")
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(width)
  }
})


test("stable release view excludes owner-classified previews and All includes them", async ({ page }) => {
  await page.goto("/releases")
  await expect(page.getByRole("link", { name: "Full release details for v1.7.0 →" })).toHaveCount(0)
  await page.getByRole("link", { name: "All updates, including previews" }).click()
  await expect(page.getByRole("link", { name: "Full release details for v1.7.0 →" })).toBeVisible()
  await page.getByRole("link", { name: "Full release details for v1.7.0 →" }).click()
  await expect(page.getByText("Preview release", { exact: true })).toBeVisible()
})

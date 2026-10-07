import { test, expect } from "@playwright/test"

test("release timeline includes every published version with an improvement and its original notes", async ({ page }) => {
  const tags = ["v1.3.8", "v1.3.7", "v1.3.6", "v1.3.5", "v1.2.3", "v1.2.1", "v1.2.0", "v1.1.7", "v1.1.6", "v1.1.3", "v1.1.2", "v1.1.1", "v1.0.2", "v1.0.0", "v0.2.0", "v0.1.1", "v0.1.0"]
  await page.goto("/releases")
  const timeline = page.getByRole("list", { name: "Release history" })
  const entries = timeline.locator(":scope > li")
  await expect(entries).toHaveCount(tags.length)
  expect(await entries.evaluateAll(elements => elements.map(element => element.id))).toEqual(tags)
  let previous = Infinity
  for (const [index, tag] of tags.entries()) {
    const entry = entries.nth(index)
    const published = Date.parse((await entry.locator("time").getAttribute("datetime"))!)
    expect(Number.isFinite(published) && published <= previous).toBe(true)
    previous = published
    await expect(entry.getByRole("heading", { level: 2 })).not.toHaveText("")
    expect(await entry.locator("article li").count()).toBeGreaterThanOrEqual(1)
    await expect(entry.getByRole("link", { name: `Full release notes for ${tag} ↗` })).toHaveAttribute("href", `https://github.com/wrestle-R/multi-codex/releases/tag/${tag}`)
  }
  await expect(entries.first()).toContainText("Latest release")
  await expect(entries.last()).toContainText("The beginning")
  await timeline.getByRole("link", { name: "v0.1.0 permalink", exact: true }).click()
  await expect(page).toHaveURL(/#v0\.1\.0$/)
  for (const theme of ["dark", "light"]) {
    await page.getByRole("button", { name: "Toggle color theme" }).click()
    await expect(page.locator("html")).toHaveAttribute("data-theme", theme)
    await page.setViewportSize({ width: 320, height: 800 })
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(320)
  }
})

test("showcase, docs and changelog navigate with working release links and no horizontal overflow", async ({ page }) => {
  const errors: string[] = []
  page.on("pageerror", error => errors.push(error.message))
  for (const [url, heading] of [["/", /A little order/], ["/docs", /A workspace/], ["/releases", "What’s new."]] as const) {
    await page.goto(url)
    await expect(page.getByRole("heading", { level: 1, name: heading })).toBeVisible()
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(page.viewportSize()!.width)
    await expect(page.getByRole("navigation", { name: "Main navigation" }).getByRole("link", { name: "Download" })).toHaveAttribute("href", "https://github.com/wrestle-R/multi-codex/releases/latest")
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

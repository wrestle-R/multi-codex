import { test, expect } from "@playwright/test"

test("switches mode immediately and keeps the chosen palette after reload", async ({ page }) => {
  await page.addInitScript(() => {
    document.startViewTransition = () => { throw new Error("Theme switching must not take a full-page snapshot") }
  })
  await page.emulateMedia({ colorScheme: "light", reducedMotion: "no-preference" })
  await page.goto("/docs")
  const picker = page.getByRole("combobox", { name: "Website palette" })
  await picker.selectOption("midnight")
  await page.getByRole("button", { name: "Toggle color theme" }).click()
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark")
  await expect(page.locator("html")).toHaveAttribute("data-palette", "midnight")
  await page.reload()
  await expect(picker).toHaveValue("midnight")
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark")
})

test("theme remains immediate with reduced motion or without view transitions", async ({ page }) => {
  await page.emulateMedia({ colorScheme: "light", reducedMotion: "reduce" })
  await page.goto("/docs")
  const toggle = page.getByRole("button", { name: "Toggle color theme" })
  await expect(toggle).toHaveAttribute("aria-pressed", "false")
  await toggle.click()
  await expect(toggle).toHaveAttribute("aria-pressed", "true")
  await expect(page.locator("html")).not.toHaveClass(/theme-transition/)
  await page.emulateMedia({ reducedMotion: "no-preference" })
  await page.evaluate(() => Object.defineProperty(document, "startViewTransition", { value: undefined, configurable: true }))
  await toggle.click()
  await expect(toggle).toHaveAttribute("aria-pressed", "false")
  await expect(page.locator("html")).not.toHaveClass(/theme-transition/)
})

test("docs keep the sidebar close and reuse the app’s terminal mark at all widths", async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "reduce" })
  for (const width of [1672, 1024, 320]) {
    await page.setViewportSize({ width, height: 1000 })
    await page.goto("/docs")
    const icon = page.locator(".nav-shell .brand-icon")
    await expect(icon.locator("span")).toHaveCount(2)
    await expect(icon.locator("i")).toHaveText(">_")
    await expect(icon).toHaveAttribute("aria-hidden", "true")
    await expect(page.locator('link[rel="icon"]')).toHaveAttribute("href", /icon\.svg/)
    for (const theme of ["light", "dark"]) {
      await page.evaluate(mode => { document.documentElement.dataset.theme = mode }, theme)
      expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(width)
      if (width > 900) {
        const sidebar = await page.locator(".docs-sidebar").boundingBox()
        const content = await page.locator(".docs-content").boundingBox()
        const gap = content!.x - sidebar!.x - sidebar!.width
        expect(gap).toBeGreaterThan(0)
        expect(gap).toBeLessThanOrEqual(48)
      }
    }
  }
})

test("docs explain Linux-only desktop placement and show screenshots matching a persistent theme", async ({ page }) => {
  test.setTimeout(60_000)
  const errors: string[] = []
  page.on("pageerror", error => errors.push(error.message))
  await page.emulateMedia({ colorScheme: "light" })
  await page.goto("/docs#launching")
  await expect(page.locator("#launching")).toContainText("Desktop selection is Linux-only")
  await expect(page.locator("#launching")).toContainText("On macOS and Windows, the app launches on the current desktop after you choose a folder.")
  await expect(page.locator("#terminals")).toContainText("installed terminals only")
  const screenshot = page.getByRole("img", { name: "Multi Codex Linux desktop selection interface" })
  await expect(screenshot).toHaveAttribute("src", /workspace-light/)
  await screenshot.scrollIntoViewIfNeeded()
  await expect.poll(() => screenshot.evaluate(element => (element as HTMLImageElement).naturalWidth), { timeout: 20_000 }).toBeGreaterThan(0)
  const toggle = page.getByRole("button", { name: "Toggle color theme" })
  await toggle.focus()
  await page.keyboard.press("Enter")
  await expect(toggle).toHaveAttribute("aria-pressed", "true")
  await expect(screenshot).toHaveAttribute("src", /workspace-dark/)
  await screenshot.scrollIntoViewIfNeeded()
  await expect.poll(() => screenshot.evaluate(element => (element as HTMLImageElement).naturalWidth), { timeout: 20_000 }).toBeGreaterThan(0)
  expect(await page.locator("body").evaluate(element => getComputedStyle(element).backgroundColor)).toBe("rgb(24, 27, 24)")
  await page.reload()
  await expect(toggle).toHaveAttribute("aria-pressed", "true")
  await page.emulateMedia({ colorScheme: "dark" })
  await page.emulateMedia({ colorScheme: "light" })
  await expect(toggle).toHaveAttribute("aria-pressed", "true")
  await page.getByRole("link", { name: "Multi Codex home" }).click()
  await expect(page.getByRole("img", { name: /account dashboard/ })).toHaveAttribute("src", /accounts-dark/)
  await toggle.click()
  await expect(page.getByRole("img", { name: /account dashboard/ })).toHaveAttribute("src", /accounts-light/)
  await page.getByRole("link", { name: "Changelog", exact: true }).click()
  await expect(toggle).toHaveAttribute("aria-pressed", "false")
  await page.setViewportSize({ width: 320, height: 700 })
  await page.goto("/docs")
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(320)
  await toggle.click()
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(320)
  expect(errors).toEqual([])
})

test("new visitors follow system theme changes until choosing their own theme", async ({ page }) => {
  await page.emulateMedia({ colorScheme: "dark" })
  await page.goto("/docs")
  const toggle = page.getByRole("button", { name: "Toggle color theme" })
  await expect(toggle).toHaveAttribute("aria-pressed", "true")
  await page.emulateMedia({ colorScheme: "light" })
  await expect(toggle).toHaveAttribute("aria-pressed", "false")
  await toggle.click()
  await page.emulateMedia({ colorScheme: "dark" })
  await page.emulateMedia({ colorScheme: "light" })
  await expect(toggle).toHaveAttribute("aria-pressed", "true")
})

test("theme selection remains usable when browser storage is unavailable", async ({ page }) => {
  await page.addInitScript(() => {
    Storage.prototype.getItem = () => { throw new Error("Storage blocked") }
    Storage.prototype.setItem = () => { throw new Error("Storage blocked") }
  })
  await page.emulateMedia({ colorScheme: "light" })
  await page.goto("/docs")
  const toggle = page.getByRole("button", { name: "Toggle color theme" })
  await expect(toggle).toHaveAttribute("aria-pressed", "false")
  await toggle.click()
  await expect(toggle).toHaveAttribute("aria-pressed", "true")
  await page.getByRole("link", { name: "Changelog", exact: true }).click()
  await expect(toggle).toHaveAttribute("aria-pressed", "true")
})

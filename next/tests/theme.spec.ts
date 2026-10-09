import { test, expect } from "@playwright/test"

test("theme curtain alternates from the toggle and cleans up after both reveals", async ({ page }) => {
  await page.addInitScript(() => {
    const start = document.startViewTransition?.bind(document)
    const observations: object[] = []
    Object.assign(window, { themeReveals: observations })
    if (!start) return
    document.startViewTransition = update => {
      const transition = start(update)
      transition.ready.then(() => {
        const root = document.documentElement
        const direction = root.dataset.themeReveal
        const layer = direction === "contract" ? "old" : "new"
        const style = getComputedStyle(root, `::view-transition-${layer}(root)`)
        observations.push({ direction, x: root.style.getPropertyValue("--theme-transition-x"), y: root.style.getPropertyValue("--theme-transition-y"), animation: style.animationName, duration: style.animationDuration, easing: style.animationTimingFunction })
      }, () => undefined)
      return transition
    }
  })
  await page.emulateMedia({ colorScheme: "light", reducedMotion: "no-preference" })
  await page.goto("/docs")
  const root = page.locator("html")
  const toggle = page.getByRole("button", { name: "Toggle color theme" })
  await expect(toggle).toHaveAttribute("aria-pressed", "false")
  const rect = await toggle.boundingBox()
  for (const pressed of ["true", "false"]) {
    await toggle.click()
    await expect(toggle).toHaveAttribute("aria-pressed", pressed)
    await expect(root).not.toHaveClass(/theme-transition/)
    await expect(root).not.toHaveAttribute("data-theme-reveal")
  }
  const observations = await page.evaluate(() => (window as unknown as { themeReveals: { direction: string; x: string; y: string; animation: string; duration: string; easing: string }[] }).themeReveals)
  if (await page.evaluate(() => typeof document.startViewTransition === "function")) {
    expect(observations.map(item => item.direction)).toEqual(["expand", "contract"])
    for (const item of observations) {
      expect(parseFloat(item.x)).toBeCloseTo(rect!.x + rect!.width / 2, 0)
      expect(parseFloat(item.y)).toBeCloseTo(rect!.y + rect!.height / 2, 0)
      expect(item.animation).toBe("theme-curtain-reveal")
      expect(item.duration).toBe("0.96s")
      expect(item.easing).toBe("cubic-bezier(0.55, 0, 0.8, 1)")
    }
  }
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

import { test, expect } from "@playwright/test"

const release = "https://github.com/wrestle-R/multi-codex/releases/download/v1.8.1/"
for (const device of [
  { name: "Windows", ua: "Mozilla/5.0 (Windows NT 10.0; Win64; x64)", hint: '"Windows"', file: "Multi.Codex_1.8.1_x64-setup.exe", label: "Windows x64", step: "Start menu" },
  { name: "Mac", ua: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)", hint: '"macOS"', file: "Multi.Codex_1.8.1_aarch64.dmg", label: "Apple Silicon Mac", step: "About This Mac" },
  { name: "Linux", ua: "Mozilla/5.0 (X11; Linux x86_64)", hint: '"Linux"', file: "Multi.Codex_1.8.1_amd64.AppImage", label: "Linux x64", step: "Permissions" },
]) {
  test(`one install link suggests ${device.name} with its setup steps`, async ({ browser }) => {
    const context = await browser.newContext({ userAgent: device.ua, extraHTTPHeaders: { "sec-ch-ua-platform": device.hint } })
    const page = await context.newPage()
    await page.goto("/install")
    await expect(page.locator(".install-download").getByRole("link", { name: `Download for ${device.label}` })).toHaveAttribute("href", release + device.file)
    await expect(page.locator(".install-download a[href*='/releases/download/']")).toHaveCount(1)
    await expect(page.locator("main")).toContainText(device.step)
    if (device.name === "Linux") {
      await page.getByText("AppImage won’t open?", { exact: true }).click()
      await expect(page.locator("code")).toContainText("--appimage-extract-and-run")
    }
    for (const width of [320, 768, 1440]) {
      await page.setViewportSize({ width, height: 900 })
      expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(width)
    }
    await context.close()
  })
}

for (const device of [
  { name: "Android", ua: "Mozilla/5.0 (Linux; Android 14; Pixel 8) Mobile", hint: '"Android"', arch: "", text: "desktop app" },
  { name: "iPad desktop browsing", ua: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15) Mobile/15E148", hint: "", arch: "", text: "desktop app" },
  { name: "Linux ARM", ua: "Mozilla/5.0 (X11; Linux aarch64)", hint: '"Linux"', arch: '"arm"', text: "native ARM package" },
  { name: "Windows ARM hint", ua: "Mozilla/5.0 (Windows NT 10.0; Win64; x64)", hint: '"Windows"', arch: '"arm"', text: "native ARM package" },
  { name: "32-bit Linux", ua: "Mozilla/5.0 (X11; Linux i686)", hint: '"Linux"', arch: "", text: "64-bit computer" },
  { name: "Chromebook", ua: "Mozilla/5.0 (X11; CrOS x86_64)", hint: '"Chrome OS"', arch: "", text: "Chromebook" },
  { name: "unknown device", ua: "Unknown browser", hint: "", arch: "", text: "Choose the computer" },
]) {
  test(`${device.name} gets a picker instead of an incompatible download`, async ({ request }) => {
    const response = await request.get("/install", { headers: { "user-agent": device.ua, "sec-ch-ua-platform": device.hint, "sec-ch-ua-arch": device.arch } })
    expect(response.status()).toBe(200)
    const html = await response.text()
    expect(html).toContain(device.text)
    expect(html).not.toContain('class="install-download"')
  })
}

test("computer selection works without JavaScript and overrides phone detection", async ({ browser }) => {
  const context = await browser.newContext({ javaScriptEnabled: false, userAgent: "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) Mobile", extraHTTPHeaders: { "sec-ch-ua-platform": '"iOS"' } })
  const page = await context.newPage()
  await page.goto("/install")
  await expect(page.locator(".install-download")).toHaveCount(0)
  await page.getByLabel("Install on", { exact: true }).selectOption("windows")
  await page.getByRole("button", { name: "Use this device" }).click()
  await expect(page).toHaveURL(/\/install\?platform=windows$/)
  await expect(page.getByRole("link", { name: "Download for Windows x64" })).toHaveAttribute("href", release + "Multi.Codex_1.8.1_x64-setup.exe")
  await page.getByLabel("Install on", { exact: true }).selectOption("mac")
  await page.getByRole("button", { name: "Use this device" }).click()
  await expect(page.getByRole("link", { name: "Download for Apple Silicon Mac" })).toHaveAttribute("href", release + "Multi.Codex_1.8.1_aarch64.dmg")
  await context.close()
})

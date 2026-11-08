export type InstallPlatform = "linux" | "mac" | "windows"

export function isInstallPlatform(value: unknown): value is InstallPlatform {
  return value === "linux" || value === "mac" || value === "windows"
}

export function detectInstallPlatform(headers: Pick<Headers, "get">): { platform: InstallPlatform | null; message: string } {
  const ua = headers.get("user-agent") || ""
  const hint = headers.get("sec-ch-ua-platform") || ""
  const arch = headers.get("sec-ch-ua-arch") || ""
  const bitness = headers.get("sec-ch-ua-bitness") || ""
  if (/Android|iPhone|iPad|iPod|Mobile/i.test(ua) || /Android|iOS/i.test(hint)) {
    return { platform: null, message: "Multi Codex is a desktop app. Open this same link on your computer, or choose the computer you’re downloading for below." }
  }
  if (/CrOS/i.test(ua) || /Chrome OS/i.test(hint)) {
    return { platform: null, message: "There isn’t a Chromebook desktop package. You can choose another computer below." }
  }
  const platform = /Windows/i.test(hint || ua) ? "windows" : /macOS|Macintosh|Mac OS X/i.test(hint || ua) ? "mac" : /Linux/i.test(hint || ua) ? "linux" : null
  if (platform && platform !== "mac" && /arm|aarch64/i.test(arch + " " + ua)) {
    return { platform: null, message: "There isn’t a native ARM package for this device. Linux and Windows downloads require an x64 computer." }
  }
  if (platform && platform !== "mac" && (bitness.includes("32") || /i[3-6]86|Win32|Windows[^;)]*;[^)]*\bx86\b/i.test(ua))) {
    return { platform: null, message: "The desktop app requires a 64-bit computer. You can choose a supported computer below." }
  }
  return { platform, message: platform ? "Suggested for this browser. Downloading for another computer? Change it below." : "Choose the computer you’re installing on to get its download and setup steps." }
}

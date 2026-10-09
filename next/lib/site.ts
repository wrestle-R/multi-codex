import { releaseHistory } from "./releases"
export const repository = "https://github.com/wrestle-R/multi-codex"
export const latestRelease = `${repository}/releases/latest`
export const vscodeMarketplace = "https://marketplace.visualstudio.com/items?itemName=russeldanielpaul.multi-codex"
export const version = releaseHistory[0].tag.slice(1)
export const windowsInstallerDownload = `${repository}/releases/download/v${version}/Multi.Codex_${version}_x64-setup.exe`
export const installCommand = (platform: "linux" | "mac" | "windows", update = false) => {
  const action = update ? "update" : "install"
  if (platform === "windows") return `Invoke-WebRequest ${latestRelease}/download/${action}-app.ps1 -UseBasicParsing -OutFile "$env:TEMP/${action}-multi-codex.ps1"\npowershell -NoProfile -ExecutionPolicy Bypass -File "$env:TEMP/${action}-multi-codex.ps1"`
  return `curl -fsSL ${latestRelease}/download/${action}-app.sh -o /tmp/${action}-multi-codex.sh\n${platform === "mac" ? "MULTI_CODEX_ALLOW_UNSIGNED_MAC=1 " : ""}bash /tmp/${action}-multi-codex.sh`
}

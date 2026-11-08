import { releaseHistory } from "./releases"
import { stabilityOf } from "./update-channels"
export const repository = "https://github.com/wrestle-R/multi-codex"
export const vscodeMarketplace = "https://marketplace.visualstudio.com/items?itemName=russeldanielpaul.multi-codex"
export const version = releaseHistory.find(release => stabilityOf(release) === "stable")!.tag.slice(1)
export const macVersion = releaseHistory.find(release => stabilityOf(release) === "stable" && release.tag !== "v1.8.0")!.tag.slice(1)
export const macInstallerDownload = `${repository}/releases/download/v${macVersion}/Multi.Codex_${macVersion}_aarch64.dmg`
export const latestRelease = `${repository}/releases/tag/v${version}`
export const installPage = "/install"
export const linuxInstallerDownload = `${repository}/releases/download/v${version}/Multi.Codex_${version}_amd64.AppImage`
export const windowsInstallerDownload = `${repository}/releases/download/v${version}/Multi.Codex_${version}_x64-setup.exe`

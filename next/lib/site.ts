export const repository = "https://github.com/wrestle-R/multi-codex"
export const latestRelease = `${repository}/releases/latest`
export const version = "1.3.7"
export const installCommand = (platform: "linux" | "mac", update = false) => {
  const action = update ? "update" : "install"
  return `curl -fsSL ${latestRelease}/download/${action}-app.sh -o /tmp/${action}-multi-codex.sh\n${platform === "mac" ? "MULTI_CODEX_ALLOW_UNSIGNED_MAC=1 " : ""}bash /tmp/${action}-multi-codex.sh`
}

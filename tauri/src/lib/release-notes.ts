import history from "./release-history.json"

function compare(a: string, b: string) {
  const left = a.replace(/^v/, "").split(".").map(Number)
  const right = b.replace(/^v/, "").split(".").map(Number)
  for (let i = 0; i < 3; i++) if (left[i] !== right[i]) return left[i] - right[i]
  return 0
}

// Bundled summaries remain available offline. Include every known skipped
// release; keep the original upgrade range when notes are reopened later.
export function releaseNotesSince(version: string, previous?: string | null) {
  const available = history.filter(entry => compare(entry.version, version) <= 0)
  const included = previous ? available.filter(entry => compare(entry.version, previous) > 0) : available
  return included.length ? included : available.filter(entry => entry.version === version)
}

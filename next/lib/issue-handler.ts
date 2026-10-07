import { issueBody, validateIssueReport } from "./issue-report"

const githubEndpoint = "https://api.github.com/repos/wrestle-R/multi-codex/issues"
const maxBytes = 16_384
type Dependencies = { token: () => string | undefined; verifyBot: () => Promise<{ isBot: boolean }>; fetch: typeof fetch }
const reply = (error: string, status: number) => Response.json({ error }, { status, headers: { "Cache-Control": "no-store" } })

export function createIssueHandler(deps: Dependencies) {
  return async function POST(request: Request) {
    const url = new URL(request.url)
    // Next's internal URL can use localhost behind a proxy; the Host header is the
    // browser-facing destination. Browser scripts cannot override Host or Origin.
    const host = request.headers.get("host") ?? url.host
    const forwardedProtocol = request.headers.get("x-forwarded-proto")
    const protocol = forwardedProtocol === "https" || forwardedProtocol === "http" ? `${forwardedProtocol}:` : url.protocol
    if (request.headers.get("origin") !== `${protocol}//${host}`) return reply("Please submit the report from this website.", 403)
    if (!/^application\/json(?:\s*;|$)/i.test(request.headers.get("content-type") ?? "")) return reply("Send the report as JSON.", 415)
    if (Number(request.headers.get("content-length")) > maxBytes) return reply("This report is too long. Please shorten it.", 413)
    // Bound streamed bodies too; Content-Length is optional and cannot be trusted.
    const reader = request.body?.getReader()
    if (!reader) return reply("Please complete the required report fields.", 400)
    let bytes = 0
    const chunks: Uint8Array[] = []
    let input: unknown
    try {
      while (true) {
        const { done, value } = await reader.read()
        if (done) break
        bytes += value.byteLength
        if (bytes > maxBytes) { await reader.cancel(); return reply("This report is too long. Please shorten it.", 413) }
        chunks.push(value)
      }
      const buffer = new Uint8Array(bytes)
      let offset = 0
      for (const chunk of chunks) { buffer.set(chunk, offset); offset += chunk.byteLength }
      input = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(buffer))
    } catch { return reply("Please complete the required report fields.", 400) }
    const report = validateIssueReport(input)
    const data = input as Record<string, unknown>
    if (!report || data.publicConsent !== true || data.website !== "") return reply("Please complete the required fields and agree to publish your report.", 400)
    const token = deps.token()
    if (!token) return reply("Direct submission is temporarily unavailable. You can open this report on GitHub instead.", 503)
    try {
      if ((await deps.verifyBot()).isBot) return reply("We could not verify this submission. Please reload the page or open your report on GitHub.", 403)
    } catch { return reply("Submission verification is unavailable. Please try again later or open your report on GitHub.", 503) }
    let response: Response
    try {
      // One attempt only: a timed-out creation may already have reached GitHub.
      response = await deps.fetch(githubEndpoint, { method: "POST", cache: "no-store", signal: AbortSignal.timeout(15_000), headers: { Accept: "application/vnd.github+json", Authorization: `Bearer ${token}`, "X-GitHub-Api-Version": "2026-03-10", "Content-Type": "application/json" }, body: JSON.stringify({ title: report.title, body: issueBody(report) }) })
    } catch { return reply("We could not confirm whether GitHub received your report. Check recent issues before trying again.", 504) }
    if (!response.ok) {
      if (response.status === 429 || (response.status === 403 && (response.headers.has("retry-after") || response.headers.get("x-ratelimit-remaining") === "0"))) return reply("GitHub is receiving too many reports. Please wait before trying again.", 429)
      return reply("GitHub could not accept the report. Your text is still here; you can open it on GitHub instead.", 502)
    }
    try {
      const issue = await response.json()
      if (!Number.isSafeInteger(issue.number) || issue.number <= 0 || issue.html_url !== `https://github.com/wrestle-R/multi-codex/issues/${issue.number}`) throw new Error("Invalid issue response")
      return Response.json({ number: issue.number, url: issue.html_url }, { status: 201, headers: { "Cache-Control": "no-store" } })
    } catch { return reply("GitHub accepted the report but we could not read its link. Check recent issues before trying again.", 502) }
  }
}

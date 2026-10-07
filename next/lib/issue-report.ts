export const issueRepository = "https://github.com/wrestle-R/multi-codex"
export const issuePlatforms = ["Linux", "macOS", "Website", "Other"] as const
export type IssueReport = { title: string; platform: string; version: string; description: string; steps: string; expected: string }

export function validateIssueReport(input: unknown): IssueReport | null {
  if (!input || typeof input !== "object" || Array.isArray(input)) return null
  const data = input as Record<string, unknown>
  const limits = { title: 160, platform: 20, version: 80, description: 6000, steps: 4000, expected: 2000 }
  const fields: Record<string, string> = {}
  for (const [key, limit] of Object.entries(limits)) {
    if (typeof data[key] !== "string" || data[key].length > limit || /[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f]/.test(data[key])) return null
    fields[key] = data[key].trim()
  }
  if (fields.title.length < 5 || /[\r\n]/.test(fields.title) || !issuePlatforms.some(platform => platform === fields.platform) || fields.description.length < 20 || fields.steps.length < 10) return null
  return fields as IssueReport
}

// Keep report text literal: Markdown, images and @mentions inside a report stay inert.
function literal(text: string) {
  const longest = Math.max(2, ...(text.match(/`+/g) ?? []).map(run => run.length))
  const fence = "`".repeat(longest + 1)
  return `${fence}text\n${text}\n${fence}`
}

export function issueBody(report: IssueReport) {
  return ["Submitted through the Multi Codex website.", `## Platform\n${report.platform}`, `## Version\n${literal(report.version || "Not provided")}`, `## What happened\n${literal(report.description)}`, `## Steps to reproduce\n${literal(report.steps)}`, `## Expected behavior\n${literal(report.expected || "Not provided")}`].join("\n\n")
}

export function githubReportLink(report: IssueReport) {
  const params = new URLSearchParams({ title: report.title, body: issueBody(report) })
  return `${issueRepository}/issues/new?${params}`
}

"use client"

import { useEffect, useRef, useState, type FormEvent } from "react"
import { githubReportLink, issuePlatforms, issueRepository, type IssueReport } from "@/lib/issue-report"

const blank: IssueReport = { title: "", platform: "Linux", version: "", description: "", steps: "", expected: "" }

export function IssueForm() {
  const [report, setReport] = useState<IssueReport>(blank)
  const [sending, setSending] = useState(false)
  const [error, setError] = useState("")
  const [created, setCreated] = useState<{ number: number; url: string } | null>(null)
  const inFlight = useRef(false)
  const resultRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (created) resultRef.current?.focus()
  }, [created])

  function update(key: keyof IssueReport, value: string) {
    setReport(previous => ({ ...previous, [key]: value }))
  }

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (inFlight.current) return
    inFlight.current = true
    setSending(true)
    setError("")
    const fields = new FormData(event.currentTarget)
    try {
      const response = await fetch("/api/issues", { method: "POST", signal: AbortSignal.timeout(30_000), headers: { "Content-Type": "application/json" }, body: JSON.stringify({ ...report, publicConsent: fields.get("publicConsent") === "on", website: fields.get("website") ?? "" }) })
      const data = await response.json()
      if (!response.ok) {
        setError(typeof data.error === "string" ? data.error : "We could not submit your report. Your text is still here.")
        return
      }
      if (!Number.isSafeInteger(data.number) || data.number <= 0 || data.url !== `${issueRepository}/issues/${data.number}`) throw new Error("Invalid issue link")
      setCreated(data)
    } catch {
      setError("We could not confirm the submission. Your text is still here. Check existing issues before trying again.")
    } finally {
      inFlight.current = false
      setSending(false)
    }
  }

  if (created) return <div className="issue-confirmation" role="status" tabIndex={-1} ref={resultRef}><span className="confirmation-mark" aria-hidden="true">✓</span><p className="eyebrow">REPORT RECEIVED</p><h2>Issue #{created.number}<br />is on GitHub.</h2><p>Thanks for the details. You can follow updates or add more information in the issue.</p><a className="button primary" href={created.url}>View your issue <span aria-hidden="true">↗</span></a><button type="button" className="text-link" onClick={() => { setCreated(null); setReport(blank); setError("") }}>Report another issue</button></div>

  return <form className="issue-form" onSubmit={submit} aria-label="Report an issue" aria-busy={sending}>
    <div className="issue-form-heading"><span className="section-index">THE DETAILS</span><p><span aria-hidden="true">*</span> Required fields</p></div>
    <fieldset disabled={sending}>
      <div className="issue-field"><label htmlFor="issue-title">A short title <span aria-hidden="true">*</span></label><input id="issue-title" name="title" value={report.title} onChange={event => update("title", event.target.value)} required minLength={5} maxLength={160} placeholder="CLI closes when I launch an account" /></div>
      <div className="issue-field-row"><div className="issue-field"><label htmlFor="issue-platform">Platform <span aria-hidden="true">*</span></label><select id="issue-platform" name="platform" value={report.platform} onChange={event => update("platform", event.target.value)} required>{issuePlatforms.map(platform => <option key={platform}>{platform}</option>)}</select></div><div className="issue-field"><label htmlFor="issue-version">App version <span className="optional">Optional</span></label><input id="issue-version" name="version" value={report.version} onChange={event => update("version", event.target.value)} maxLength={80} placeholder="e.g. 1.3.8" /></div></div>
      <div className="issue-field"><label htmlFor="issue-description">What happened? <span aria-hidden="true">*</span></label><textarea id="issue-description" name="description" value={report.description} onChange={event => update("description", event.target.value)} required minLength={20} maxLength={6000} rows={5} placeholder="Describe the problem and any error you saw." /></div>
      <div className="issue-field"><label htmlFor="issue-steps">Steps to reproduce <span aria-hidden="true">*</span></label><textarea id="issue-steps" name="steps" value={report.steps} onChange={event => update("steps", event.target.value)} required minLength={10} maxLength={4000} rows={3} placeholder={"1. Open Multi Codex\n2. Choose an account\n3. …"} /></div>
      <div className="issue-field"><label htmlFor="issue-expected">What did you expect? <span className="optional">Optional</span></label><textarea id="issue-expected" name="expected" value={report.expected} onChange={event => update("expected", event.target.value)} maxLength={2000} rows={3} /></div>
      <div className="issue-trap" aria-hidden="true"><label htmlFor="issue-website">Website</label><input id="issue-website" name="website" tabIndex={-1} autoComplete="off" /></div>
      <label className="issue-consent"><input type="checkbox" name="publicConsent" required /><span>I understand this report will be published publicly on GitHub.</span></label>
    </fieldset>
    {error ? <div className="issue-error" role="alert"><p>{error}</p><a href={`${issueRepository}/issues`}>Check existing issues ↗</a></div> : null}
    <div className="issue-submit"><button className="button primary" type="submit" disabled={sending}>{sending ? "Submitting…" : "Submit issue"}<span aria-hidden="true">↗</span></button><a className="text-link" href={githubReportLink(report)}>Open on GitHub instead</a></div>
    <p className="issue-form-footnote">After submission, you’ll get a link to your issue. You don’t need a GitHub account to send this form.</p>
  </form>
}

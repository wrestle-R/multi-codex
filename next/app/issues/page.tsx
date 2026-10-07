import type { Metadata } from "next"
import { IssueForm } from "@/components/issue-form"
import { repository } from "@/lib/site"

export const metadata: Metadata = { title: "Report an issue", description: "Report a Multi Codex bug directly to the project's GitHub issue tracker." }

export default function Issues() {
  return <main id="main" className="issue-page page-width">
    <div className="issue-intro"><p className="eyebrow">HELP MAKE IT BETTER</p><h1>Something<br />out of place?</h1><p>Tell us what happened. Your report goes straight to the Multi Codex issue tracker, where we can follow up.</p><a className="text-link" href={`${repository}/issues`}>Browse existing issues <span aria-hidden="true">↗</span></a><div className="issue-guidance"><span className="section-index">A GOOD REPORT GOES A LONG WAY</span><p>Include the steps we can follow, your platform, and the app version. Paste only the relevant part of an error.</p><p>Reports are public. Leave out passwords, login tokens, <code>auth.json</code>, and private conversation data.</p></div></div>
    <IssueForm />
  </main>
}

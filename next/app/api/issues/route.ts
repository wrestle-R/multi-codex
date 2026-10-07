import { checkBotId } from "botid/server"
import { createIssueHandler } from "@/lib/issue-handler"

export const runtime = "nodejs"
export const maxDuration = 30
export const POST = createIssueHandler({
  token: () => process.env.GITHUB_ISSUES_TOKEN,
  verifyBot: () => checkBotId({ advancedOptions: { checkLevel: "basic" } }),
  fetch: (...args) => fetch(...args),
})

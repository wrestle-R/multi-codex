import { test, expect } from "@playwright/test"
import { createIssueHandler } from "../lib/issue-handler"
const report = { title: "CLI closes immediately", platform: "Linux", version: "1.3.8", description: "The terminal closes immediately after launch.", steps: "1. Open the app. 2. Launch CLI.", expected: "The terminal stays open.", website: "", publicConsent: true }
const request = (body: unknown = report, origin = "https://multi-codex.vercel.app") => new Request("https://multi-codex.vercel.app/api/issues", { method: "POST", headers: { origin, "Content-Type": "application/json" }, body: JSON.stringify(body) })

test("issue API creates one issue in the fixed repository with server-only authorization", async () => {
  const calls: { url: string; options?: RequestInit }[] = []
  const handler = createIssueHandler({ token: () => "private-test-token", verifyBot: async () => ({ isBot: false }), fetch: async (url, options) => { calls.push({ url: String(url), options }); return Response.json({ number: 42, html_url: "https://github.com/wrestle-R/multi-codex/issues/42", token: "must-not-return" }, { status: 201 }) } })
  const response = await handler(request({ ...report, repository: "attacker/other", description: report.description + "\n```\n@someone ![tracker](https://example.com/a)" }))
  expect(response.status).toBe(201)
  expect(await response.json()).toEqual({ number: 42, url: "https://github.com/wrestle-R/multi-codex/issues/42" })
  expect(calls).toHaveLength(1)
  expect(calls[0].url).toBe("https://api.github.com/repos/wrestle-R/multi-codex/issues")
  expect((calls[0].options?.headers as Record<string, string>).Authorization).toBe("Bearer private-test-token")
  expect(JSON.parse(calls[0].options!.body as string).body).toContain("````text\nThe terminal")
})

test("invalid, cross-origin, oversized and unconsented reports never reach GitHub", async () => {
  let calls = 0
  const handler = createIssueHandler({ token: () => "private-test-token", verifyBot: async () => ({ isBot: false }), fetch: async () => { calls++; throw new Error("Must not call GitHub") } })
  for (const [body, status] of [[{ ...report, publicConsent: false }, 400], [{ ...report, website: "spam" }, 400], [{ ...report, title: "x" }, 400], [{ ...report, platform: "<script>" }, 400], [{ ...report, description: "x".repeat(16_385) }, 413], [null, 400]] as const) expect((await handler(request(body))).status).toBe(status)
  expect((await handler(request(report, "https://attacker.example"))).status).toBe(403)
  expect((await handler(new Request("https://multi-codex.vercel.app/api/issues", { method: "POST", headers: { origin: "https://multi-codex.vercel.app", "Content-Type": "application/json" }, body: "{" }))).status).toBe(400)
  expect(calls).toBe(0)
})

test("missing credentials, bots and verification failures fail closed", async () => {
  let calls = 0
  const mockFetch: typeof fetch = async () => { calls++; throw new Error("Must not call GitHub") }
  for (const [token, verifyBot, status] of [[() => undefined, async () => ({ isBot: false }), 503], [() => "private-test-token", async () => ({ isBot: true }), 403], [() => "private-test-token", async () => { throw new Error("private verification details") }, 503]] as const) {
    const response = await createIssueHandler({ token, verifyBot, fetch: mockFetch })(request())
    expect(response.status).toBe(status)
    expect(await response.text()).not.toContain("private")
  }
  expect(calls).toBe(0)
})

test("upstream failures do not leak secrets, retry uncertain writes or return untrusted links", async () => {
  for (const [upstream, status] of [[async () => new Response("private-token", { status: 401 }), 502], [async () => new Response("private-token", { status: 429 }), 429], [async () => new Response("private-token", { status: 403, headers: { "x-ratelimit-remaining": "0" } }), 429], [async () => { throw new Error("private-token timeout") }, 504], [async () => Response.json({ number: 2, html_url: "https://attacker.example" }, { status: 201 }), 502]] as const) {
    let calls = 0
    const response = await createIssueHandler({ token: () => "private-token", verifyBot: async () => ({ isBot: false }), fetch: async () => { calls++; return upstream() } })(request())
    expect(response.status).toBe(status)
    expect(await response.text()).not.toContain("private-token")
    expect(calls).toBe(1)
  }
})

test("same-origin validation uses the browser-facing host behind Next's proxy", async () => {
  let calls = 0
  const handler = createIssueHandler({ token: () => "private-test-token", verifyBot: async () => ({ isBot: false }), fetch: async () => { calls++; return Response.json({ number: 3, html_url: "https://github.com/wrestle-R/multi-codex/issues/3" }, { status: 201 }) } })
  const proxied = new Request("http://localhost:3000/api/issues", { method: "POST", headers: { host: "multi-codex.vercel.app", "x-forwarded-proto": "https", origin: "https://multi-codex.vercel.app", "Content-Type": "application/json" }, body: JSON.stringify(report) })
  expect((await handler(proxied)).status).toBe(201)
  expect(calls).toBe(1)
})

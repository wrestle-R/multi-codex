# Multi Codex website

Small Next.js showcase and documentation site. Static documentation, bundled local fonts and screenshots, plus a server-side GitHub issue submission endpoint. The site does not read local Multi Codex account data.

```sh
npm ci
npm run build
npm test
npm run dev
```

Playwright checks the production build on desktop Chromium, desktop WebKit and mobile Chromium. Before the first test run, use `npx playwright install chromium webkit` (Linux may also need browser system dependencies).

Deploy from this directory to the dedicated `multi-codex` Vercel project after local and macOS/Linux CI checks pass. Run `vercel project inspect` here before deployment to confirm the intended team/project. `.vercel` is ignored. Release links resolve to GitHub’s latest stable packages; the displayed changelog version is in `lib/site.ts`.

## Issue submissions

`/issues` submits to `/api/issues`, which creates an issue in `wrestle-R/multi-codex` and returns the issue number and link. Visitors do not need a GitHub account. The form explains that reports are public and requires consent. Requests must pass same-origin checks, bounded-body validation, the honeypot and Vercel BotID Basic verification. The GitHub repository is fixed on the server.

Set **`GITHUB_ISSUES_TOKEN`** as a server-only Production secret in the dedicated Vercel project. Use a fine-grained GitHub token restricted to this repository with **Issues: read and write**. Never prefix it with `NEXT_PUBLIC_` or put it in a committed file. When rotating it, replace the Vercel secret and redeploy. For local development, provide a token through the process environment or an ignored local environment file; BotID uses its documented development behavior only outside production.

If the credential is missing or GitHub is unavailable, submission returns an error and preserves the report. “Open on GitHub instead” opens GitHub's issue composer with the report filled in. A timeout does not trigger an automatic retry because GitHub may already have created the issue. API and browser tests use fake credentials and mocked creation responses; they do not create public issues in CI.

References: [GitHub issue API](https://docs.github.com/en/rest/issues/issues#create-an-issue), [Vercel BotID setup](https://vercel.com/docs/botid/get-started).

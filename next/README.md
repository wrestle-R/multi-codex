# Multi Codex website

Small Next.js showcase and documentation site. Static pages, bundled local fonts and screenshots; no account credentials or backend services.

```sh
npm ci
npm run build
npm test
npm run dev
```

Playwright checks the production build on desktop Chromium, desktop WebKit and mobile Chromium. Before the first test run, use `npx playwright install chromium webkit` (Linux may also need browser system dependencies).

Deploy from this directory to the dedicated `multi-codex` Vercel project after local and macOS/Linux CI checks pass. Run `vercel project inspect` here before deployment to confirm the intended team/project. `.vercel` is ignored. Release links resolve to GitHub’s latest stable packages; the displayed changelog version is in `lib/site.ts`.

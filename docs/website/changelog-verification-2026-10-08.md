# Changelog verification — October 8, 2026

The website groups selected updates into six release series, newest first. Each version entry includes its publication date, change summary, permalink and full release-note link. The website, README and current application package identify v1.3.8 as the latest version.

## Checks

- Website production build and TypeScript checks passed.
- All 38 local Chromium and mobile Chromium checks passed, including issue-form and API regressions.
- [Linux and Mac CI](https://github.com/wrestle-R/multi-codex/actions/runs/37732607598) passed, including website checks in Chromium, WebKit and mobile Chromium on both runners.
- All 20 production website checks passed against `https://multi-codex.vercel.app`, covering navigation, release links, version anchors, installation commands, screenshots, small-screen layout and theme behavior.
- All 78 app frontend tests, 82 local Rust tests, six installer/release regression checks and release packaging checks passed.
- GitHub release metadata checks confirmed 27 version tags, 22 release records, matching asset names, original publication dates and existing draft states.

## Production website

[Deployment](https://vercel.com/russeldanielpaul-gmailcoms-projects/multi-codex/GGeBfW8tHtmzcHn28bjvXDUFiFw9) is ready and aliased to [the public website](https://multi-codex.vercel.app).

Production browser command, run from `next/`:

```sh
BASE_URL=https://multi-codex.vercel.app npx playwright test tests/site.spec.ts tests/theme.spec.ts --project=chromium --project=mobile
```

Local WebKit cannot start on this host because legacy shared libraries are missing. WebKit coverage passed on both CI runners.

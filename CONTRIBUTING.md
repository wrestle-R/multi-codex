# Release and contribution guidelines

Use clear, verifiable descriptions of what changed and how it affects someone using Multi Codex. The website is the source of truth for detailed release information. GitHub hosts package downloads and a short link to the corresponding website release page. Keep package versions and website documentation consistent.

## Version numbers

For future stable releases, use `MAJOR.MINOR.PATCH` and prefix Git tags with `v`:

- Increase the major version for incompatible changes to supported behavior or interfaces, including a platform requirement that makes an existing installation unsupported. Include migration instructions.
- Increase the minor version for backward-compatible features. Reset the patch number to zero.
- Increase the patch version for backward-compatible bug fixes and maintenance. Small interface refinements can use a patch version when they do not introduce a new feature.

For example, after `1.3.8`, a bug fix would be `1.3.9`, a new compatible feature would be `1.4.0`, and an incompatible change would be `2.0.0`. These examples are not a release schedule.

Test-only and documentation-only changes do not normally need an app release. Versions identify packages; they do not count commits. Keep prerelease builds separate from the stable download and website's latest-version indicator.

Treat stable versions as immutable. A later code or package change gets a new version rather than reusing a published version number.

## Changelog entries

The website at `/releases` presents selected user-facing updates grouped by major/minor series, newest first. A series summary describes the combined capabilities of that series; each version entry describes only changes included in that version.

- Use concrete titles, such as “Workspace folder picker” or “Linux package checksum correction.”
- Explain the affected behavior and result. Avoid slogans, unsupported guarantees and internal test details in the short website summary.
- Use the published release date. Do not substitute the commit date or documentation update date.
- Link each version to its full website page at `/releases/v<VERSION>`. Preserve existing changelog anchors and published dates.
- Keep package assets on GitHub. Release descriptions contain only a short link to the website page. Do not use the README as a changelog or duplicate full notes into GitHub descriptions.
- Store full public notes under `next/content/releases/v<VERSION>.md`, rendered by the website. Include platform requirements, exact changes, upgrade instructions, known limitations and validation evidence. Keep unpublished work and draft validation clearly separate from published releases.

## Commit titles

Use `type(scope): description` when a scope helps, or `type: description` otherwise. Write a short imperative description of the actual change, without a trailing period.

- `feat(launch): add preferred CLI terminal selection`
- `fix(auth): preserve account isolation on repeated launches`
- `docs(releases): clarify changelog grouping and versioning`
- `test(installers): cover missing release assets`
- `chore(release): prepare version 1.3.9`

Use `feat` for capabilities, `fix` for defects, `docs` for documentation, `test` for test coverage and `chore` for maintenance. Mark incompatible changes with `!` and explain them in the commit body and release notes. Describe validation in the commit body or pull request instead of crowding the title.

## Release checklist

1. Choose the next version based on the changes since the last stable release.
2. Synchronize the app package and lockfile, Tauri configuration, Rust package and lockfile, website package and lockfile, and `next/lib/site.ts`. The release workflow checks the app version against its tag.
3. Write full release details in `next/content/releases/v<VERSION>.md`. Preview the website page and check its changes, installation/update instructions, compatibility requirements, limitations and evidence. Do not claim checks that have not passed.
4. Complete build, installer, signed-updater and functional release gates before publishing. Configure the GitHub Actions `TAURI_SIGNING_PRIVATE_KEY` secret and optional password. The matching public key is in the Tauri config; private signing material never belongs in Git or website content. Signing the update payload is separate from Apple Developer ID signing/notarization.
5. After publication, update `next/lib/releases.ts` with the actual publication timestamp and concise highlights. Add a series summary when a new major/minor series appears.
6. Verify the deployed website release page, badge, version anchors and installation commands. Publish GitHub assets with a short link to that page. Keep the README focused on project setup and installation, linking to the website changelog.

## Website verification

From `next/`, run `npm run build`, `npm run typecheck`, and `npx playwright test tests/site.spec.ts`. The browser checks cover grouping, chronological order, existing version permalinks, the latest download link and small-screen layout. Website copy and layout updates do not require rebuilding desktop packages.

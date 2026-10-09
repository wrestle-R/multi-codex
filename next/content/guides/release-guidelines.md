# Release and contribution guidelines

Use clear, verifiable descriptions of what changed and how it affects someone using Multi Codex. Keep package versions, release notes, the README and the website consistent.

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
- Include a link to the version's full GitHub release notes. Preserve existing version permalinks when editing copy or layout.
- Keep full package history on GitHub. Do not describe a selection of updates as every published version or invent entries to fill gaps.
- Put platform requirements, known limitations and links to validation evidence in the full notes under `release/notes/`.

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
3. Write full release notes in `release/notes/v<VERSION>.md`, including installation changes, compatibility requirements and known limitations. Link evidence without claiming checks that have not passed.
4. Complete the project's build, installer and release validation gates before publishing the stable release.
5. After publication, update `next/lib/releases.ts` with the actual publication timestamp and concise highlights. Add a series summary when a new major/minor series appears.
6. Align the README's current version and latest-change summary with the stable release, and verify the website badge, release-note links, version anchors and installation commands.

## Website verification

From `next/`, run `npm run build`, `npm run typecheck`, and `npx playwright test tests/site.spec.ts`. The browser checks cover grouping, chronological order, existing version permalinks, the latest download link and small-screen layout. Website copy and layout updates do not require rebuilding desktop packages.

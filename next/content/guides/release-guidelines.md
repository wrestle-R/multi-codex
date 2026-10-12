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
6. Publish the validated draft manually with the short website link using the commands below. Verify the deployed website release page, badge, version anchors and installation commands. Keep the README focused on project setup and installation, linking to the website changelog.

## Automation and manual publication

Ordinary branch pushes do not run desktop CI, Windows packaging or extension packaging. Version-tag pushes matching `v*.*.*` start the Release workflow, which builds and tests native packages and creates a **draft** after its checks pass. A tag must match the committed stable app version. The draft is not automatically published. CI remains available for pull requests and explicit manual runs; individual platform workflows can also be called by Release or started manually for investigation.

Website deployments are separate: keep Vercel Git deployments enabled for ordinary pushes. A branch push can update the website without building or publishing a desktop release. Production follows `main`; other branches can create previews.

After recording truthful native evidence and exact hashes in `release/evidence/v<VERSION>-validation.json`, download and verify the draft before publishing it manually from the repository root. This snippet intentionally refuses to edit an already-published stable release. No Actions workflow writes the final GitHub description.

```bash
set -e
VERSION=$(node -p 'JSON.parse(require("fs").readFileSync("tauri/package.json", "utf8")).version')
TAG="v$VERSION"
[[ "$(gh release view "$TAG" --json isDraft --jq '.isDraft')" == "true" ]]
RELEASE_DIR=$(mktemp -d)
gh release download "$TAG" --dir "$RELEASE_DIR"
node tauri/scripts/release-gates.mjs "$RELEASE_DIR"
node tauri/scripts/updater-manifest.mjs "$RELEASE_DIR" "$VERSION" --check
if command -v sha256sum >/dev/null 2>&1; then
  (cd "$RELEASE_DIR" && sha256sum --check SHA256SUMS)
else
  (cd "$RELEASE_DIR" && shasum -a 256 -c SHA256SUMS)
fi
test -f "next/content/releases/$TAG.md"
printf 'Full release details: https://multi-codex.vercel.app/releases/%s\n' "$TAG" > "$RELEASE_DIR/release-link.md"
gh release edit "$TAG" --draft=false --prerelease=false --latest \
  --title "Multi Codex $TAG" --notes-file "$RELEASE_DIR/release-link.md"
```

Verify updater payload signatures against the configured public key as part of release evidence. The manifest/checksum checks above complement native functional and signature checks; do not fabricate passed evidence or skip failed checks. After publication, use GitHub's actual publication timestamp in the website history and push that website update.

## Website verification

From `next/`, run `npm run build`, `npm run typecheck`, and `npx playwright test tests/site.spec.ts`. The browser checks cover grouping, chronological order, existing version permalinks, the latest download link and small-screen layout. Website copy and layout updates do not require rebuilding desktop packages.

import { createHash } from 'node:crypto'
import { readFileSync, existsSync } from 'node:fs'
import { basename, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = fileURLToPath(new URL('../../', import.meta.url))
export const requiredChecks = [
  'packaged-macos-27-arm64', 'packaged-macos-26-arm64', 'macos-signing-notarization',
  'packaged-debian-ubuntu', 'packaged-fedora', 'packaged-arch',
  'hyprland-placement', 'gnome-wayland-placement', 'gnome-x11-placement',
  'kde-wayland-placement', 'kde-x11-placement',
]

export function evaluate(report, version, artifactDirectory) {
  const failures = []
  if (report.version !== version || report.schemaVersion !== 1) failures.push('Validation report version/schema does not match the candidate')
  for (const id of requiredChecks) {
    const check = report.checks?.[id]
    if (check?.status !== 'passed') failures.push(`${id}: ${check?.reason || 'functional validation is pending'}`)
    else if (!check.evidence || !check.osVersion || !check.testedBy || !check.testedAt || Number.isNaN(Date.parse(check.testedAt))) failures.push(`${id}: evidence, exact OS version, tester and timestamp are required`)
  }
  const assets = [
    `Multi.Codex_${version}_amd64.AppImage`,
    `Multi.Codex_${version}_amd64.deb`,
    `Multi.Codex-${version}-1.x86_64.rpm`,
    `Multi.Codex_${version}_aarch64.dmg`,
  ]
  for (const name of assets) {
    const expected = report.artifacts?.[name]
    if (!/^[a-f0-9]{64}$/.test(expected ?? '')) failures.push(`${name}: validated artifact SHA256 is missing`)
    if (artifactDirectory) {
      const path = resolve(artifactDirectory, name)
      if (!existsSync(path)) failures.push(`${name}: packaged artifact is missing`)
      else if (createHash('sha256').update(readFileSync(path)).digest('hex') !== expected) failures.push(`${name}: artifact differs from the tested candidate`)
    }
  }
  return failures
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const version = JSON.parse(readFileSync(resolve(root, 'tauri/package.json'), 'utf8')).version
  const report = JSON.parse(readFileSync(resolve(root, `docs/releases/v${version}-validation.json`), 'utf8'))
  const failures = evaluate(report, version, process.argv[2])
  if (failures.length) {
    console.error(`Release v${version} is blocked:\n${failures.map(f => `- ${f}`).join('\n')}`)
    process.exitCode = 1
  } else console.log(`Release v${version}: all required functional checks and artifact hashes passed`)
}

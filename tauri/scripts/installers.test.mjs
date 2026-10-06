import { test } from 'node:test'
import assert from 'node:assert/strict'
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, existsSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { spawnSync } from 'node:child_process'
import { createHash } from 'node:crypto'

const root = fileURLToPath(new URL('../../', import.meta.url))
for (const script of ['install-app.sh', 'update-app.sh']) {
  test(`${script} chooses compatible assets and rejects unsupported hosts before installation`, () => {
    const task = mkdtempSync(join(tmpdir(), 'multi-codex-installer-'))
    try {
      const bin = join(task, 'bin'); mkdirSync(bin)
      writeFileSync(join(bin, 'uname'), '#!/bin/sh\nif [ "$1" = "-s" ]; then echo "$MOCK_SYSTEM"; else echo "$MOCK_ARCH"; fi\n', { mode: 0o755 })
      writeFileSync(join(bin, 'sw_vers'), '#!/bin/sh\necho "$MOCK_MACOS"\n', { mode: 0o755 })
      // Never downloads or executes a package: exit after recording the chosen asset.
      writeFileSync(join(bin, 'curl'), '#!/bin/sh\ncase "$*" in *url_effective*) echo "https://github.com/wrestle-R/multi-codex/releases/tag/$MOCK_TAG";; *) printf "%s\\n" "$*" > "$MOCK_REQUEST"; exit 56;; esac\n', { mode: 0o755 })
      for (const [system, arch, osVersion, tag, expectedAsset, expectedCode] of [
        ['Linux', 'x86_64', '', 'v1.3.0', 'Multi.Codex_1.3.0_amd64.AppImage', 56],
        ['Linux', 'aarch64', '', 'v1.3.0', null, 1],
        ['Darwin', 'arm64', '27.0.1', 'v1.3.0', 'Multi.Codex_1.3.0_aarch64.dmg', 56],
        ['Darwin', 'arm64', '25.0', 'v1.3.0', null, 1],
        ['Darwin', 'x86_64', '26.0', 'v1.3.0', null, 1],
        ['Darwin', 'x86_64', '15.0', 'v1.2.3', 'Multi.Codex_1.2.3_universal.dmg', 56],
      ]) {
        const request = join(task, 'request')
        rmSync(request, { force: true })
        const result = spawnSync('/bin/bash', [join(root, 'scripts', script)], { env: { ...process.env, PATH: `${bin}:${process.env.PATH}`, HOME: task, MOCK_SYSTEM: system, MOCK_ARCH: arch, MOCK_MACOS: osVersion, MOCK_TAG: tag, MOCK_REQUEST: request }, encoding: 'utf8' })
        assert.equal(result.status, expectedCode, `${system}/${arch}/${tag}: ${result.stderr}`)
        if (expectedAsset) assert.ok(readFileSync(request, 'utf8').includes(expectedAsset))
        else assert.equal(existsSync(request), false, 'must reject before any asset download')
        assert.equal(existsSync(join(task, 'Applications')), false, 'must not alter an installed app')
      }
    } finally { rmSync(task, { recursive: true, force: true }) }
  })
}

for (const script of ['install-app.sh', 'update-app.sh']) {
  test(`${script} verifies downloads before replacing the app and preserves account data`, () => {
    const task = mkdtempSync(join(tmpdir(), 'multi-codex-installer-'))
    try {
      const bin = join(task, 'bin'); mkdirSync(bin)
      const temporary = join(task, 'temporary'); mkdirSync(temporary)
      const payload = join(task, 'release.AppImage')
      writeFileSync(payload, '#!/bin/sh\nexit 0\n')
      const checksum = createHash('sha256').update(readFileSync(payload)).digest('hex')
      writeFileSync(join(bin, 'uname'), '#!/bin/sh\nif [ "$1" = "-s" ]; then echo Linux; else echo x86_64; fi\n', { mode: 0o755 })
      writeFileSync(join(bin, 'curl'), `#!/bin/bash
if [[ "$*" == *url_effective* ]]; then
  printf 'https://github.com/wrestle-R/multi-codex/releases/tag/v1.3.5\\n'
  exit 0
fi
output=""
manifest=0
while (( $# )); do
  case "$1" in
    --output) output="$2"; shift 2 ;;
    */SHA256SUMS) manifest=1; shift ;;
    *) shift ;;
  esac
done
if (( manifest )); then
  printf '%s  Multi.Codex_1.3.5_amd64.AppImage\\n' "$MOCK_CHECKSUM" > "$output"
else
  cp "$MOCK_PAYLOAD" "$output"
fi
`, { mode: 0o755 })
      const appDir = join(task, '.local/bin'); mkdirSync(appDir, { recursive: true })
      const destination = join(appDir, 'multi-codex.AppImage')
      const profileDir = join(task, '.local/share/multi-codex/profiles/keep/codex-home')
      mkdirSync(profileDir, { recursive: true })
      const preserved = [
        join(task, '.local/share/multi-codex/profiles.json'),
        join(profileDir, 'auth.json'),
        join(profileDir, 'history.jsonl'),
      ]
      for (const file of preserved) writeFileSync(file, 'private test sentinel')
      for (const [expectedChecksum, expectedStatus] of [['0'.repeat(64), 1], [checksum, 0]]) {
        writeFileSync(destination, 'previous app')
        const result = spawnSync('/bin/bash', [join(root, 'scripts', script)], {
          env: { ...process.env, PATH: `${bin}:${process.env.PATH}`, HOME: task, TMPDIR: temporary, MULTI_CODEX_NO_LAUNCH: '1', MOCK_CHECKSUM: expectedChecksum, MOCK_PAYLOAD: payload, staged: profileDir },
          encoding: 'utf8',
        })
        assert.equal(result.status, expectedStatus, result.stderr)
        assert.equal(readFileSync(destination, 'utf8'), expectedStatus === 0 ? readFileSync(payload, 'utf8') : 'previous app')
        for (const file of preserved) assert.equal(readFileSync(file, 'utf8'), 'private test sentinel')
      }
    } finally { rmSync(task, { recursive: true, force: true }) }
  })
}

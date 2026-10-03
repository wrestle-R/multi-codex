import { test } from 'node:test'
import assert from 'node:assert/strict'
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, existsSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { spawnSync } from 'node:child_process'

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

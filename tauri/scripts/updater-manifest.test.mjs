import { test } from 'node:test'
import assert from 'node:assert/strict'
import { mkdtempSync, writeFileSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { createUpdaterManifest } from './updater-manifest.mjs'

test('only manifests the matching signed artifacts for all three desktop platforms', () => {
  const directory = mkdtempSync(join(tmpdir(), 'multi-codex-updater-'))
  try {
    assert.throws(() => createUpdaterManifest(directory, '1.5.0'), /Missing updater artifact/)
    const names = ['Multi.Codex_1.5.0_amd64.AppImage', 'Multi.Codex_1.5.0_aarch64.app.tar.gz', 'Multi.Codex_1.5.0_x64-setup.exe']
    for (const name of names) {
      writeFileSync(join(directory, name), 'disposable package')
      writeFileSync(join(directory, `${name}.sig`), Buffer.from('untrusted comment: synthetic signature fixture').toString('base64'))
    }
    const manifest = createUpdaterManifest(directory, '1.5.0')
    assert.equal(manifest.version, '1.5.0')
    assert.equal(Object.keys(manifest.platforms).length, 3)
    assert.equal(manifest.platforms['linux-x86_64'].url, 'https://github.com/wrestle-R/multi-codex/releases/download/v1.5.0/Multi.Codex_1.5.0_amd64.AppImage')
    assert.ok(manifest.notes.includes('/releases/v1.5.0'))
    writeFileSync(join(directory, `${names[0]}.sig`), '')
    assert.throws(() => createUpdaterManifest(directory, '1.5.0'), /invalid updater signature/)
    assert.throws(() => createUpdaterManifest(directory, '../../other'), /stable package version/)
  } finally { rmSync(directory, { recursive: true, force: true }) }
})

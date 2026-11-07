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

test('the Linux and Windows release does not offer a new macOS updater', () => {
  const directory = mkdtempSync(join(tmpdir(), 'multi-codex-held-mac-'))
  try {
    for (const name of ['Multi.Codex_1.8.0_amd64.AppImage', 'Multi.Codex_1.8.0_x64-setup.exe']) {
      writeFileSync(join(directory, name), 'fixture')
      writeFileSync(join(directory, `${name}.sig`), Buffer.from('untrusted comment: fixture').toString('base64'))
    }
    const manifest = createUpdaterManifest(directory, '1.8.0')
    assert.deepEqual(Object.keys(manifest.platforms), ['linux-x86_64', 'windows-x86_64'])
  } finally { rmSync(directory, { recursive: true, force: true }) }
})


test('1.8.1 requires a signed Mac updater alongside Linux and Windows', () => {
  const directory = mkdtempSync(join(tmpdir(), 'multi-codex-full-platform-'))
  try {
    for (const name of ['Multi.Codex_1.8.1_amd64.AppImage','Multi.Codex_1.8.1_x64-setup.exe']) {
      writeFileSync(join(directory,name),'fixture')
      writeFileSync(join(directory,`${name}.sig`),Buffer.from('untrusted comment: fixture').toString('base64'))
    }
    assert.throws(() => createUpdaterManifest(directory,'1.8.1'),/Missing updater artifact/)
    const mac = 'Multi.Codex_1.8.1_aarch64.app.tar.gz'
    writeFileSync(join(directory,mac),'fixture')
    writeFileSync(join(directory,`${mac}.sig`),Buffer.from('untrusted comment: fixture').toString('base64'))
    assert.deepEqual(Object.keys(createUpdaterManifest(directory,'1.8.1').platforms),['linux-x86_64','darwin-aarch64','windows-x86_64'])
  } finally { rmSync(directory,{recursive:true,force:true}) }
})

import { test } from 'node:test'
import assert from 'node:assert/strict'
import { mkdtempSync, writeFileSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { createHash } from 'node:crypto'
import { evaluate, requiredChecks } from './release-gates.mjs'

function releaseChecks() {
  return Object.fromEntries(requiredChecks.map(id => [id, { status: 'passed', evidence: 'record', environment: 'test runner', testedBy: 'tester', testedAt: '2026-10-03T00:00:00Z' }]))
}

test('blocks failed Mac installation even when other publication checks pass', () => {
  const checks = releaseChecks()
  checks['macos-installer'] = { status: 'blocked', reason: 'Mac install failed' }
  assert.ok(evaluate({ version: '1.3.0', schemaVersion: 1, releaseChecks: checks, artifacts: {} }, '1.3.0').some(error => error.includes('Mac install failed')))
})

test('requires the exact packages that were functionally tested', () => {
  const directory = mkdtempSync(join(tmpdir(), 'multi-codex-gates-'))
  try {
    const names = ['Multi.Codex_1.3.0_amd64.AppImage', 'Multi.Codex_1.3.0_amd64.deb', 'Multi.Codex-1.3.0-1.x86_64.rpm', 'Multi.Codex_1.3.0_aarch64.dmg', 'install-app.sh', 'update-app.sh']
    const artifacts = {}
    for (const name of names) {
      writeFileSync(join(directory, name), 'tested package')
      artifacts[name] = createHash('sha256').update('tested package').digest('hex')
    }
    const report = { version: '1.3.0', schemaVersion: 1, releaseChecks: releaseChecks(), artifacts }
    assert.deepEqual(evaluate(report, '1.3.0', directory), [])
    report.checks = { 'macos-signing-notarization': { status: 'pending' }, 'packaged-fedora': { status: 'pending' } }
    assert.deepEqual(evaluate(report, '1.3.0', directory), [], 'Optional signing and unfinished wider validation must not block publication')
    const comprehensive = evaluate(report, '1.3.0', directory, { fullValidation: true })
    assert.ok(comprehensive.some(error => error.includes('packaged-fedora')))
    assert.ok(comprehensive.every(error => !error.includes('macos-signing-notarization')), 'Apple notarization is optional even for comprehensive functional validation')
    writeFileSync(join(directory, names[4]), 'different installer')
    assert.ok(evaluate(report, '1.3.0', directory).some(error => error.includes('install-app.sh: artifact differs')))
    writeFileSync(join(directory, names[4]), 'tested package')
    writeFileSync(join(directory, names[0]), 'different package')
    assert.ok(evaluate(report, '1.3.0', directory).some(error => error.includes('differs from the tested candidate')))
    assert.ok(evaluate(report, '1.3.1', directory).some(error => error.includes('does not match')))
  } finally { rmSync(directory, { recursive: true, force: true }) }
})

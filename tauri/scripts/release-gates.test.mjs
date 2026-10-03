import { test } from 'node:test'
import assert from 'node:assert/strict'
import { mkdtempSync, writeFileSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { createHash } from 'node:crypto'
import { evaluate, requiredChecks } from './release-gates.mjs'

test('blocks missing native Mac validation even when other checks pass', () => {
  const checks = Object.fromEntries(requiredChecks.map(id => [id, { status: 'passed', evidence: 'record', osVersion: 'exact version', testedBy: 'tester', testedAt: '2026-10-03T00:00:00Z' }]))
  checks['native-spaces-macos-27'] = { status: 'blocked', reason: 'No verified native placement' }
  assert.ok(evaluate({ version: '2.4.0', schemaVersion: 1, checks, artifacts: {} }, '2.4.0').some(error => error.includes('No verified native placement')))
})

test('requires the exact packages that were functionally tested', () => {
  const directory = mkdtempSync(join(tmpdir(), 'multi-codex-gates-'))
  try {
    const names = ['Multi.Codex_2.4.0_amd64.AppImage', 'Multi.Codex_2.4.0_amd64.deb', 'Multi.Codex-2.4.0-1.x86_64.rpm', 'Multi.Codex_2.4.0_aarch64.dmg']
    const checks = Object.fromEntries(requiredChecks.map(id => [id, { status: 'passed', evidence: 'record', osVersion: 'exact version', testedBy: 'tester', testedAt: '2026-10-03T00:00:00Z' }]))
    const artifacts = {}
    for (const name of names) {
      writeFileSync(join(directory, name), 'tested package')
      artifacts[name] = createHash('sha256').update('tested package').digest('hex')
    }
    const report = { version: '2.4.0', schemaVersion: 1, checks, artifacts }
    assert.deepEqual(evaluate(report, '2.4.0', directory), [])
    writeFileSync(join(directory, names[0]), 'different package')
    assert.ok(evaluate(report, '2.4.0', directory).some(error => error.includes('differs from the tested candidate')))
    assert.ok(evaluate(report, '2.4.1', directory).some(error => error.includes('does not match')))
  } finally { rmSync(directory, { recursive: true, force: true }) }
})

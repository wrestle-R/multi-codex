import { test, expect } from '@playwright/test'
import { createUpdateHandler } from '../lib/update-handler'
const release = (tag: string, prerelease = false) => ({ tag_name: tag, draft: false, prerelease, published_at: '2026-10-15T00:00:00Z', assets: [{ name: 'latest.json', browser_download_url: `https://github.com/wrestle-R/multi-codex/releases/download/${tag}/latest.json` }] })

test('channel endpoint serves the matching manifest, not the unstable latest release', async () => {
  for (const [channel, version] of [['stable', '1.6.1'], ['all', '1.8.0-preview.1']]) {
    const calls: string[] = []
    const handler = createUpdateHandler(async input => {
      const url = String(input); calls.push(url)
      return Response.json(url.includes('api.github.com') ? [release('v1.8.0-preview.1', true), release('v1.7.0'), release('v1.6.1')] : { version, platforms: { 'linux-x86_64': { signature: 'signed-package', url: 'https://example.invalid/package' } } })
    })
    const response = await handler(channel)
    expect(response.status).toBe(200); expect((await response.json()).version).toBe(version)
    expect(calls[1]).toContain(`/v${version}/latest.json`)
    expect(response.headers.get('X-Multi-Codex-Channel')).toBe(channel)
  }
})
test('missing, unavailable and mismatched release feeds fail without an unsigned fallback', async () => {
  const empty = await createUpdateHandler(async () => Response.json([]))('stable')
  expect(empty.status).toBe(204)
  expect((await createUpdateHandler(async () => { throw Error('private-upstream-canary') })('wrong')).status).toBe(404)
  for (const payload of [{ version: '9.9.9', platforms: {} }, { version: '1.6.1' }, { version: '1.6.1', platforms: {} }, { version: '1.6.1', platforms: [] }, ...[
    { url: 'https://example.invalid/package' },
    { signature: '', url: 'https://example.invalid/package' },
    { signature: 'signed-package', url: 'http://example.invalid/package' },
    { signature: 'signed-package', url: 'invalid-package-location' },
  ].map(platform => ({ version: '1.6.1', platforms: { 'linux-x86_64': platform } }))]) {
    const result = await createUpdateHandler(async input => Response.json(String(input).includes('api.github.com') ? [release('v1.6.1')] : payload))('stable')
    expect(result.status).toBe(503); expect(result.headers.get('Cache-Control')).toBe('no-store')
  }
  const failed = await createUpdateHandler(async () => { throw Error('private-upstream-canary') })('stable')
  expect(failed.status).toBe(503); expect(await failed.text()).not.toContain('private-upstream-canary')
})

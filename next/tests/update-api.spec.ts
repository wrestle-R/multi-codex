import { test, expect } from '@playwright/test'
import { createUpdateHandler } from '../lib/update-handler'
const release = (tag: string, prerelease = false) => ({ tag_name: tag, draft: false, prerelease, published_at: '2026-10-15T00:00:00Z', assets: [{ name: 'latest.json', browser_download_url: `https://github.com/wrestle-R/multi-codex/releases/download/${tag}/latest.json` }, ...['_amd64.AppImage', '_aarch64.app.tar.gz', '_x64-setup.exe'].filter(suffix => tag !== 'v1.8.0' || !suffix.includes('aarch64')).map(suffix => ({name: `Multi.Codex_${tag.slice(1)}${suffix}`,browser_download_url:'https://example.invalid/package'}))] })

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


test('legacy feeds hold at 1.7.5 while platform feeds select their newest package', async () => {
  const inventory = [release('v1.8.0'), release('v1.7.5')].map(item => ({ ...item, assets: [...item.assets, {name: `Multi.Codex_${item.tag_name.slice(1)}_amd64.AppImage`, browser_download_url: 'https://example.invalid/linux'}, ...(item.tag_name === 'v1.7.5' ? [{name:'Multi.Codex_1.7.5_aarch64.app.tar.gz',browser_download_url:'https://example.invalid/mac'}] : [])] }))
  const fetcher: typeof fetch = async input => {
    const url = String(input)
    return Response.json(url.includes('api.github.com') ? inventory : { version: url.includes('v1.8.0') ? '1.8.0' : '1.7.5', platforms: { 'linux-x86_64': { signature:'signed',url:'https://example.invalid/package' } } })
  }
  const handler = createUpdateHandler(fetcher)
  expect((await (await handler('stable')).json()).version).toBe('1.7.5')
  expect((await (await handler('stable', 'linux-x86_64')).json()).version).toBe('1.8.0')
  expect((await (await handler('stable', 'darwin-aarch64')).json()).version).toBe('1.7.5')
})


test('a complete 1.8.1 release restores legacy updates and updates Mac without enabling T3', async () => {
  const inventory = [release('v1.8.1'), release('v1.8.0'), release('v1.7.5')]
  const handler = createUpdateHandler(async input => {
    const url = String(input)
    const version = /download\/v([^/]+)\//.exec(url)?.[1]
    return Response.json(url.includes('api.github.com') ? inventory : { version, platforms: {
      'linux-x86_64': {signature:'signed',url:'https://example.invalid/linux'},
      'windows-x86_64': {signature:'signed',url:'https://example.invalid/windows'},
      'darwin-aarch64': {signature:'signed',url:'https://example.invalid/mac'},
    } })
  })
  for (const channel of ['stable','all']) {
    for (const platform of [undefined,'linux-x86_64','windows-x86_64','darwin-aarch64']) {
      expect((await (await handler(channel,platform)).json()).version).toBe('1.8.1')
    }
  }
})

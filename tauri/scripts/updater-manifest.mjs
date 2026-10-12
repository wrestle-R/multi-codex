import { existsSync, readFileSync, writeFileSync } from 'node:fs'
import { resolve, join } from 'node:path'
import { fileURLToPath } from 'node:url'

export function createUpdaterManifest(directory, version) {
  if (!/^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$/.test(version)) throw new Error('Expected a stable package version')
  const names = {
    'linux-x86_64': `Multi.Codex_${version}_amd64.AppImage`,
    'darwin-aarch64': `Multi.Codex_${version}_aarch64.app.tar.gz`,
    'windows-x86_64': `Multi.Codex_${version}_x64-setup.exe`,
  }
  const platforms = Object.fromEntries(Object.entries(names).map(([platform, name]) => {
    if (!existsSync(join(directory, name))) throw new Error(`Missing updater artifact: ${name}`)
    const signature = readFileSync(join(directory, `${name}.sig`), 'utf8').trim()
    if (!signature || !Buffer.from(signature, 'base64').toString('utf8').startsWith('untrusted comment:')) throw new Error(`Missing or invalid updater signature: ${name}`)
    return [platform, { signature, url: `https://github.com/wrestle-R/multi-codex/releases/download/v${version}/${name}` }]
  }))
  return { version, notes: `Full release details: https://multi-codex.vercel.app/releases/v${version}`, platforms }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const [directory, version, action] = process.argv.slice(2)
  const expected = createUpdaterManifest(directory, version)
  const destination = join(directory, 'latest.json')
  if (action === '--check') {
    if (JSON.stringify(JSON.parse(readFileSync(destination, 'utf8'))) !== JSON.stringify(expected)) throw new Error('Updater manifest does not match the exact signed release assets')
    console.log('Updater manifest and signed artifact references match')
  } else {
    writeFileSync(destination, JSON.stringify(expected, null, 2) + '\n')
    console.log('Prepared latest.json for the signed desktop update')
  }
}

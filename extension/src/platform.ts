import { access } from 'node:fs/promises';
import { constants } from 'node:fs';
import { join } from 'node:path';

/** Resolve only this official extension's native executable, never another installation. */
export async function bundledEnginePath(extensionPath: string, platform = process.platform, architecture = process.arch): Promise<string> {
  if (!['x64', 'arm64'].includes(architecture) || !['linux', 'darwin'].includes(platform)) {
    throw new Error(`Multi Codex does not support ${platform} ${architecture}.`);
  }
  const cpu = architecture === 'arm64' ? 'aarch64' : 'x86_64';
  const directories = platform === 'darwin'
    ? [`macos-${cpu}`, architecture === 'arm64' ? 'darwin-arm64' : 'darwin-x86_64', `darwin-${cpu}`]
    : [`linux-${cpu}`];
  for (const directory of new Set(directories)) {
    const path = join(extensionPath, 'bin', directory, 'codex');
    try { await access(path, constants.X_OK); return path; } catch { /* Try the older official layout. */ }
  }
  throw new Error(`The official Codex extension has no executable for ${platform} ${architecture}. Reinstall its matching platform build.`);
}

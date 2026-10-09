import { access, readFile, readdir } from 'node:fs/promises';
import { constants } from 'node:fs';
import { basename, dirname, join } from 'node:path';

/** Re-resolve inside the same VS Code installation after official extension updates. */
export async function startupEnginePath(savedEngine: string, officialExtensionPath?: string): Promise<string> {
  const savedExtension = officialExtensionPath || dirname(dirname(dirname(savedEngine)));
  if (!basename(savedExtension).startsWith('openai.chatgpt-')) {
    await access(savedEngine, constants.X_OK);
    return savedEngine;
  }
  const extensionsRoot = dirname(savedExtension);
  let installed: any[] | undefined;
  try { installed = JSON.parse(await readFile(join(extensionsRoot, 'extensions.json'), 'utf8')); }
  catch (error) { if ((error as NodeJS.ErrnoException).code !== 'ENOENT') throw new Error('Could not read the installed Codex extension registry.'); }
  const names: string[] = installed
    ? installed.filter(entry => entry.identifier?.id?.toLowerCase() === 'openai.chatgpt').map(entry => entry.relativeLocation || basename(entry.location?.path || '')).filter(name => basename(name) === name && name.startsWith('openai.chatgpt-'))
    : (await readdir(extensionsRoot)).filter(name => name.startsWith('openai.chatgpt-'));
  names.sort((a, b) => b.localeCompare(a, 'en', { numeric: true }));
  for (const name of names) {
    const directory = join(extensionsRoot, name);
    try {
      const manifest = JSON.parse(await readFile(join(directory, 'package.json'), 'utf8'));
      if (manifest.publisher !== 'openai' || manifest.name !== 'chatgpt') continue;
      return await bundledEnginePath(directory);
    } catch { /* An update can remove an older installation during discovery. */ }
  }
  throw new Error('The official Codex extension has no installed executable. Reinstall its matching platform build.');
}

/** Resolve only this official extension's native executable, never another installation. */
export async function bundledEnginePath(extensionPath: string, platform = process.platform, architecture = process.arch): Promise<string> {
  if (!['x64', 'arm64'].includes(architecture) || !['linux', 'darwin', 'win32'].includes(platform) || (platform === 'win32' && architecture !== 'x64')) {
    throw new Error(`Multi Codex does not support ${platform} ${architecture}.`);
  }
  const cpu = architecture === 'arm64' ? 'aarch64' : 'x86_64';
  const directories = platform === 'darwin'
    ? [`macos-${cpu}`, architecture === 'arm64' ? 'darwin-arm64' : 'darwin-x86_64', `darwin-${cpu}`]
    : platform === 'win32' ? [`windows-${cpu}`] : [`linux-${cpu}`];
  for (const directory of new Set(directories)) {
    const path = join(extensionPath, 'bin', directory, platform === 'win32' ? 'codex.exe' : 'codex');
    try { await access(path, constants.X_OK); return path; } catch { /* Try the older official layout. */ }
  }
  throw new Error(`The official Codex extension has no executable for ${platform} ${architecture}. Reinstall its matching platform build.`);
}

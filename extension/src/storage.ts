import { readFile, realpath } from 'node:fs/promises';
import { basename, dirname, isAbsolute, join, relative, resolve, sep } from 'node:path';
import { homedir } from 'node:os';

export function defaultDataDirectory(platform = process.platform, home = homedir(), env = process.env): string {
  if (platform === 'darwin') return join(home, 'Library', 'Application Support', 'multi-codex');
  if (platform === 'linux') return join(env.XDG_DATA_HOME && isAbsolute(env.XDG_DATA_HOME) ? env.XDG_DATA_HOME : join(home, '.local', 'share'), 'multi-codex');
  throw new Error('Multi Codex supports local Linux and macOS.');
}

async function canonicalPath(path: string): Promise<string> {
  try { return await realpath(path); }
  catch (error) {
    if ((error as NodeJS.ErrnoException).code !== 'ENOENT') throw error;
    const parent = dirname(path);
    return parent === path ? path : join(await canonicalPath(parent), basename(path));
  }
}
function contains(parent: string, child: string) {
  const part = relative(parent, child);
  return part === '' || (part !== '..' && !part.startsWith(`..${sep}`) && !isAbsolute(part));
}

/** An app-launched VS Code inherits a managed profile home, not the desktop's global home. */
export async function resolveAccountPaths(options: {
  dataDirectory?: string; globalCodexHome?: string;
  platform?: NodeJS.Platform; home?: string; env?: NodeJS.ProcessEnv;
} = {}) {
  const home = options.home ?? homedir(); const env = options.env ?? process.env;
  const dataRoot = options.dataDirectory || defaultDataDirectory(options.platform, home, env);
  if (!isAbsolute(dataRoot)) throw new Error('Multi Codex account data directory must be an absolute path.');
  let desktop: { globalCodexHome?: string } = {};
  try { desktop = JSON.parse(await readFile(join(dataRoot, 'executables.json'), 'utf8')); }
  catch (error) {
    if ((error as NodeJS.ErrnoException).code !== 'ENOENT') throw new Error('Could not read Multi Codex desktop settings. Check the account data directory.');
  }
  const canonicalRoot = await canonicalPath(resolve(dataRoot));
  async function usable(candidate: string) {
    if (!isAbsolute(candidate)) return false;
    const canonicalHome = await canonicalPath(resolve(candidate));
    return !contains(canonicalRoot, canonicalHome) && !contains(canonicalHome, canonicalRoot);
  }
  const explicit = options.globalCodexHome;
  if (explicit && !await usable(explicit)) throw new Error('The configured global Codex home must be an absolute path outside Multi Codex account storage. Clear Multi Codex: Global Codex Home to use the desktop settings.');
  const candidates = [explicit, env.CODEX_HOME, desktop?.globalCodexHome, join(home, '.codex')];
  for (const candidate of candidates) {
    if (typeof candidate === 'string' && candidate && await usable(candidate)) return { dataRoot, globalHome: candidate };
  }
  throw new Error('Multi Codex account storage must be separate from the global Codex home. Check Multi Codex storage settings.');
}

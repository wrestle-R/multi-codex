import { chmod, copyFile, mkdir, readFile, symlink, writeFile } from 'node:fs/promises';
import { isAbsolute, join } from 'node:path';

/** The launch home can be a desktop account; it is separate from shared account storage. */
export function startupHomes(base: { startupHome?: string; globalHome: string }, env = process.env) {
  const sourceHome = env.CODEX_HOME || base.startupHome || base.globalHome;
  const historyHome = env.CODEX_SQLITE_HOME || sourceHome;
  if (!isAbsolute(sourceHome) || !isAbsolute(historyHome)) throw new Error('Codex startup homes must be absolute paths.');
  return { sourceHome, historyHome };
}

/** Keep switches private, but start with the current home's login and resources. */
export async function prepareSessionHome(sourceHome: string, sessionHome: string) {
  await mkdir(sessionHome, { recursive: true, mode: 0o700 });
  await mkdir(sourceHome, { recursive: true, mode: 0o700 });
  for (const name of ['sessions', 'archived_sessions', 'skills', 'rules', 'plugins', 'memories', 'prompts']) {
    await mkdir(join(sourceHome, name), { recursive: true, mode: 0o700 });
    await symlink(join(sourceHome, name), join(sessionHome, name), process.platform === 'win32' ? 'junction' : 'dir');
  }
  try {
    await readFile(join(sourceHome, 'AGENTS.md'));
    if (process.platform === 'win32') await copyFile(join(sourceHome, 'AGENTS.md'), join(sessionHome, 'AGENTS.md'));
    else await symlink(join(sourceHome, 'AGENTS.md'), join(sessionHome, 'AGENTS.md'));
  } catch (error) { if ((error as NodeJS.ErrnoException).code !== 'ENOENT') throw error; }
  let originalConfig = '';
  try { originalConfig = await readFile(join(sourceHome, 'config.toml'), 'utf8'); }
  catch (error) { if ((error as NodeJS.ErrnoException).code !== 'ENOENT') throw error; }
  await writeFile(join(sessionHome, 'config.toml'), 'cli_auth_credentials_store = "file"\n' + originalConfig.replace(/^cli_auth_credentials_store\s*=.*$/gm, ''), { mode: 0o600 });
  try {
    await copyFile(join(sourceHome, 'auth.json'), join(sessionHome, 'auth.json'));
    await chmod(join(sessionHome, 'auth.json'), 0o600);
  } catch (error) { if ((error as NodeJS.ErrnoException).code !== 'ENOENT') throw error; }
}

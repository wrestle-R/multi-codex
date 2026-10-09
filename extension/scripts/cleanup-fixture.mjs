import { readdir, readFile, rm } from 'node:fs/promises';
import { basename, dirname, join } from 'node:path';
import { tmpdir } from 'node:os';
import { createRequire } from 'node:module';
const { AccountClient } = createRequire(import.meta.url)('../dist/core.cjs');

export async function cleanupFixture(root, engine, helper) {
  if (dirname(root) !== tmpdir() || !basename(root).startsWith('multi-codex-vscode-')) throw new Error('Refusing to clean an unrelated directory');
  const profiles = JSON.parse(await readFile(join(root, 'data', 'profiles.json'), 'utf8').catch(() => '[]'));
  for (const profile of profiles) {
    const auth = JSON.parse(await readFile(join(root, 'data', 'profiles', profile.id, 'codex-home', 'auth.json'), 'utf8'));
    if (!['synthetic-account-a', 'synthetic-account-b'].includes(auth.tokens?.account_id)) throw new Error('Refusing to clean a non-fixture account');
  }
  // A failed test may leave the wrapper alive after Electron exits. Verify its
  // private test home before terminating only the recorded fixture processes.
  if (process.platform === 'linux') for (const window of await readdir(join(root, 'runtime', 'windows')).catch(() => [])) {
    const info = JSON.parse(await readFile(join(root, 'runtime', 'windows', window, 'backend.json'), 'utf8').catch(() => '{}'));
    for (const pid of [info.bridgePid, info.pid]) {
      if (!Number.isInteger(pid)) continue;
      const environment = await readFile(`/proc/${pid}/environ`, 'utf8').catch(() => '');
      if (environment.split('\0').some(entry => entry.startsWith(`CODEX_HOME=${root}/`))) try { process.kill(pid, 'SIGTERM'); } catch {}
    }
  }
  // Direct attachment has no wrapper registration. Backend plugin workers can
  // also outlive Electron briefly; terminate only processes with this exact
  // disposable home, never a normal VS Code/profile process.
  if (process.platform === 'linux') for (const name of await readdir('/proc')) {
    if (!/^\d+$/.test(name) || Number(name) === process.pid) continue;
    const environment = await readFile(`/proc/${name}/environ`, 'utf8').catch(() => '');
    if (environment.split('\0').some(entry => entry.startsWith(`CODEX_HOME=${root}/`))) {
      try { process.kill(Number(name), 'SIGTERM'); } catch {}
    }
  }
  const client = new AccountClient(helper, engine, join(root, 'data'), join(root, 'global-home'));
  try {
    for (const profile of profiles) {
      let deleted = false;
      let lastError;
      for (let attempt = 0; attempt < 20; attempt++) {
        try { await client.request('accounts/delete', { id: profile.id }); deleted = true; break; }
        catch (error) { lastError = error; await new Promise(resolve => setTimeout(resolve, 50)); }
      }
      if (!deleted) throw lastError;
    }
  } finally { await client.dispose(); }
  await rm(root, { recursive: true, force: true, maxRetries: 30, retryDelay: 100 });
}

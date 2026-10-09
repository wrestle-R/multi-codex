import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, writeFile, symlink, rm } from 'node:fs/promises';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { createRequire } from 'node:module';
const { defaultDataDirectory, resolveAccountPaths } = createRequire(import.meta.url)('../dist/core.cjs');

test('storage discovers the desktop directory on Mac and honors only absolute Linux XDG paths', () => {
  assert.equal(defaultDataDirectory('linux', '/home/user', {}), join('/home/user', '.local/share/multi-codex'));
  assert.equal(defaultDataDirectory('linux', '/home/user', { XDG_DATA_HOME: '/custom/data' }), join('/custom/data', 'multi-codex'));
  assert.equal(defaultDataDirectory('linux', '/home/user', { XDG_DATA_HOME: 'relative' }), join('/home/user', '.local/share/multi-codex'));
  assert.equal(defaultDataDirectory('darwin', '/Users/user', {}), join('/Users/user', 'Library/Application Support/multi-codex'));
  assert.equal(defaultDataDirectory('win32', '/Users/user', { APPDATA: '/roaming' }), join('/roaming', 'multi-codex'));
});

test('app-launched account home cannot hide desktop accounts, including through symlinks', async () => {
  const home = await mkdtemp(join(tmpdir(), 'mc-storage-'));
  const root = defaultDataDirectory('linux', home, {});
  const profile = join(root, 'profiles', 'saved-account', 'codex-home');
  const global = join(home, 'configured-global');
  await mkdir(profile, { recursive: true });
  await writeFile(join(root, 'executables.json'), JSON.stringify({ globalCodexHome: global }));
  await symlink(profile, join(home, 'profile-alias'), process.platform === 'win32' ? 'junction' : 'dir');
  try {
    for (const inherited of [profile, join(home, 'profile-alias')]) {
      const paths = await resolveAccountPaths({ platform: 'linux', home, env: { CODEX_HOME: inherited } });
      assert.deepEqual(paths, { dataRoot: root, globalHome: global });
    }
    await rm(join(root, 'executables.json'));
    assert.equal((await resolveAccountPaths({ platform: 'linux', home, env: { CODEX_HOME: profile } })).globalHome, join(home, '.codex'));
  } finally { await rm(home, { recursive: true, force: true }); }
});

test('standalone storage preserves an external Codex home and rejects explicit overlapping configuration', async () => {
  const home = await mkdtemp(join(tmpdir(), 'mc-storage-'));
  const root = join(home, 'accounts'); const external = join(home, 'current-codex');
  await mkdir(root);
  try {
    const paths = await resolveAccountPaths({ dataDirectory: root, home, env: { CODEX_HOME: external } });
    assert.deepEqual(paths, { dataRoot: root, globalHome: external });
    const explicit = join(home, 'override');
    assert.equal((await resolveAccountPaths({ dataDirectory: root, globalCodexHome: explicit, home, env: { CODEX_HOME: external } })).globalHome, explicit);
    await assert.rejects(() => resolveAccountPaths({ dataDirectory: root, globalCodexHome: root, home }), /outside/);
    await assert.rejects(() => resolveAccountPaths({ dataDirectory: root, globalCodexHome: home, home }), /outside/);
    await assert.rejects(() => resolveAccountPaths({ dataDirectory: 'relative', home }), /absolute/);
    await writeFile(join(root, 'executables.json'), 'broken json');
    await assert.rejects(() => resolveAccountPaths({ dataDirectory: root, home }), /desktop settings/);
  } finally { await rm(home, { recursive: true, force: true }); }
});

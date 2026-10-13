import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, writeFile, readFile, rm } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { createRequire } from 'node:module';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { syntheticAuth } from './fixtures/local-service.mjs';
const { AccountClient, helperName } = createRequire(import.meta.url)('../dist/core.cjs');
const engine = process.env.MULTI_CODEX_TEST_ENGINE;
test('packaged helper protects Windows runtime and inherited child files', { skip: !engine || process.platform !== 'win32' }, async () => {
  const root = await mkdtemp(join(tmpdir(), 'mc-runtime-acl-'));
  const helper = resolve('bin', helperName);
  try {
    await promisify(execFile)(helper, ['--protect-directory', root], { windowsHide: true });
    const child = join(root, 'bridge-config.json');
    await writeFile(child, '{}');
    // Windows PowerShell can inherit PowerShell 7's module path on the runner.
    // Read NTFS security directly through .NET instead of autoloading Get-Acl.
    const script = `$ErrorActionPreference='Stop'; Set-StrictMode -Version Latest; $owner=[Security.Principal.WindowsIdentity]::GetCurrent().User.Value; $a=[IO.Directory]::GetAccessControl($env:MULTI_CODEX_TEST_DIRECTORY); $b=[IO.File]::GetAccessControl([IO.Path]::Combine($env:MULTI_CODEX_TEST_DIRECTORY,'bridge-config.json')); @{owner=$owner;protected=$a.AreAccessRulesProtected;directory=@($a.GetAccessRules($true,$true,[Security.Principal.SecurityIdentifier]) | ForEach-Object {$_.IdentityReference.Value});child=@($b.GetAccessRules($true,$true,[Security.Principal.SecurityIdentifier]) | ForEach-Object {$_.IdentityReference.Value})} | ConvertTo-Json -Compress`;
    const { stdout, stderr } = await promisify(execFile)('powershell.exe', ['-NoProfile', '-NonInteractive', '-EncodedCommand', Buffer.from(script, 'utf16le').toString('base64')], { windowsHide: true, env: { ...process.env, MULTI_CODEX_TEST_DIRECTORY: root } });
    assert.equal(stderr.trim(), '', 'Windows ACL inspection reported an error');
    const acl = JSON.parse(stdout);
    assert.equal(acl.protected, true);
    for (const identities of [acl.directory, acl.child]) assert.deepEqual([...new Set(identities)].sort(), [acl.owner, 'S-1-5-18'].sort());
    await assert.rejects(() => promisify(execFile)(helper, ['--protect-directory', '.'], { windowsHide: true }));
  } finally { await rm(root, { recursive: true, force: true }); }
});
test('native storage supports standalone add/import, shared discovery and cross-process leases', { skip: !engine }, async () => {
  const root = await mkdtemp(join(tmpdir(), 'mc-native-test-')); const home = join(root, 'default');
  await mkdir(home); const authJson = JSON.stringify(syntheticAuth('synthetic-account-a'));
  await writeFile(join(home, 'auth.json'), authJson);
  const helper = resolve('bin', helperName); const data = join(root, 'data');
  const first = new AccountClient(helper, engine, data, home); const second = new AccountClient(helper, engine, data, home);
  try {
    assert.deepEqual(await first.list(), []);
    await first.request('accounts/add', { name: 'Standalone account', authJson });
    await second.request('accounts/import', { name: 'Imported current account' });
    let profiles = await first.list(); assert.equal(profiles.length, 2);
    const active = profiles.find(profile => profile.name === 'Standalone account');
    assert.deepEqual(await second.credential(active.id), JSON.parse(authJson));
    assert.equal(await readFile(join(home, 'auth.json'), 'utf8'), authJson, 'Import changed the default login');
    await first.request('accounts/lease', { id: active.id }); await second.request('accounts/lease', { id: active.id });
    await assert.rejects(() => second.request('accounts/delete', { id: active.id }), /running|open|close/i);
    await first.request('accounts/release', { id: active.id });
    await assert.rejects(() => first.request('accounts/delete', { id: active.id }), /running|open|close/i);
    await second.request('accounts/release', { id: active.id });
    await first.request('accounts/lockCredential', { id: active.id });
    await assert.rejects(() => second.request('accounts/lockCredential', { id: active.id }), /refreshing/);
    await assert.rejects(() => second.request('accounts/delete', { id: active.id }), /close/i);
    await first.request('accounts/unlockCredential', { id: active.id });
    await first.request('accounts/rename', { id: active.id, name: 'Renamed account' });
    assert.equal((await second.list()).find(p => p.id === active.id).name, 'Renamed account');
    await Promise.all(Array.from({ length: 4 }, (_, i) => (i % 2 ? first : second).request('accounts/add', { name: `Concurrent ${i}`, authJson })));
    assert.equal((await first.list()).length, 6);
    const cold = new AccountClient(helper, engine, data, home);
    try { await cold.request('accounts/delete', { id: active.id }); }
    finally { await cold.dispose(); }
    assert.equal((await second.list()).length, 5, 'A fresh helper could not remove a persisted account');
  } finally {
    for (const profile of await first.list().catch(() => [])) { await first.request('accounts/release', { id: profile.id }).catch(() => {}); await second.request('accounts/release', { id: profile.id }).catch(() => {}); }
    for (const profile of await first.list().catch(() => [])) { await first.request('accounts/unlockCredential', { id: profile.id }).catch(() => {}); await second.request('accounts/unlockCredential', { id: profile.id }).catch(() => {}); }
    for (const profile of await first.list().catch(() => [])) await first.request('accounts/delete', { id: profile.id }).catch(() => {});
    await Promise.all([first.dispose(), second.dispose()]); await rm(root, { recursive: true, force: true, maxRetries: 20, retryDelay: 100 });
  }
});

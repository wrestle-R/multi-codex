// Verify native account storage before asking the user to authenticate. This
// disposable API-key fixture never makes an API or model request.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { mkdtemp, mkdir, realpath, rm } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { tmpdir } from 'node:os';
const { AccountClient, helperName } = createRequire(import.meta.url)('../dist/core.cjs');
const root = await realpath(await mkdtemp(join(tmpdir(), 'mc-account-preflight-')));
const globalHome = join(root, 'global');
await mkdir(globalHome, { mode: 0o700 });
const accounts = new AccountClient(resolve('bin', helperName), resolve(process.env.AUTH_ENGINE), join(root, 'data'), globalHome);
const authJson = JSON.stringify({ auth_mode: 'apikey', OPENAI_API_KEY: 'inert-local-storage-fixture' });
const report = { platform: process.platform, passed: false, source: process.env.GITHUB_SHA ?? 'local' };
let phase = 'add-account';
try {
  const added = await accounts.request('accounts/add', { name: 'Disposable preflight', authJson });
  phase = 'verify-account-response';
  assert.match(added.id, /^[0-9a-f]{8}-(?:[0-9a-f]{4}-){3}[0-9a-f]{12}$/);
  assert.equal(added.name, 'Disposable preflight');
  phase = 'read-account';
  assert.equal((await accounts.list())[0].id, added.id);
  assert.deepEqual(await accounts.credential(added.id), JSON.parse(authJson));
  phase = 'delete-account';
  await accounts.request('accounts/delete', { id: added.id });
  assert.deepEqual(await accounts.list(), []);
  report.passed = true;
} catch {
  report.failedStage = phase;
  process.exitCode = 1;
} finally {
  for (const account of await accounts.list().catch(() => { report.cleanupFailed = true; return []; })) {
    await accounts.request('accounts/delete', { id: account.id }).catch(() => { report.cleanupFailed = true; });
  }
  await accounts.dispose();
  await rm(root, { recursive: true, force: true, maxRetries: 10, retryDelay: 300 }).catch(() => { report.cleanupFailed = true; });
  if (report.cleanupFailed) { report.passed = false; process.exitCode = 1; }
  console.log(JSON.stringify(report));
}

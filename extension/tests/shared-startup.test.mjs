import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, writeFile, readFile, lstat, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createRequire } from 'node:module';
const { prepareSessionHome, privateBackendEnvironment } = createRequire(import.meta.url)('../dist/core.cjs');

test('shared private backend cannot inherit another account or provider environment', () => {
  const env = privateBackendEnvironment('/private/home', { CODEX_HOME: '/other', CODEX_SQLITE_HOME: '/other-db', OPENAI_API_KEY: 'inert-fixture', OPENAI_BASE_URL: 'https://example.invalid', CODEX_API_KEY: 'inert-fixture', NODE_OPTIONS: '--fixture', ELECTRON_RUN_AS_NODE: '1', PATH: '/usr/bin' });
  assert.deepEqual(env, { PATH: '/usr/bin', CODEX_HOME: '/private/home', CODEX_SQLITE_HOME: '/private/home' });
});

test('managed sharing uses private writable histories and leaves source chats unchanged', async () => {
  const root = await mkdtemp(join(tmpdir(), 'mc-shared-startup-'));
  const source = join(root, 'source'); const runtime = join(root, 'private');
  await mkdir(join(source, 'sessions'), { recursive: true });
  await writeFile(join(source, 'sessions', 'original.jsonl'), 'full original history');
  await writeFile(join(source, 'auth.json'), '{"auth_mode":"fixture"}');
  try {
    await prepareSessionHome(source, runtime, { sharedChats: true });
    for (const name of ['sessions', 'archived_sessions']) assert.equal((await lstat(join(runtime, name))).isSymbolicLink(), false);
    await writeFile(join(runtime, 'sessions', 'original.jsonl'), 'private next turn');
    assert.equal(await readFile(join(source, 'sessions', 'original.jsonl'), 'utf8'), 'full original history');
    assert.equal(await readFile(join(runtime, 'auth.json'), 'utf8'), await readFile(join(source, 'auth.json'), 'utf8'));
    assert.equal((await lstat(join(runtime, 'skills'))).isSymbolicLink(), true);
  } finally { await rm(root, { recursive: true, force: true }); }
});

test('legacy startup retains its existing history routing when sharing is not enabled', async () => {
  const root = await mkdtemp(join(tmpdir(), 'mc-legacy-startup-'));
  try {
    const source = join(root, 'source'); const runtime = join(root, 'private');
    await prepareSessionHome(source, runtime);
    assert.equal((await lstat(join(runtime, 'sessions'))).isSymbolicLink(), true);
    assert.equal((await lstat(join(runtime, 'archived_sessions'))).isSymbolicLink(), true);
  } finally { await rm(root, { recursive: true, force: true }); }
});

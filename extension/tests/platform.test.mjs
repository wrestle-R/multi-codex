import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, writeFile, rm } from 'node:fs/promises';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { createRequire } from 'node:module';
const { bundledEnginePath } = createRequire(import.meta.url)('../dist/core.cjs');

test('engine discovery resolves official Mac and Linux layouts and rejects unavailable platforms', async () => {
  const root = await mkdtemp(join(tmpdir(), 'mc-platform-'));
  async function executable(directory) {
    const path = join(root, 'bin', directory, 'codex');
    await mkdir(join(root, 'bin', directory), { recursive: true });
    await writeFile(path, '#!/bin/sh\nexit 0\n', { mode: 0o700 });
    return path;
  }
  try {
    const arm = await executable('macos-aarch64');
    const intel = await executable('macos-x86_64');
    const linux = await executable('linux-x86_64');
    assert.equal(await bundledEnginePath(root, 'darwin', 'arm64'), arm);
    assert.equal(await bundledEnginePath(root, 'darwin', 'x64'), intel);
    assert.equal(await bundledEnginePath(root, 'linux', 'x64'), linux);
    await assert.rejects(() => bundledEnginePath(root, 'win32', 'x64'), /does not support/);
    await assert.rejects(() => bundledEnginePath(root, 'linux', 'arm64'), /no executable/);
    await rm(arm);
    const legacy = await executable('darwin-arm64');
    assert.equal(await bundledEnginePath(root, 'darwin', 'arm64'), legacy);
  } finally { await rm(root, { recursive: true, force: true }); }
});

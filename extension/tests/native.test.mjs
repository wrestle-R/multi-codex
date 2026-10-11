import test from 'node:test';
import assert from 'node:assert/strict';
import { once } from 'node:events';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { randomUUID } from 'node:crypto';
import { createRequire } from 'node:module';
const { AccountClient } = createRequire(import.meta.url)('../dist/core.cjs');

test('a missing native helper can be disposed so Refresh can reconnect', { timeout: 2000 }, async () => {
  const missing = join(tmpdir(), `missing-multi-codex-${randomUUID()}`);
  const client = new AccountClient(missing, missing, tmpdir(), tmpdir());
  await once(client.child, 'error');
  assert.equal(client.isRunning, false);
  await assert.rejects(() => client.list(), /unavailable/);
  await client.dispose();
  assert.equal(client.child.stdin.destroyed, true);
  assert.equal(client.child.stdout.destroyed, true);
});

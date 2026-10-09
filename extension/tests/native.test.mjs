import test from 'node:test';
import assert from 'node:assert/strict';
import { once } from 'node:events';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { randomUUID } from 'node:crypto';
import { createRequire } from 'node:module';
import { createServer, createConnection } from 'node:net';
const { AccountClient, controlEndpoint } = createRequire(import.meta.url)('../dist/core.cjs');

test('independent control servers with a shared long ID prefix remain separate', async () => {
  const prefix = `shared-long-directory-prefix-${randomUUID()}`;
  const paths = [controlEndpoint(`${prefix}-account-a`), controlEndpoint(`${prefix}-account-b`)];
  const servers = paths.map((_, index) => createServer(socket => socket.end(String(index))));
  try {
    await Promise.all(servers.map((server, index) => new Promise((resolve, reject) => { server.once('error', reject); server.listen(paths[index], resolve); })));
    const answers = await Promise.all(paths.map(path => new Promise((resolve, reject) => {
      let received = ''; const socket = createConnection(path);
      socket.on('data', chunk => { received += chunk; }); socket.once('error', reject); socket.once('end', () => resolve(received));
    })));
    assert.deepEqual(answers, ['0', '1']);
  } finally { await Promise.all(servers.map(server => new Promise(resolve => server.close(resolve)))); }
});

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

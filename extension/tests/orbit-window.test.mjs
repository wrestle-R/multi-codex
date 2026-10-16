import test from 'node:test';
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { randomUUID } from 'node:crypto';
import { createConnection } from 'node:net';
import { stat } from 'node:fs/promises';
const { OrbitWindow } = createRequire(import.meta.url)('../dist/core.cjs');
const request = (id, message = id) => new Promise((resolve, reject) => {
  const socket = createConnection(`/tmp/mcx-orbit-${id}.sock`);
  let reply = '';
  socket.on('error', reject);
  socket.on('connect', () => socket.write(`${message}\n`));
  socket.on('data', data => reply += data);
  socket.on('end', () => resolve(reply));
});
test('Orbit focus selects only the owning window and rejects stale or mismatched sessions', { skip: process.platform === 'win32' }, async () => {
  const a = new OrbitWindow(), b = new OrbitWindow();
  const aid = randomUUID(), bid = randomUUID();
  let af = 0, bf = 0;
  try {
    await a.listen(aid, async () => { af++; });
    await b.listen(bid, async () => { bf++; });
    assert.equal((await stat(`/tmp/mcx-orbit-${aid}.sock`)).mode & 0o777, 0o600);
    assert.equal(await request(aid, bid), 'closed\n');
    assert.deepEqual([af, bf], [0, 0]);
    assert.equal(await request(bid), 'focused\n');
    assert.deepEqual([af, bf], [0, 1]);
    assert.equal(await request(aid), 'focused\n');
    assert.deepEqual([af, bf], [1, 1]);
    await a.dispose();
    await assert.rejects(() => request(aid), /ENOENT|ECONNREFUSED/);
    assert.equal(await request(bid), 'focused\n');
    assert.deepEqual([af, bf], [1, 2]);
  } finally { await a.dispose(); await b.dispose(); }
});

import test from 'node:test';
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
const { ThreadOperations } = createRequire(import.meta.url)('../dist/core.cjs');
test('one chat preserves mutation order while other chats progress, including after a failure', async () => {
  const operations = new ThreadOperations(); const events = []; let release;
  const hold = new Promise(resolve => { release = resolve; });
  const first = operations.run('a', async () => { events.push('a-first'); await hold; throw new Error('Injected operation failure'); });
  const rejected = assert.rejects(first, /Injected operation failure/);
  const second = operations.run('a', async () => { events.push('a-second'); });
  await operations.run('b', async () => { events.push('b'); });
  assert.deepEqual(events, ['a-first', 'b']);
  let drained = false; const drain = operations.drain().then(() => { drained = true; });
  await Promise.resolve(); assert.equal(drained, false);
  release(); await Promise.all([rejected, second, drain]); assert.deepEqual(events, ['a-first', 'b', 'a-second']);
});

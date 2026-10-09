import test from 'node:test';
import assert from 'node:assert/strict';
import { PassThrough } from 'node:stream';
import { createRequire } from 'node:module';
const { JsonLines } = createRequire(import.meta.url)('../dist/core.cjs');

test('a long chat response exceeding the former 8 MiB limit arrives once across fragmented chunks', () => {
  const input = new PassThrough(); const output = new PassThrough();
  const lines = new JsonLines(input, output);
  const messages = []; const faults = [];
  lines.on('message', message => messages.push(message)); lines.on('fault', error => faults.push(error));
  const history = 'x'.repeat(9 * 1024 * 1024) + '🙂';
  const bytes = Buffer.from(JSON.stringify({ id: 'history', result: { history } }) + '\n');
  for (let offset = 0; offset < bytes.length; offset += 32767) input.write(bytes.subarray(offset, offset + 32767));
  assert.deepEqual(faults, []); assert.equal(messages.length, 1); assert.equal(messages[0].result.history, history);
});
test('oversized malformed input faults once and discards through its newline before reading another frame', () => {
  const input = new PassThrough(); const lines = new JsonLines(input, new PassThrough(), 32);
  const messages = []; const faults = [];
  lines.on('message', message => messages.push(message)); lines.on('fault', error => faults.push(error));
  input.write('x'.repeat(33)); input.write('still the oversized frame\n{"id":"next"}\n');
  assert.equal(faults.length, 1); assert.match(faults[0].message, /too large/);
  assert.deepEqual(messages, [{ id: 'next' }]);
});

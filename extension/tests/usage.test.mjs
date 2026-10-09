import test from 'node:test';
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
const { usageSummary, refreshAllAccounts } = createRequire(import.meta.url)('../dist/core.cjs');
test('all-account refresh bounds backends and continues after an account fails', async () => {
  let running = 0; let peak = 0; const results = [];
  await refreshAllAccounts(['a', 'b', 'c', 'd'], async id => {
    running++; peak = Math.max(peak, running);
    await new Promise(resolve => setTimeout(resolve, 5)); running--;
    if (id === 'b') throw new Error('Expired account');
  }, (id, error) => results.push([id, Boolean(error)]));
  assert.equal(peak, 2); assert.equal(results.length, 4);
  assert.deepEqual(results.find(([id]) => id === 'b'), ['b', true]);
});
test('usage labels preserve windows, reset times and unknown limits', () => {
  const now = 1800000000000;
  assert.match(usageSummary({ rateLimits: { primary: { windowDurationMins: 300, usedPercent: 29, resetsAt: now / 1000 + 7200 }, secondary: { windowDurationMins: 10080, usedPercent: 68 } } }, now), /5h: 71% left · resets in 2h · Weekly: 32% left/);
  assert.equal(usageSummary({}), 'Usage unavailable');
  assert.match(usageSummary({ rateLimits: { primary: { usedPercent: undefined } } }), /unavailable/);
});

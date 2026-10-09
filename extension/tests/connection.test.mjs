import test from 'node:test';
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
const { waitForReady } = createRequire(import.meta.url)('../dist/core.cjs');

test('early account selection waits for initialization without clearing active-work guards', async () => {
  let initialized = false;
  const pending = waitForReady(async () => ({ ready: initialized, canSwitch: false, reason: 'A Codex chat is still running.' }), 1000);
  setTimeout(() => { initialized = true; }, 20);
  const state = await pending;
  assert.equal(state.ready, true);
  assert.equal(state.canSwitch, false);
  assert.match(state.reason, /still running/);
});
test('a dead backend reports disconnection immediately rather than waiting for initialization', async () => {
  await assert.rejects(waitForReady(async () => ({ ready: false, connectionFailed: true })), /backend disconnected/);
});
test('a backend that never initializes stops waiting and gives a retry action', async () => {
  await assert.rejects(waitForReady(async () => ({ ready: false }), 30), /Open its panel.*retry/);
});

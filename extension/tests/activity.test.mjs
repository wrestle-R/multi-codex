import { test } from 'node:test';
import assert from 'node:assert/strict';
import { ActivityGuard, loginParams, matchesIdentity } from '../dist/core.cjs';
import { syntheticAuth } from './fixtures/local-service.mjs';

test('switching requires a connected backend, then acquires one exclusive lock', () => {
  const guard = new ActivityGuard(); assert.throws(() => guard.acquire(), /connecting/);
  guard.ready = true; guard.acquire(); assert.throws(() => guard.acquire(), /already/);
});
test('a submitted turn blocks switching before the started event arrives', () => {
  const guard = new ActivityGuard(); guard.ready = true;
  guard.outgoing({ id: 1, method: 'turn/start' }); assert.throws(() => guard.acquire(), /running/);
  guard.incoming({ id: 1, result: { turn: { id: 'turn-a', status: 'inProgress' } } }); assert.throws(() => guard.acquire(), /running/);
  guard.incoming({ method: 'turn/completed', params: { turn: { id: 'turn-a', status: 'completed' } } }); guard.acquire();
});
test('a hidden chat or subagent and a waiting approval are busy', () => {
  const guard = new ActivityGuard(); guard.ready = true;
  guard.incoming({ method: 'thread/status/changed', params: { threadId: 'hidden', status: { type: 'active', activeFlags: ['waitingOnApproval'] } } });
  assert.throws(() => guard.acquire(), /running/);
  guard.incoming({ method: 'thread/status/changed', params: { threadId: 'hidden', status: { type: 'idle' } } });
  guard.incoming({ id: 'approval', method: 'item/commandExecution/requestApproval' }); assert.throws(() => guard.acquire(), /approval/);
  guard.outgoing({ id: 'approval', result: { decision: 'accept' } }); guard.acquire();
});
test('queued messages and unknown queue status both block switching', () => {
  const guard = new ActivityGuard(); guard.ready = true;
  guard.incoming({ method: 'thread/queue/changed', params: { threadId: 'a', queue: [{}] } }); assert.throws(() => guard.acquire(), /queued/);
  guard.incoming({ method: 'thread/queue/changed', params: { threadId: 'a', queue: [] } }); assert.equal(guard.snapshot().canSwitch, true);
  guard.incoming({ method: 'thread/queue/changed', params: { threadId: 'a' } }); assert.throws(() => guard.acquire(), /verified/);
});
test('failed request does not leave a phantom running turn', () => {
  const guard = new ActivityGuard(); guard.ready = true;
  guard.outgoing({ id: 'bad', method: 'turn/start' }); guard.incoming({ id: 'bad', error: { code: -1, message: 'failed' } }); guard.acquire();
});
test('background commands and streaming processes remain busy', () => {
  const guard = new ActivityGuard(); guard.ready = true;
  guard.incoming({ method: 'process/started', params: { processId: 'command' } }); assert.throws(() => guard.acquire(), /running/);
  guard.incoming({ method: 'process/exited', params: { processId: 'command' } }); guard.acquire();
});
test('selected identity must match the adopted token, not only the account label', () => {
  const a = syntheticAuth('synthetic-account-a'); const b = syntheticAuth('synthetic-account-b');
  assert.equal(loginParams(a).chatgptAccountId, 'synthetic-account-a');
  assert.equal(matchesIdentity({ authMethod: 'chatgptAuthTokens', authToken: a.tokens.access_token }, a), true);
  assert.equal(matchesIdentity({ authMethod: 'chatgptAuthTokens', authToken: b.tokens.access_token }, a), false);
  assert.throws(() => loginParams({ auth_mode: 'chatgpt', tokens: {} }), /access token/);
});

import * as vscode from 'vscode';
import assert from 'node:assert/strict';
import { writeFile, readFile } from 'node:fs/promises';
import { join } from 'node:path';

async function waitFor<T>(read: () => Promise<T>, check: (value: T) => boolean, timeout = 20000): Promise<T> {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) { const value = await read(); if (check(value)) return value; await new Promise(resolve => setTimeout(resolve, 100)); }
  throw new Error('VS Code integration condition timed out');
}

export async function run() {
  const root = process.env.MULTI_CODEX_VSCODE_TEST_ROOT!;
  const checks: string[] = [];
  const extensionId = process.env.MULTI_CODEX_TEST_EXTENSION_ID;
  assert.ok(extensionId, 'The test harness must provide the packaged extension ID');
  const extension = vscode.extensions.getExtension(extensionId)!;
  const api = await extension.activate();
  await vscode.commands.executeCommand('chatgpt.openSidebar');
  if (process.env.MULTI_CODEX_TEST_LIVE_ATTACH === '1') {
    const before = api.testBackendInfo();
    assert.ok(before.pid, 'The original Codex backend must already be running');
    await Promise.all([api.enableSwitching(), api.enableSwitching()]);
    const after = await api.getState();
    assert.equal(after.bridge.backendPid, before.pid, 'Initial attachment restarted Codex');
    assert.equal(after.activationId, before.activationId, 'Initial attachment reactivated Multi Codex');
    assert.equal(after.extensionHostPid, before.hostPid, 'Initial attachment restarted the extension host');
    checks.push('Attached to an already-running official Codex backend without restarting either extension or VS Code');
  }
  const initial = await waitFor<any>(() => api.getState(), value => !!value.bridge?.ready && value.accounts.length === 2);
  const activationId = initial.activationId; const hostPid = initial.extensionHostPid;
  const backendPid = initial.bridge.backendPid; const bridgeInstanceId = initial.bridge.instanceId;
  const a = initial.accounts.find((account: any) => account.name === 'Account A');
  const b = initial.accounts.find((account: any) => account.name === 'Account B');
  async function inspectPanel(label: string) {
    if (process.env.MULTI_CODEX_UI_TEST !== '1') return;
    await vscode.commands.executeCommand('chatgpt.openSidebar');
    await writeFile(join(root, 'ui-phase.json'), JSON.stringify({ label }));
    await waitFor(() => readFile(join(root, 'ui-ack.json'), 'utf8').catch(() => '{}'), value => JSON.parse(value).label === label);
  }
  async function verifyStable() {
    const value = await api.getState();
    assert.equal(value.activationId, activationId, 'Multi Codex reactivated'); assert.equal(value.extensionHostPid, hostPid, 'Extension host restarted');
    assert.equal(value.bridge.backendPid, backendPid, 'Codex backend unexpectedly restarted'); assert.equal(value.bridge.instanceId, bridgeInstanceId);
    return value;
  }
  for (const account of [a, b, a]) {
    await api.switchAccount(account.id);
    const value = await verifyStable(); assert.equal(value.bridge.selectedId, account.id);
    const auth = await api.testRequest('getAuthStatus', { includeToken: true, refreshToken: false });
    const jwt = JSON.parse(Buffer.from(auth.authToken.split('.')[1], 'base64url').toString());
    assert.equal(jwt['https://api.openai.com/auth'].chatgpt_account_id, account.name === 'Account A' ? 'synthetic-account-a' : 'synthetic-account-b');
    await inspectPanel(`${checks.length}-${account.name}`);
    checks.push(`Selected ${account.name}`);
  }
  checks.push('A → B → A verified in official Codex backend; VS Code, Multi Codex and backend stayed running');
  const usage = await api.testRequest('account/rateLimits/read'); assert.equal(usage.rateLimits.primary.usedPercent, 10);
  checks.push('Usage read through the official bundled backend');
  const thread = await api.testRequest('thread/start', { cwd: join(root, 'workspace'), model: 'gpt-5.4', approvalPolicy: 'never', sandbox: 'read-only' });
  await writeFile(join(root, 'hold-response'), 'hold');
  await waitFor(() => readFile(join(root, 'hold-ready'), 'utf8').catch(() => ''), value => value === 'ready');
  await api.testRequest('turn/start', { threadId: thread.thread.id, input: [{ type: 'text', text: 'Reply with local-fixture-ok.' }] });
  await waitFor<any>(() => api.getState(), value => value.bridge.activeTurns > 0);
  await assert.rejects(() => api.switchAccount(b.id), /running/);
  assert.equal((await api.getState()).bridge.selectedId, a.id);
  checks.push('Switch rejected during an actual in-progress Codex turn');
  await writeFile(join(root, 'release-response'), 'release');
  await waitFor<any>(() => api.getState(), value => value.bridge.canSwitch, 25000);
  await api.switchAccount(b.id); await verifyStable();
  checks.push('Switch allowed after the actual turn completed');
  await api.testRequest('turn/start', { threadId: thread.thread.id, input: [{ type: 'text', text: 'Continue the same chat with local-fixture-ok.' }] });
  await waitFor<any>(() => api.getState(), value => value.bridge.canSwitch, 25000);
  const continued = await api.testRequest('thread/read', { threadId: thread.thread.id, includeTurns: true });
  assert.equal(continued.thread.id, thread.thread.id); assert.equal(continued.thread.turns.length, 2);
  checks.push('Existing chat continued after switching accounts without creating a new thread');
  await inspectPanel('continued-Account B');
  await assert.rejects(() => api.switchAccount('00000000-0000-4000-8000-000000000000'), /not found/);
  assert.equal((await api.getState()).bridge.selectedId, b.id);
  checks.push('Missing account leaves the previous verified account active');
  await vscode.commands.executeCommand('workbench.view.extension.multiCodex');
  await writeFile(join(root, 'result.json'), JSON.stringify({ passed: true, checks, hostPid, backendPid, activationId, bridgeInstanceId }, null, 2));
}

import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process';
import { spawnExecutable, terminateExecutable } from './native';
import { JsonLines, RpcPeer } from './protocol';
import type { StoredAuth } from './auth';

export type Account = { id: string; name: string; authMode: string; notes?: string; accountTier?: string; status: string; error?: string };
export class AccountClient {
  readonly child: ChildProcessWithoutNullStreams;
  private peer: RpcPeer;
  private closed = false;
  get isRunning() { return !this.closed; }
  constructor(helper: string, engine: string, dataRoot: string, globalHome: string) {
    this.child = spawnExecutable(helper, [engine, dataRoot, globalHome]);
    this.child.stderr.resume();
    this.peer = new RpcPeer(new JsonLines(this.child.stdout, this.child.stdin), 'account-helper:');
    const close = () => { this.closed = true; this.peer.close(); };
    this.child.on('error', close); this.child.on('exit', close);
  }
  request(method: string, params: any = {}) {
    if (this.closed) return Promise.reject(new Error('Account storage helper is unavailable. Use Refresh to reconnect.'));
    return this.peer.request(method, params);
  }
  async list(): Promise<Account[]> { return this.request('accounts/list'); }
  async credential(id: string): Promise<StoredAuth> { const result = await this.request('accounts/credential', { id }); return JSON.parse(result.auth); }
  async withCredentialLock<T>(id: string, action: () => Promise<T>): Promise<T> {
    await this.request('accounts/lockCredential', { id });
    try { return await action(); } finally { await this.request('accounts/unlockCredential', { id }).catch(() => {}); }
  }
  private disposal?: Promise<void>;
  dispose() { return this.disposal ??= (async () => { this.closed = true; this.peer.close(); this.child.stdin.end(); await terminateExecutable(this.child); })(); }
}

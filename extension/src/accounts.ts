import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process';
import { JsonLines, RpcPeer } from './protocol';
import type { StoredAuth } from './auth';

export type Account = { id: string; name: string; authMode: string; notes?: string; accountTier?: string; status: string; error?: string };
export class AccountClient {
  readonly child: ChildProcessWithoutNullStreams;
  private peer: RpcPeer;
  constructor(helper: string, engine: string, dataRoot: string, globalHome: string) {
    this.child = spawn(helper, [engine, dataRoot, globalHome], { stdio: ['pipe', 'pipe', 'pipe'] });
    this.child.stderr.resume();
    this.peer = new RpcPeer(new JsonLines(this.child.stdout, this.child.stdin), 'account-helper:');
    this.child.on('error', () => this.peer.close()); this.child.on('exit', () => this.peer.close());
  }
  request(method: string, params: any = {}) { return this.peer.request(method, params); }
  async list(): Promise<Account[]> { return this.request('accounts/list'); }
  async credential(id: string): Promise<StoredAuth> { const result = await this.request('accounts/credential', { id }); return JSON.parse(result.auth); }
  async withCredentialLock<T>(id: string, action: () => Promise<T>): Promise<T> {
    await this.request('accounts/lockCredential', { id });
    try { return await action(); } finally { await this.request('accounts/unlockCredential', { id }).catch(() => {}); }
  }
  dispose() { this.peer.close(); this.child.stdin.end(); this.child.kill(); }
}

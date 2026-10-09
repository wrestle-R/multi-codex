import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process';
import { createServer, type Server, type Socket } from 'node:net';
import { spawnExecutable, terminateExecutable } from './native';
import { chmod, mkdir, readFile, rm } from 'node:fs/promises';
import { join } from 'node:path';
import { randomUUID, timingSafeEqual } from 'node:crypto';
import { ActivityGuard } from './activity';
import { AccountClient } from './accounts';
import { claims, loginParams, matchesIdentity, type StoredAuth } from './auth';
import { JsonLines, RpcPeer, type Message } from './protocol';
import { withAccountBackend } from './worker';

export type BridgeConfig = { engine: string; helper: string; dataRoot: string; globalHome: string; sessionHome: string; sourceHome?: string; historyHome?: string; socket: string; token: string; workspace: string; attached?: boolean };
export type ExistingBackend = { child: ChildProcessWithoutNullStreams; lines: JsonLines; restore: () => void };
export class CodexBridge {
  readonly activity = new ActivityGuard();
  readonly instanceId = randomUUID();
  readonly backend: ChildProcessWithoutNullStreams;
  readonly backendLines: JsonLines;
  readonly backendRpc: RpcPeer;
  private selectedId: string | null = null;
  private selectedAuth: StoredAuth | null = null;
  private clients = new Map<JsonLines, Socket>();
  private server?: Server;
  private accounts: AccountClient;
  private initializeId: string | number | undefined;
  private connectionFailed = false;
  private connectionFailure: string | null = null;
  private authenticationUnverified = false;
  private disposed = false;
  private blockedWorkDuringSwitch = false;
  private credentialRequests = new Set<string | number>();
  constructor(readonly config: BridgeConfig, readonly frontend: JsonLines, args: string[], private existing?: ExistingBackend) {
    const env: NodeJS.ProcessEnv = { ...process.env, CODEX_HOME: config.sessionHome, CODEX_SQLITE_HOME: config.historyHome ?? config.sessionHome };
    delete env.OPENAI_API_KEY; delete env.CODEX_API_KEY; delete env.ELECTRON_RUN_AS_NODE;
    this.backend = existing?.child ?? spawnExecutable(config.engine, args, { env, cwd: config.workspace, stdio: ['pipe', 'pipe', 'pipe'] });
    // Codex diagnostics can contain workspace and account data. Never mirror them into VS Code logs.
    if (!existing) this.backend.stderr.resume();
    this.backendLines = existing?.lines ?? new JsonLines(this.backend.stdout, this.backend.stdin);
    this.backendRpc = new RpcPeer(this.backendLines);
    this.accounts = new AccountClient(config.helper, config.engine, config.dataRoot, config.globalHome);
    frontend.on('message', (message: Message) => this.fromFrontend(message));
    frontend.on('end', () => void this.dispose());
    frontend.on('fault', () => this.fail('frontend-protocol'));
    this.backendLines.on('message', (message: Message) => this.fromBackend(message));
    this.backendLines.on('fault', () => this.fail('backend-protocol'));
    this.backendLines.on('end', () => this.fail('backend-output-closed'));
    this.backend.on('error', () => this.fail('backend-start-failed')); this.backend.on('exit', () => this.fail('backend-exited'));
  }
  async attachReady() {
    if (!this.existing) throw new Error('No existing backend');
    // Read the current identity for rollback before allowing any mutation.
    const deadline = Date.now() + 10000;
    for (;;) {
      try { this.selectedAuth = await this.currentAuth(); break; }
      catch (error) {
        if (Date.now() >= deadline || this.connectionFailed || !(error instanceof Error) || !/not initialized/i.test(error.message)) throw error;
        await new Promise(resolve => setTimeout(resolve, 100));
      }
    }
    if (this.selectedAuth?.OPENAI_API_KEY) {
      throw new Error('Live attachment requires a ChatGPT login. API-key switching uses the isolated bridge on the next Codex startup.');
    }
    this.activity.ready = true;
    this.broadcast();
  }
  private async currentAuth(): Promise<StoredAuth | null> {
    const status = await this.backendRpc.request('getAuthStatus', { includeToken: true, refreshToken: false });
    if (status.authToken) {
      if (status.authMethod === 'apikey') return { auth_mode: 'apikey', OPENAI_API_KEY: status.authToken };
      const accountId = claims(status.authToken)['https://api.openai.com/auth']?.chatgpt_account_id;
      if (!accountId) throw new Error('The current Codex identity could not be verified');
      return { auth_mode: 'ChatGPT', tokens: { access_token: status.authToken, account_id: accountId } };
    }
    // Recent backends can withhold tokens for a file-based ChatGPT login.
    // Compare that private file with the backend's authoritative routing identity.
    const current = await this.backendRpc.request('account/read', { refreshToken: false });
    if (!current.account) return null;
    const home = this.config.attached ? this.config.sourceHome ?? this.config.globalHome : this.config.sessionHome;
    const auth: StoredAuth = JSON.parse(await readFile(join(home, 'auth.json'), 'utf8'));
    const login = loginParams(auth);
    if (current.account.type !== 'chatgpt' || login.type !== 'chatgptAuthTokens' || login.chatgptAccountId !== current.workspaceRouting?.chatgptAccountId) {
      throw new Error('The current Codex identity could not be verified. Account switching was not started.');
    }
    return auth;
  }
  async listen() {
    await mkdir(this.config.sessionHome, { recursive: true, mode: 0o700 });
    this.server = createServer(socket => this.accept(socket));
    await new Promise<void>((resolve, reject) => { this.server!.once('error', reject); this.server!.listen(this.config.socket, resolve); });
    if (process.platform !== 'win32') await chmod(this.config.socket, 0o600);
  }
  private accept(socket: Socket) {
    const lines = new JsonLines(socket, socket, 2 * 1024 * 1024); let authorized = false;
    const deadline = setTimeout(() => socket.destroy(), 3000);
    lines.on('fault', () => socket.destroy());
    lines.on('message', async (message: Message) => {
      if (!authorized) {
        const token = message.params?.token;
        if (message.method !== 'connect' || typeof token !== 'string' || Buffer.byteLength(token) !== Buffer.byteLength(this.config.token) || !timingSafeEqual(Buffer.from(token), Buffer.from(this.config.token))) { socket.destroy(); return; }
        authorized = true; clearTimeout(deadline); this.clients.set(lines, socket);
        lines.send({ id: message.id, result: { protocolVersion: 1, ...this.state() } }); return;
      }
      if (message.id === undefined) return;
      try {
        const result = await this.control(message.method!, message.params ?? {});
        lines.send({ id: message.id, result });
      } catch (error) { lines.send({ id: message.id, error: { code: -32000, message: error instanceof Error ? error.message : 'Codex request failed' } }); }
    });
    socket.on('close', () => { clearTimeout(deadline); this.clients.delete(lines); });
  }
  private fromFrontend(message: Message) {
    if (message.method === 'initialize') {
      this.initializeId = message.id;
      message = { ...message, params: { ...message.params, capabilities: { ...message.params?.capabilities, experimentalApi: true } } };
    }
    const authMutation = ['account/login/start', 'account/logout', 'login', 'logout'].includes(message.method ?? '');
    if (this.connectionFailed || ((this.activity.switching || this.authenticationUnverified) && (this.activity.isWork(message.method) || authMutation)) || (authMutation && !this.activity.snapshot().canSwitch)) {
      if (this.activity.switching && this.activity.isWork(message.method)) this.blockedWorkDuringSwitch = true;
      if (message.id !== undefined) this.frontend.send({ id: message.id, error: { code: -32001, message: 'Multi Codex is changing accounts. Retry when the account is verified.' } });
      return;
    }
    if (authMutation) {
      // Direct login in the official panel leaves managed-account mode explicitly.
      if (this.selectedId) void this.accounts.request('accounts/release', { id: this.selectedId }).catch(() => {});
      this.selectedId = null; this.selectedAuth = null; this.broadcast();
    }
    this.activity.outgoing(message); this.backendLines.send(message); this.broadcast();
  }
  private fromBackend(message: Message) {
    if (message.id === this.initializeId && !message.method) {
      this.activity.ready = !message.error;
    }
    if (message.method === 'account/chatgptAuthTokens/refresh' && message.id !== undefined && this.selectedId) {
      void this.refreshToken(message.id, message.params); return;
    }
    this.activity.incoming(message);
    if (!this.backendRpc.owns(message.id)) this.frontend.send(message);
    this.broadcast();
  }
  private async refreshToken(id: string | number, params: any) {
    this.credentialRequests.add(id); this.activity.pendingApprovals.add(id); this.broadcast();
    try {
      const selectedId = this.selectedId!;
      await this.accounts.withCredentialLock(selectedId, () => withAccountBackend(this.config.engine, join(this.config.dataRoot, 'profiles', selectedId, 'codex-home'), rpc => rpc.request('getAuthStatus', { includeToken: false, refreshToken: true })));
      const auth = await this.accounts.credential(selectedId);
      const login = loginParams(auth);
      if (login.chatgptAccountId !== params.previousAccountId || selectedId !== this.selectedId) throw new Error('Account identity changed while refreshing');
      this.selectedAuth = auth;
      this.backendLines.send({ id, result: { accessToken: login.accessToken, chatgptAccountId: login.chatgptAccountId, chatgptPlanType: login.chatgptPlanType } });
    } catch { this.backendLines.send({ id, error: { code: -32000, message: 'The saved account needs fresh credentials. Sign in again in Multi Codex.' } }); }
    finally { this.credentialRequests.delete(id); this.activity.pendingApprovals.delete(id); this.broadcast(); }
  }
  state() { return { ...this.activity.snapshot(), ...(this.connectionFailed ? { canSwitch: false, reason: this.connectionFailure?.includes('protocol') ? 'Codex protocol connection could no longer be verified. Save your work and reopen Codex, then retry.' : 'Codex backend disconnected. Save your work and reopen Codex, then retry.' } : {}), selectedId: this.selectedId, sourceHome: this.config.sourceHome ?? this.config.globalHome, bridgePid: process.pid, backendPid: this.backend.pid, instanceId: this.instanceId, connectionFailed: this.connectionFailed, connectionFailure: this.connectionFailure, authenticationUnverified: this.authenticationUnverified }; }
  private broadcast() { for (const client of this.clients.keys()) client.send({ method: 'state/changed', params: this.state() }); }
  async switchAccount(id: string) {
    if (typeof id !== 'string' || !id) throw new Error('Select a saved account');
    this.activity.acquire(); this.blockedWorkDuringSwitch = false; this.broadcast();
    const previousId = this.selectedId; let previousAuth = this.selectedAuth;
    let attempted = false;
    let leased = false;
    try {
      if (!previousAuth) previousAuth = await this.currentAuth();
      await this.accounts.request('accounts/lease', { id }); leased = true;
      let auth = await this.accounts.credential(id);
      let login;
      try { login = loginParams(auth); }
      catch (error) {
        if (error instanceof Error && error.message.includes('expired')) {
          await this.accounts.withCredentialLock(id, () => withAccountBackend(this.config.engine, join(this.config.dataRoot, 'profiles', id, 'codex-home'), rpc => rpc.request('getAuthStatus', { includeToken: false, refreshToken: true })));
          auth = await this.accounts.credential(id); login = loginParams(auth);
        } else throw error;
      }
      // Freeze frontend work before checking the authoritative backend. All loaded threads count.
      const loaded = await this.backendRpc.request('thread/loaded/list');
      if (!Array.isArray(loaded.data)) throw new Error('Codex activity could not be verified. Switching is blocked.');
      for (const threadId of loaded.data) {
        const result = await this.backendRpc.request('thread/read', { threadId, includeTurns: false });
        const status = result.thread?.status?.type;
        if (status !== 'idle' && status !== 'notLoaded') throw new Error('A Codex chat, subagent, or tool is still running.');
        if (this.activity.queuedThreads.has(threadId)) throw new Error('Resolve queued messages before switching.');
      }
      if (this.blockedWorkDuringSwitch || this.activity.unknown || this.activity.activeTurns.size || this.activity.activeThreads.size || this.activity.processes.size || this.activity.pendingWork.size || this.activity.pendingApprovals.size || this.credentialRequests.size) throw new Error('Codex became busy. Account switching was cancelled.');
      if (this.config.attached && login.type === 'apiKey') throw new Error('API-key switching requires the isolated bridge on the next Codex startup. Live attachment never writes the original Codex login.');
      attempted = true;
      // The official backend requires clearing external ChatGPT authentication
      // before adopting an API key. This keeps the same process and panel.
      if (login.type === 'apiKey') await this.backendRpc.request('account/logout');
      await this.backendRpc.request('account/login/start', login);
      const status = await this.backendRpc.request('getAuthStatus', { includeToken: true, refreshToken: false });
      if (!matchesIdentity(status, auth)) throw new Error('Codex did not confirm the selected account.');
      this.selectedId = id; this.selectedAuth = auth;
      if (previousId && previousId !== id) await this.accounts.request('accounts/release', { id: previousId });
      return this.state();
    } catch (error) {
      if (attempted) {
        try {
          if (previousAuth) {
            const previousLogin = loginParams(previousAuth);
            if (previousLogin.type === 'apiKey') await this.backendRpc.request('account/logout');
            await this.backendRpc.request('account/login/start', previousLogin);
            const status = await this.backendRpc.request('getAuthStatus', { includeToken: true, refreshToken: false });
            if (!matchesIdentity(status, previousAuth)) throw new Error('Rollback identity mismatch');
            this.selectedId = previousId; this.selectedAuth = previousAuth;
          } else if (this.config.attached) { throw new Error('No original identity is available for rollback'); }
          else { await this.backendRpc.request('account/logout'); this.selectedId = null; this.selectedAuth = null; }
        } catch { this.activity.unknown = true; this.authenticationUnverified = true; this.selectedId = null; this.selectedAuth = null; }
      }
      if (leased && id !== previousId) await this.accounts.request('accounts/release', { id }).catch(() => {});
      throw error;
    } finally { this.activity.switching = false; this.broadcast(); }
  }
  private async control(method: string, params: any) {
    switch (method) {
      case 'state': return this.state();
      case 'switch': return this.switchAccount(params.id);
      case 'usage': {
        if (this.activity.switching) throw new Error('Wait for the account switch before checking usage');
        const selectedId = this.selectedId;
        const result = await this.backendRpc.request('account/rateLimits/read');
        if (selectedId !== this.selectedId || this.activity.switching) throw new Error('The active account changed while checking usage. Check again.');
        return result;
      }
      case 'test/request':
        if (process.env.MULTI_CODEX_TEST_MODE !== '1') throw new Error('Test requests are unavailable');
        if (this.activity.switching && this.activity.isWork(params.method)) throw new Error('Switching is in progress');
        const requestId = randomUUID();
        this.activity.outgoing({ id: requestId, method: params.method });
        try { return await this.backendRpc.request(params.method, params.params ?? {}); }
        finally { this.activity.pendingWork.delete(requestId); this.broadcast(); }
      default: throw new Error('Unknown bridge request');
    }
  }
  private fail(reason: string) { if (this.connectionFailed) return; this.connectionFailure = reason; this.connectionFailed = true; this.activity.ready = false; this.backendRpc.close(); this.broadcast(); }
  private disposal?: Promise<void>;
  dispose() { return this.disposal ??= (async () => {
    this.disposed = true; this.fail('bridge-disposed');
    for (const [client, socket] of this.clients) { client.send({ method: 'disconnected' }); socket.end(); socket.destroy(); }
    const serverClosed = this.server ? new Promise<void>(resolve => this.server!.close(() => resolve())) : Promise.resolve();
    if (this.existing) this.existing.restore();
    await Promise.all([this.accounts.dispose(), this.existing ? Promise.resolve() : terminateExecutable(this.backend), serverClosed]);
    if (process.platform !== 'win32') await rm(this.config.socket, { force: true });
  })(); }

}

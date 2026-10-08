import * as vscode from 'vscode';
import { createHash, randomBytes, randomUUID } from 'node:crypto';
import { access, chmod, mkdir, readFile, writeFile } from 'node:fs/promises';
import { join, isAbsolute } from 'node:path';
import { homedir, tmpdir } from 'node:os';
import { spawn } from 'node:child_process';
import { AccountClient, type Account } from './accounts';
import { BridgeClient } from './client';
import { JsonLines, RpcPeer } from './protocol';
import { withAccountBackend } from './worker';
import { CodexBridge } from './bridge';
import { findCodexBackend, tapBackend } from './stdio-tap';

function dataDirectory() {
  if (process.platform === 'darwin') return join(homedir(), 'Library', 'Application Support', 'multi-codex');
  if (process.platform === 'linux') return join(process.env.XDG_DATA_HOME || join(homedir(), '.local', 'share'), 'multi-codex');
  throw new Error('This local preview supports Linux and macOS. Windows support needs a native account helper.');
}
function bundledEngine(extension: vscode.Extension<any>) {
  const architecture = process.arch === 'arm64' ? 'aarch64' : 'x86_64';
  return join(extension.extensionPath, 'bin', `${process.platform}-${architecture}`, 'codex');
}
function quote(value: string) { return `'${value.replace(/'/g, `'"'"'`)}'`; }

class AccountsTree implements vscode.TreeDataProvider<Account> {
  private emitter = new vscode.EventEmitter<void>();
  readonly onDidChangeTreeData = this.emitter.event;
  accounts: Account[] = [];
  state: any = null;
  refresh() { this.emitter.fire(); }
  getChildren() { return this.accounts; }
  getTreeItem(account: Account) {
    const item = new vscode.TreeItem(account.name);
    item.id = account.id;
    item.description = `${account.accountTier ?? account.authMode}${this.state?.selectedId === account.id ? ' · Active' : ''}`;
    item.tooltip = account.notes || `Use ${account.name} in this Codex window`;
    item.contextValue = 'multiCodexAccount';
    item.iconPath = new vscode.ThemeIcon(this.state?.selectedId === account.id ? 'account' : 'person');
    item.command = { command: 'multiCodex.switchAccount', title: 'Switch account', arguments: [account] };
    return item;
  }
}

export async function activate(context: vscode.ExtensionContext) {
  const activationId = randomUUID();
  const tree = new AccountsTree();
  const status = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Right, 110);
  status.command = 'multiCodex.pickAccount'; status.text = '$(account) Multi Codex'; status.show();
  context.subscriptions.push(status, vscode.window.registerTreeDataProvider('multiCodex.accounts', tree));
  let accounts: AccountClient | undefined; let bridge: BridgeClient | undefined;
  let runtimeRoot = ''; let engine = ''; let helper = ''; let root = ''; let globalHome = '';
  let monitor: NodeJS.Timeout | undefined; let connecting = false; let accountsRefreshing = false;
  let usage: any = null; let usageAt = 0;
  let setupPending: Promise<void> | undefined;
  function display() {
    tree.state = bridge?.state; tree.refresh();
    const state = bridge?.state;
    const account = tree.accounts.find(account => account.id === state?.selectedId);
    status.text = state?.switching ? '$(sync~spin) Switching Codex…' : account ? `$(account) ${account.name}` : '$(account) Multi Codex';
    status.tooltip = !bridge ? 'Connect Multi Codex to the Codex extension' : state?.reason ?? (account ? `Codex is using ${account.name}` : 'Choose a saved Codex account');
  }
  async function refreshAccounts() {
    if (!accounts || accountsRefreshing) return; accountsRefreshing = true;
    try { tree.accounts = await accounts.list(); display(); } finally { accountsRefreshing = false; }
  }
  async function connect() {
    if (!runtimeRoot || bridge || connecting) return; connecting = true;
    try {
      const registration = JSON.parse(await readFile(join(runtimeRoot, 'connections', `${process.pid}.json`), 'utf8'));
      bridge = await BridgeClient.connect(registration.socket, registration.token);
      const thisBridge = bridge;
      thisBridge.on('state', () => display());
      thisBridge.on('close', () => { if (bridge === thisBridge) { bridge = undefined; display(); } });
      display();
    } catch { /* Discovery is expected to fail until the backend starts through the bridge. */ }
    finally { connecting = false; }
  }
  async function prepare() {
    if (vscode.env.remoteName) throw new Error('Remote SSH, containers and WSL need a separate bridge setup. This preview supports local VS Code.');
    const codex = vscode.extensions.getExtension('openai.chatgpt');
    if (!codex) throw new Error('Install the official Codex extension first.');
    const settings = vscode.workspace.getConfiguration('multiCodex');
    root = settings.get<string>('dataDirectory') || dataDirectory();
    const desktopSettings = await readFile(join(root, 'executables.json'), 'utf8').then(text => JSON.parse(text)).catch(() => ({}));
    globalHome = settings.get<string>('globalCodexHome') || process.env.CODEX_HOME || desktopSettings.globalCodexHome || join(homedir(), '.codex');
    if (!isAbsolute(root) || !isAbsolute(globalHome)) throw new Error('Account storage and Codex home paths must be absolute.');
    engine = bundledEngine(codex); await access(engine);
    runtimeRoot = join(context.globalStorageUri.fsPath, 'runtime');
    helper = join(context.extensionPath, 'bin', 'multi-codex-account-helper'); await access(helper);
    await mkdir(root, { recursive: true, mode: 0o700 }); await mkdir(runtimeRoot, { recursive: true, mode: 0o700 });
    accounts = new AccountClient(helper, engine, root, globalHome);
    await refreshAccounts();
    monitor = setInterval(() => { void connect(); void refreshAccounts().catch(() => {}); }, 1500);
    context.subscriptions.push({ dispose() { if (monitor) clearInterval(monitor); bridge?.dispose(); accounts?.dispose(); } });
    await connect();
  }
  async function setupBridge() {
    if (setupPending) return setupPending;
    setupPending = configureBridge();
    try { await setupPending; } finally { setupPending = undefined; }
  }
  async function configureBridge() {
    if (!accounts) await prepare();
    const current = vscode.workspace.getConfiguration('chatgpt').get<string>('cliExecutable');
    const wrapper = join(runtimeRoot, 'codex-bridge');
    if (current && current !== wrapper) throw new Error('Codex already has a custom executable. Restore its bundled executable before enabling Multi Codex.');
    const config = { engine, helper, dataRoot: root, globalHome, historyHome: process.env.CODEX_SQLITE_HOME || globalHome, runtimeRoot };
    const configPath = join(runtimeRoot, 'bridge-config.json');
    await writeFile(configPath, JSON.stringify(config), { mode: 0o600 });
    await writeFile(wrapper, `#!/bin/sh\nexport ELECTRON_RUN_AS_NODE=1\nexec ${quote(process.execPath)} ${quote(join(context.extensionPath, 'dist', 'bridge-main.cjs'))} --bridge-config ${quote(configPath)} "$@"\n`, { mode: 0o700 });
    await chmod(wrapper, 0o700);
    if (current !== wrapper) await context.globalState.update('previousCliExecutable', current ?? null);
    await vscode.workspace.getConfiguration('chatgpt').update('cliExecutable', wrapper, vscode.ConfigurationTarget.Global);
    await connect();
    if (!bridge) {
      const child = findCodexBackend(engine);
      if (child) {
        const session = randomUUID();
        const sessionHome = join(runtimeRoot, 'attached', session);
        const workspace = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath || homedir();
        const selections = join(runtimeRoot, 'selections');
        await mkdir(selections, { recursive: true, mode: 0o700 });
        const selectionPath = join(selections, createHash('sha256').update(workspace).digest('hex') + '.json');
        await mkdir(sessionHome, { recursive: true, mode: 0o700 });
        const tap = tapBackend(child);
        const live = new CodexBridge({ engine, helper, dataRoot: root, globalHome, sessionHome,
          workspace, selectionPath, attached: true,
          socket: join(tmpdir(), `mc-${process.getuid?.() ?? 'user'}-${session.slice(0, 12)}.sock`), token: randomBytes(32).toString('hex') }, tap.frontend, [], tap);
        try {
          await live.attachReady(); await live.listen();
          bridge = await BridgeClient.connect(live.config.socket, live.config.token);
          bridge.on('state', display);
          bridge.on('close', () => { bridge = undefined; display(); });
          context.subscriptions.push({ dispose() { void live.dispose(); } });
        } catch (error) { await live.dispose(); throw error; }
      } else {
        const codex = vscode.extensions.getExtension('openai.chatgpt')!;
        if (!codex.isActive) { await codex.activate(); await connect(); }
      }
    }
    display();
    if (!bridge) throw new Error('This Codex build does not expose a compatible local backend. The isolated bridge is configured for the next Codex startup; no window was reloaded.');
    vscode.window.showInformationMessage('Multi Codex is connected. Account switching is ready without reloading VS Code or Codex.');
  }
  async function switchAccount(accountOrId?: Account | string) {
    await connect(); if (!bridge) throw new Error('The Codex bridge is not connected. Run Multi Codex: Enable Account Switching first.');
    let id = typeof accountOrId === 'string' ? accountOrId : accountOrId?.id;
    if (!id) {
      const selected = await vscode.window.showQuickPick(tree.accounts.map(account => ({ label: account.name, description: account.accountTier ?? account.authMode, id: account.id })), { title: 'Use account in this Codex window' });
      if (!selected) return; id = selected.id;
    }
    const state = await bridge.request('state');
    if (!state.canSwitch) throw new Error(state.reason);
    await vscode.window.withProgress({ location: vscode.ProgressLocation.Notification, title: 'Switching Codex account', cancellable: false }, async () => { await bridge!.request('switch', { id }); });
    bridge.state = await bridge.request('state'); usage = null; usageAt = 0; display();
  }
  async function refreshUsage() {
    await connect(); if (!bridge?.state.selectedId) throw new Error('Choose an account before checking usage.');
    usage = await bridge.request('usage'); usageAt = Date.now();
    const windows = [usage.rateLimits?.primary, usage.rateLimits?.secondary].filter(Boolean);
    const text = windows.map((window: any) => `${window.windowDurationMins >= 10080 ? 'Weekly' : 'Short-term'}: ${Math.max(0, 100 - window.usedPercent)}% remaining`).join(' · ');
    status.tooltip = `${status.tooltip}\n${text}\nChecked ${new Date(usageAt).toLocaleTimeString()}`;
    await vscode.window.showInformationMessage(text || 'Codex did not return usage windows for this account.'); return usage;
  }
  async function addAccount() {
    if (!accounts) throw new Error('Account storage is not ready');
    const name = await vscode.window.showInputBox({ title: 'Account name', prompt: 'A label for this Codex account', validateInput: value => !value.trim() ? 'Enter an account name' : undefined });
    if (!name) return;
    const choice = await vscode.window.showQuickPick(['Sign in with browser', 'Import current Codex login', 'Import auth JSON file'], { title: 'Add Codex account' });
    if (!choice) return;
    if (choice === 'Import current Codex login') { await accounts.request('accounts/import', { name }); await refreshAccounts(); return; }
    if (choice === 'Import auth JSON file') {
      const files = await vscode.window.showOpenDialog({ canSelectMany: false, filters: { 'Codex auth JSON': ['json'] } }); if (!files?.length) return;
      const authJson = await readFile(files[0].fsPath, 'utf8'); await accounts.request('accounts/add', { name, authJson }); await refreshAccounts(); return;
    }
    const directory = join(runtimeRoot, 'logins', randomUUID()); await mkdir(directory, { recursive: true, mode: 0o700 });
    await writeFile(join(directory, 'config.toml'), 'cli_auth_credentials_store = "file"\n[analytics]\nenabled = false\n', { mode: 0o600 });
    const loginEnvironment: NodeJS.ProcessEnv = { ...process.env, CODEX_HOME: directory, CODEX_SQLITE_HOME: directory };
    delete loginEnvironment.OPENAI_API_KEY; delete loginEnvironment.CODEX_API_KEY; delete loginEnvironment.ELECTRON_RUN_AS_NODE;
    const child = spawn(engine, ['app-server'], { env: loginEnvironment, stdio: ['pipe', 'pipe', 'pipe'] });
    child.stderr.resume(); const lines = new JsonLines(child.stdout, child.stdin); const rpc = new RpcPeer(lines, 'login:');
    try {
      await rpc.request('initialize', { clientInfo: { name: 'multi_codex_login', version: '0.1.0' } }); lines.send({ method: 'initialized' });
      await vscode.window.withProgress({ location: vscode.ProgressLocation.Notification, title: 'Finish signing in to Codex in your browser', cancellable: true }, async (_, cancellation) => {
        const complete: Promise<void> = new Promise((resolve, reject) => {
          lines.on('message', message => { if (message.method === 'account/login/completed') message.params?.success ? resolve() : reject(new Error('Codex sign-in did not complete')); });
          cancellation.onCancellationRequested(() => reject(new Error('Sign-in cancelled')));
          child.once('exit', () => reject(new Error('Codex sign-in closed')));
        });
        // Attach a handler before starting browser login: cancellation can occur
        // while login/start is still awaiting its response.
        void complete.catch(() => {});
        let timeout: NodeJS.Timeout | undefined;
        try {
          const login = await rpc.request('account/login/start', { type: 'chatgpt' });
          if (!await vscode.env.openExternal(vscode.Uri.parse(login.authUrl))) throw new Error('The browser could not open the Codex sign-in page');
          await Promise.race([complete, new Promise((_, reject) => { timeout = setTimeout(() => reject(new Error('Sign-in timed out. Try adding the account again.')), 5 * 60 * 1000); })]);
        } finally { if (timeout) clearTimeout(timeout); }
        const authJson = await readFile(join(directory, 'auth.json'), 'utf8'); await accounts!.request('accounts/add', { name, authJson });
      });
    } finally { rpc.close(); child.kill(); const fs = await import('node:fs/promises'); await fs.rm(directory, { recursive: true, force: true }); }
    await refreshAccounts();
  }
  function command(name: string, action: (...args: any[]) => Promise<any>) {
    context.subscriptions.push(vscode.commands.registerCommand(name, async (...args) => { try { return await action(...args); } catch (error) { vscode.window.showErrorMessage(error instanceof Error ? error.message : 'Multi Codex request failed'); } }));
  }
  command('multiCodex.enableSwitching', setupBridge);
  command('multiCodex.switchAccount', switchAccount);
  command('multiCodex.pickAccount', () => switchAccount());
  command('multiCodex.refreshAccounts', refreshAccounts);
  command('multiCodex.refreshUsage', refreshUsage);
  command('multiCodex.checkAccountUsage', async (account: Account) => {
    if (!accounts) throw new Error('Account storage is not ready');
    if (!tree.accounts.some(saved => saved.id === account.id)) throw new Error('Account not found');
    const result = await accounts.withCredentialLock(account.id, async () => { await accounts!.credential(account.id); return withAccountBackend(engine, join(root, 'profiles', account.id, 'codex-home'), rpc => rpc.request('account/rateLimits/read')); });
    const windows = [result.rateLimits?.primary, result.rateLimits?.secondary].filter(Boolean);
    vscode.window.showInformationMessage(`${account.name}: ${windows.map((window: any) => `${window.windowDurationMins >= 10080 ? 'Weekly' : 'Short-term'} ${Math.max(0, 100 - window.usedPercent)}% remaining`).join(' · ') || 'Usage is unavailable'}`);
    return result;
  });
  command('multiCodex.addAccount', addAccount);
  command('multiCodex.renameAccount', async (account: Account) => { const name = await vscode.window.showInputBox({ title: 'Rename account', value: account.name }); if (name) { await accounts!.request('accounts/rename', { id: account.id, name }); await refreshAccounts(); } });
  command('multiCodex.removeAccount', async (account: Account) => { if (bridge?.state.selectedId === account.id) throw new Error('Switch away from this account before removing it.'); const answer = await vscode.window.showWarningMessage(`Remove saved account “${account.name}”?`, { modal: true }, 'Remove'); if (answer === 'Remove') { await accounts!.request('accounts/delete', { id: account.id }); await refreshAccounts(); } });
  command('multiCodex.disableSwitching', async () => { const wrapper = join(runtimeRoot, 'codex-bridge'); const settings = vscode.workspace.getConfiguration('chatgpt'); if (settings.get('cliExecutable') === wrapper) await settings.update('cliExecutable', context.globalState.get('previousCliExecutable', null), vscode.ConfigurationTarget.Global); vscode.window.showInformationMessage('The bundled Codex executable will be restored the next time Codex starts.'); });
  try { await prepare(); } catch (error) { status.tooltip = error instanceof Error ? error.message : 'Multi Codex setup is incomplete'; }
  return {
    activationId, refreshAccounts, switchAccount, enableSwitching: setupBridge,
    async getState() { await connect(); return { activationId, extensionHostPid: process.pid, accounts: tree.accounts, bridge: bridge ? await bridge.request('state') : null }; },
    ...(process.env.MULTI_CODEX_TEST_MODE === '1' ? {
      testBackendInfo() { return { pid: findCodexBackend(engine)?.pid, activationId, hostPid: process.pid }; },
      async testRequest(method: string, params: any = {}) { await connect(); if (!bridge) throw new Error('Bridge not connected'); return bridge.request('test/request', { method, params }); }
    } : {}),
  };
}

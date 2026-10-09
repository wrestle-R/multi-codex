import * as vscode from 'vscode';
import { randomBytes, randomUUID } from 'node:crypto';
import { access, copyFile, mkdir, readFile, rename, rm, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { homedir, tmpdir } from 'node:os';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { controlEndpoint, helperName, wrapperName, spawnExecutable, terminateExecutable } from './native';
import { usageSummary, refreshAllAccounts } from './usage';
import { AccountClient, type Account } from './accounts';
import { BridgeClient } from './client';
import { JsonLines, RpcPeer } from './protocol';
import { withAccountBackend } from './worker';
import { CodexBridge } from './bridge';
import { findCodexBackend, tapBackend } from './stdio-tap';
import { bundledEnginePath } from './platform';
import { resolveAccountPaths } from './storage';
import { extensionVersion } from './version';
function quote(value: string) { return `'${value.replace(/'/g, `'"'"'`)}'`; }

class AccountsTree implements vscode.TreeDataProvider<Account> {
  private emitter = new vscode.EventEmitter<void>();
  readonly onDidChangeTreeData = this.emitter.event;
  accounts: Account[] = [];
  state: any = null;
  usage = new Map<string, string>();
  checkedAt = new Map<string, number>();
  checking = new Set<string>();
  refresh() { this.emitter.fire(); }
  getChildren() { return this.accounts; }
  getTreeItem(account: Account) {
    const item = new vscode.TreeItem(account.name);
    item.id = account.id;
    const active = this.state?.selectedId === account.id;
    item.description = [active ? `Active · ${account.accountTier ?? account.authMode}` : account.accountTier ?? account.authMode, this.checking.has(account.id) ? 'Checking usage…' : this.usage.get(account.id)].filter(Boolean).join(' · ');
    item.tooltip = [account.name, account.accountTier ?? account.authMode, active ? 'Currently used by Codex' : 'Click to use this account in the current Codex window', this.usage.get(account.id), account.notes].filter(Boolean).join('\n');
    item.contextValue = 'multiCodexAccount';
    item.iconPath = new vscode.ThemeIcon(this.checking.has(account.id) ? 'sync~spin' : active ? 'pass-filled' : account.error ? 'warning' : 'account', active ? new vscode.ThemeColor('testing.iconPassed') : undefined);
    const checked = this.checkedAt.get(account.id);
    if (checked) item.tooltip += `\nChecked ${new Date(checked).toLocaleTimeString()}`;
    if (account.error) item.tooltip += `\n${account.error}`;
    item.command = { command: 'multiCodex.switchAccount', title: 'Switch account', arguments: [account] };
    return item;
  }
}

export async function activate(context: vscode.ExtensionContext) {
  const activationId = randomUUID();
  const tree = new AccountsTree();
  const view = vscode.window.createTreeView('multiCodex.accounts', { treeDataProvider: tree });
  view.message = 'Loading saved accounts…';
  const status = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Right, 110);
  status.command = 'multiCodex.pickAccount'; status.text = '$(account) Multi Codex'; status.show();
  context.subscriptions.push(status, view);
  let accounts: AccountClient | undefined; let bridge: BridgeClient | undefined;
  let runtimeRoot = ''; let engine = ''; let helper = ''; let root = ''; let globalHome = '';
  let monitor: NodeJS.Timeout | undefined; let connecting = false; let accountsRefreshing = false;
  let usage: any = null; let usageAt = 0;
  let refreshingUsage: Promise<void> | undefined;
  const usageOperations = new Map<string, Promise<unknown>>();
  let setupPending: Promise<void> | undefined;
  let preparePending: Promise<void> | undefined; let storageError = '';
  function display() {
    tree.state = bridge?.state; tree.refresh();
    const state = bridge?.state;
    const account = tree.accounts.find(account => account.id === state?.selectedId);
    view.description = tree.accounts.length ? `${tree.accounts.length} saved` : undefined;
    view.message = storageError ? `${storageError}\nUse Refresh above to retry.`
      : !accounts ? 'Loading saved accounts…'
      : !tree.accounts.length ? 'No saved accounts yet. Use + above to sign in or import an account.'
      : setupPending ? 'Connecting to Codex…'
      : state?.switching ? 'Switching account…'
      : state?.ready && !state.canSwitch ? state.reason
      : undefined;
    status.text = setupPending ? '$(sync~spin) Connecting Codex…' : state?.switching ? '$(sync~spin) Switching Codex…' : account ? `$(account) ${account.name}` : state?.ready ? '$(account) Codex · Current home' : '$(account) Multi Codex';
    status.tooltip = storageError || (!bridge ? 'Choose an account to connect to Codex automatically' : state?.reason ?? (account ? `Codex is using ${account.name}` : `Codex started from ${state?.sourceHome ?? globalHome}\nChoose an account to change this window’s login`));
    if (account && tree.usage.has(account.id)) status.tooltip += `\n${tree.usage.get(account.id)}`;
  }
  async function refreshAccounts() {
    if (!accounts?.isRunning) { await prepare(); return; }
    if (accountsRefreshing) return; accountsRefreshing = true;
    try { tree.accounts = await accounts.list(); storageError = ''; display(); }
    catch (error) { storageError = error instanceof Error ? error.message : 'Could not load saved accounts'; display(); throw error; }
    finally { accountsRefreshing = false; }
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
    if (preparePending) return preparePending;
    preparePending = prepareStorage();
    try { await preparePending; }
    catch (error) { storageError = error instanceof Error ? error.message : 'Could not open saved accounts'; display(); throw error; }
    finally { preparePending = undefined; }
  }
  async function prepareStorage() {
    if (vscode.env.remoteName) throw new Error('Remote SSH, containers and WSL need a separate bridge setup. This preview supports local VS Code.');
    const codex = vscode.extensions.getExtension('openai.chatgpt');
    if (!codex) throw new Error('Install the official Codex extension first.');
    const settings = vscode.workspace.getConfiguration('multiCodex');
    const paths = await resolveAccountPaths({ dataDirectory: settings.get<string>('dataDirectory'), globalCodexHome: settings.get<string>('globalCodexHome') });
    root = paths.dataRoot; globalHome = paths.globalHome;
    engine = await bundledEnginePath(codex.extensionPath);
    runtimeRoot = join(context.globalStorageUri.fsPath, 'runtime');
    helper = join(context.extensionPath, 'bin', helperName); await access(helper);
    await mkdir(root, { recursive: true, mode: 0o700 }); await mkdir(runtimeRoot, { recursive: true, mode: 0o700 });
    if (process.platform === 'win32') await promisify(execFile)(helper, ['--protect-directory', runtimeRoot], { windowsHide: true });
    await accounts?.dispose();
    accounts = new AccountClient(helper, engine, root, globalHome);
    // Refresh an existing launcher before another normal Codex startup. Account
    // discovery's global home must never replace an inherited desktop account home.
    if (vscode.workspace.getConfiguration('chatgpt').get<string>('cliExecutable') === join(runtimeRoot, wrapperName)) await writeBridgeLauncher();
    try { await refreshAccounts(); }
    catch (error) { await accounts.dispose(); accounts = undefined; throw error; }
    if (monitor) clearInterval(monitor);
    monitor = setInterval(() => { void connect(); void refreshAccounts().catch(() => {}); }, 1500);
    context.subscriptions.push({ dispose() { if (monitor) clearInterval(monitor); bridge?.dispose(); accounts?.dispose(); } });
    await connect();
  }
  async function setupBridge() {
    if (setupPending) return setupPending;
    setupPending = configureBridge();
    display();
    try { await setupPending; } finally { setupPending = undefined; display(); }
  }
  async function configureBridge() {
    if (!accounts?.isRunning) await prepare();
    const current = vscode.workspace.getConfiguration('chatgpt').get<string>('cliExecutable');
    const wrapper = join(runtimeRoot, wrapperName);
    if (current && current !== wrapper) throw new Error('Codex already has a custom executable. Restore its bundled executable before enabling Multi Codex.');
    await writeBridgeLauncher();
    if (current !== wrapper) await context.globalState.update('previousCliExecutable', current ?? null);
    await vscode.workspace.getConfiguration('chatgpt').update('cliExecutable', wrapper, vscode.ConfigurationTarget.Global);
    await connect();
    if (!bridge) {
      const child = findCodexBackend(engine);
      if (child) {
        const session = randomUUID();
        const sessionHome = join(runtimeRoot, 'attached', session);
        const workspace = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath || homedir();
        await mkdir(sessionHome, { recursive: true, mode: 0o700 });
        const tap = tapBackend(child);
        const live = new CodexBridge({ engine, helper, dataRoot: root, globalHome, sessionHome,
          workspace, sourceHome: process.env.CODEX_HOME || globalHome, attached: true,
          socket: controlEndpoint(session), token: randomBytes(32).toString('hex') }, tap.frontend, [], tap);
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
  }
  async function writeBridgeLauncher() {
    const config = { engine, helper, dataRoot: root, globalHome, startupHome: process.env.CODEX_HOME || globalHome,
      officialExtensionPath: vscode.extensions.getExtension('openai.chatgpt')!.extensionPath, runtimeRoot, nodeExecutable: process.execPath, bridgeScript: join(context.extensionPath, 'dist', 'bridge-main.cjs') };
    const configPath = join(runtimeRoot, process.platform === 'win32' ? 'codex-bridge.json' : 'bridge-config.json');
    async function replaceFile(path: string, contents: string, mode: number) {
      const temporary = `${path}.${randomUUID()}.tmp`;
      try { await writeFile(temporary, contents, { mode }); await rename(temporary, path); }
      finally { await rm(temporary, { force: true }); }
    }
    await replaceFile(configPath, JSON.stringify(config), 0o600);
    const wrapper = join(runtimeRoot, wrapperName);
    if (process.platform === 'win32') {
      const source = await readFile(helper);
      const existing = await readFile(wrapper).catch((error: NodeJS.ErrnoException) => { if (error.code !== 'ENOENT') throw error; return undefined; });
      // An active Windows executable is locked. Reactivation of the same
      // extension only refreshes its config, without rewriting its wrapper.
      if (existing?.equals(source)) return;
      const temporary = `${wrapper}.${randomUUID()}.tmp`;
      try { await copyFile(helper, temporary); await rename(temporary, wrapper); }
      catch (error) { throw new Error(`Could not update the Codex wrapper. Close Codex windows using Multi Codex and retry. ${error instanceof Error ? error.message : error}`); }
      finally { await rm(temporary, { force: true }); }
      return;
    }
    await replaceFile(wrapper, `#!/bin/sh\nexport ELECTRON_RUN_AS_NODE=1\nexec ${quote(process.execPath)} ${quote(join(context.extensionPath, 'dist', 'bridge-main.cjs'))} --bridge-config ${quote(configPath)} "$@"\n`, 0o700);
  }
  async function switchAccount(accountOrId?: Account | string) {
    if (!accounts?.isRunning) await prepare();
    let id = typeof accountOrId === 'string' ? accountOrId : accountOrId?.id;
    if (!id) {
      const selected = await vscode.window.showQuickPick(tree.accounts.map(account => ({ label: account.name, description: account.accountTier ?? account.authMode, id: account.id })), { title: 'Use account in this Codex window' });
      if (!selected) return; id = selected.id;
    }
    await connect(); if (!bridge) await setupBridge();
    if (!bridge) throw new Error('Codex could not connect. Open its panel and retry.');
    const state = await bridge.request('state');
    if (!state.canSwitch) throw new Error(state.reason);
    await vscode.window.withProgress({ location: vscode.ProgressLocation.Notification, title: 'Switching Codex account', cancellable: false }, async () => { await bridge!.request('switch', { id }); });
    bridge.state = await bridge.request('state'); usage = null; usageAt = 0; display();
    const selected = tree.accounts.find(account => account.id === id);
    void vscode.window.showInformationMessage(`Codex is now using ${selected?.name ?? 'the selected account'}. Login verified. This window’s conversations and home resources are preserved.`);
  }
  async function refreshUsage() {
    if (refreshingUsage) return refreshingUsage;
    refreshingUsage = (async () => {
      if (!accounts?.isRunning) await prepare();
      await refreshAccounts(); await connect();
      const saved = [...tree.accounts];
      if (!saved.length) { void vscode.window.showInformationMessage('Add an account to check usage.'); return; }
      await vscode.window.withProgress({ location: { viewId: 'multiCodex.accounts' }, title: 'Checking all accounts' }, async progress => {
        let finished = 0; let failed = 0;
        await refreshAllAccounts(saved, checkAccountUsage, (account, error) => {
          finished++; if (error) { failed++; tree.usage.set(account.id, 'Check failed'); }
          progress.report({ message: `${finished}/${saved.length} accounts`, increment: 100 / saved.length }); display();
        });
        if (failed) void vscode.window.showWarningMessage(`Checked ${saved.length} accounts. ${failed} could not refresh; use the account’s usage action to retry and see its error.`);
      });
    })();
    try { await refreshingUsage; } finally { refreshingUsage = undefined; }
  }
  function checkAccountUsage(account: Account): Promise<unknown> {
    const existing = usageOperations.get(account.id); if (existing) return existing;
    const pending = readAccountUsage(account).finally(() => usageOperations.delete(account.id));
    usageOperations.set(account.id, pending); return pending;
  }
  async function readAccountUsage(account: Account) {
    if (!accounts?.isRunning) await prepare();
    if (!tree.accounts.some(saved => saved.id === account.id)) throw new Error('Account not found');
    tree.checking.add(account.id); display();
    try {
      const result = bridge?.state.selectedId === account.id ? await bridge.request('usage')
        : await accounts!.withCredentialLock(account.id, async () => { await accounts!.credential(account.id); return withAccountBackend(engine, join(root, 'profiles', account.id, 'codex-home'), rpc => rpc.request('account/rateLimits/read')); });
      tree.usage.set(account.id, usageSummary(result)); tree.checkedAt.set(account.id, Date.now());
      if (bridge?.state.selectedId === account.id) { usage = result; usageAt = Date.now(); }
      return result;
    } finally { tree.checking.delete(account.id); display(); }
  }

  async function addAccount() {
    if (!accounts?.isRunning) await prepare();
    const name = await vscode.window.showInputBox({ title: 'Account name', prompt: 'A label for this Codex account', validateInput: value => !value.trim() ? 'Enter an account name' : undefined });
    if (!name) return;
    const choice = await vscode.window.showQuickPick(['Sign in with browser', 'Import current Codex login', 'Import auth JSON file'], { title: 'Add Codex account' });
    if (!choice) return;
    if (choice === 'Import current Codex login') {
      const currentHome = process.env.CODEX_HOME || globalHome;
      const authJson = await readFile(join(currentHome, 'auth.json'), 'utf8');
      await accounts!.request('accounts/add', { name, authJson }); await refreshAccounts(); return;
    }
    if (choice === 'Import auth JSON file') {
      const files = await vscode.window.showOpenDialog({ canSelectMany: false, filters: { 'Codex auth JSON': ['json'] } }); if (!files?.length) return;
      const authJson = await readFile(files[0].fsPath, 'utf8'); await accounts!.request('accounts/add', { name, authJson }); await refreshAccounts(); return;
    }
    const directory = join(runtimeRoot, 'logins', randomUUID()); await mkdir(directory, { recursive: true, mode: 0o700 });
    await writeFile(join(directory, 'config.toml'), 'cli_auth_credentials_store = "file"\n[analytics]\nenabled = false\n', { mode: 0o600 });
    const loginEnvironment: NodeJS.ProcessEnv = { ...process.env, CODEX_HOME: directory, CODEX_SQLITE_HOME: directory };
    delete loginEnvironment.OPENAI_API_KEY; delete loginEnvironment.CODEX_API_KEY; delete loginEnvironment.ELECTRON_RUN_AS_NODE;
    const child = spawnExecutable(engine, ['app-server'], { env: loginEnvironment });
    child.stderr.resume(); const lines = new JsonLines(child.stdout, child.stdin); const rpc = new RpcPeer(lines, 'login:');
    try {
      await rpc.request('initialize', { clientInfo: { name: 'multi_codex_login', version: extensionVersion } }); lines.send({ method: 'initialized' });
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
    } finally { rpc.close(); await terminateExecutable(child); const fs = await import('node:fs/promises'); await fs.rm(directory, { recursive: true, force: true }); }
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
  command('multiCodex.checkAccountUsage', checkAccountUsage);
  command('multiCodex.addAccount', addAccount);
  command('multiCodex.renameAccount', async (account: Account) => { const name = await vscode.window.showInputBox({ title: 'Rename account', value: account.name }); if (name) { await accounts!.request('accounts/rename', { id: account.id, name }); await refreshAccounts(); } });
  command('multiCodex.removeAccount', async (account: Account) => { if (bridge?.state.selectedId === account.id) throw new Error('Switch away from this account before removing it.'); const answer = await vscode.window.showWarningMessage(`Remove saved account “${account.name}”?`, { modal: true }, 'Remove'); if (answer === 'Remove') { await accounts!.request('accounts/delete', { id: account.id }); await refreshAccounts(); } });
  command('multiCodex.disableSwitching', async () => { const wrapper = join(runtimeRoot, wrapperName); const settings = vscode.workspace.getConfiguration('chatgpt'); if (settings.get('cliExecutable') === wrapper) await settings.update('cliExecutable', context.globalState.get('previousCliExecutable', null), vscode.ConfigurationTarget.Global); vscode.window.showInformationMessage('The bundled Codex executable will be restored the next time Codex starts.'); });
  try { await prepare(); } catch { /* Storage errors are shown inline with a retry action. */ }
  return {
    activationId, refreshAccounts, switchAccount, enableSwitching: setupBridge,
    async getState() { await connect(); return { activationId, extensionHostPid: process.pid, accounts: tree.accounts, storageError, panelMessage: view.message, usageSummaries: Object.fromEntries(tree.usage), bridge: bridge ? await bridge.request('state') : null }; },
    ...(process.env.MULTI_CODEX_TEST_MODE === '1' ? {
      testBackendInfo() { return { pid: findCodexBackend(engine)?.pid, activationId, hostPid: process.pid }; },
      async testStopAccountHelper() {
        const child = accounts?.child;
        if (!child || !accounts?.isRunning) return;
        await new Promise<void>(resolve => { child.once('exit', () => resolve()); child.kill(); });
      },
      async testRequest(method: string, params: any = {}) { await connect(); if (!bridge) throw new Error('Bridge not connected'); return bridge.request('test/request', { method, params }); }
    } : {}),
  };
}

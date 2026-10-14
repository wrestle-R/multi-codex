import { spawn, execFileSync } from 'node:child_process';
import { mkdir, mkdtemp, writeFile, readFile, readdir, symlink, cp, stat, access, rm, rename } from 'node:fs/promises';
import { tmpdir, homedir } from 'node:os';
import { join, resolve } from 'node:path';
import { randomUUID } from 'node:crypto';
import { localService, syntheticAuth } from '../tests/fixtures/local-service.mjs';
import { createServer } from 'node:net';
import { chromium } from 'playwright-core';
import { inspectWebview } from './webview-inspector.mjs';
import { cleanupFixture } from './cleanup-fixture.mjs';
import { createRequire } from 'node:module';
const { bundledEnginePath, helperName, wrapperName } = createRequire(import.meta.url)('../dist/core.cjs');

async function discoverExtension() {
  if (process.env.MULTI_CODEX_TEST_EXTENSION) return process.env.MULTI_CODEX_TEST_EXTENSION;
  const roots = [join(homedir(), '.vscode', 'extensions')];
  const profiles = join(process.env.XDG_DATA_HOME || join(homedir(), '.local', 'share'), 'multi-codex', 'profiles');
  for (const profile of await readdir(profiles).catch(() => [])) roots.push(join(profiles, profile, 'vscode-extensions'));
  const candidates = [];
  for (const root of roots) for (const name of await readdir(root).catch(() => [])) if (name.startsWith('openai.chatgpt-')) candidates.push(join(root, name));
  candidates.sort((a, b) => b.split('/').at(-1).localeCompare(a.split('/').at(-1), 'en', { numeric: true }));
  if (!candidates.length) throw new Error('Install the official Codex extension or set MULTI_CODEX_TEST_EXTENSION.');
  return candidates[0];
}
const extension = await discoverExtension();
const packageInfo = JSON.parse(await readFile(join(extension, 'package.json'), 'utf8'));
const code = process.env.MULTI_CODEX_TEST_VSCODE || (process.platform === 'darwin'
  ? join('/Applications/Visual Studio Code.app/Contents/MacOS', execFileSync('/usr/libexec/PlistBuddy',
    ['-c', 'Print :CFBundleExecutable', '/Applications/Visual Studio Code.app/Contents/Info.plist'], { encoding: 'utf8' }).trim())
  : '/usr/share/code/code');
await access(code);
const developmentPath = process.env.MULTI_CODEX_TEST_DEVELOPMENT_PATH || resolve('.');
const developmentManifest = JSON.parse(await readFile(join(developmentPath, 'package.json'), 'utf8'));
const extensionId = `${developmentManifest.publisher}.${developmentManifest.name}`;
const liveAttach = process.env.MULTI_CODEX_TEST_LIVE_ATTACH === '1';
const portProbe = createServer(); await new Promise(resolve => portProbe.listen(0, '127.0.0.1', resolve));
const debugPort = portProbe.address().port; await new Promise(resolve => portProbe.close(resolve));
const testRoot = await mkdtemp(join(tmpdir(), 'multi-codex-vscode-'));
const fixture = await localService({ port: 8000 });
const storage = join(testRoot, 'data'); const globalHome = join(testRoot, 'global-home');
const ownStorage = join(testRoot, 'user-data', 'User', 'globalStorage', extensionId);
// The native Windows helper rejects junctions at its private runtime boundary.
// Exercise the actual VS Code storage directory rather than a fixture alias.
const runtimeRoot = process.platform === 'win32' ? join(ownStorage, 'runtime') : join(testRoot, 'runtime');
await mkdir(join(testRoot, 'workspace'), { recursive: true });
await mkdir(join(testRoot, 'user-data', 'User'), { recursive: true }); await mkdir(join(testRoot, 'extensions'));
// Only the disposable copy gets a fetch shim. Every official-extension HTTP call stays local.
const testExtension = join(testRoot, 'extensions', `openai.chatgpt-${packageInfo.version}`);
await mkdir(testExtension);
for (const entry of await readdir(extension)) if (entry !== 'out') { if (process.platform === 'win32') await cp(join(extension, entry), join(testExtension, entry), { recursive: true }); else await symlink(join(extension, entry), join(testExtension, entry)); }
await mkdir(join(testExtension, 'out'));
for (const entry of await readdir(join(extension, 'out'))) if (entry !== 'extension.js') { if (process.platform === 'win32') await cp(join(extension, 'out', entry), join(testExtension, 'out', entry), { recursive: true }); else await symlink(join(extension, 'out', entry), join(testExtension, 'out', entry)); }
const fetchShim = `const multiCodexOriginalFetch = globalThis.fetch; globalThis.fetch = (input, options) => { const url = new URL(typeof input === 'string' ? input : input.url ?? input.toString()); if (url.protocol === 'http:' || url.protocol === 'https:') return multiCodexOriginalFetch(${JSON.stringify(fixture.origin)} + url.pathname + url.search, options); return multiCodexOriginalFetch(input, options); };\n`;
const preparedUser = `\nconst multiCodexActivate = module.exports.activate; module.exports = { ...module.exports, activate: async (context, ...args) => { for (const key of ['viewed2025-09-15-nux', 'viewed2025-09-15-full-chatgpt-auth-nux', 'viewed2025-09-15-apikey-auth-nux']) await context.globalState.update(key, true); return multiCodexActivate(context, ...args); } };\n`;
await writeFile(join(testExtension, 'out', 'extension.js'), fetchShim + await readFile(join(extension, 'out', 'extension.js'), 'utf8') + preparedUser);
await mkdir(globalHome); await writeFile(join(globalHome, 'config.toml'), fixture.config);
const originalAuth = JSON.stringify(syntheticAuth('synthetic-account-a'));
if (liveAttach) await writeFile(join(globalHome, 'auth.json'), originalAuth, { mode: 0o600 });
const profiles = [];
for (const [index, label] of ['A', 'B'].entries()) {
  const id = randomUUID(); const home = join(storage, 'profiles', id, 'codex-home'); await mkdir(home, { recursive: true, mode: 0o700 });
  await writeFile(join(home, 'auth.json'), JSON.stringify(syntheticAuth(`synthetic-account-${label.toLowerCase()}`)), { mode: 0o600 });
  await writeFile(join(home, 'config.toml'), fixture.config, { mode: 0o600 });
  profiles.push({ id, name: `Account ${label}`, authMode: 'ChatGPT', createdAt: new Date().toISOString(), updatedAt: new Date().toISOString() });
}
await writeFile(join(storage, 'profiles.json'), JSON.stringify(profiles), { mode: 0o600 });
// Reproduce app-launched Code: desktop metadata supplies the global home while
// the extension host inherits a managed account's CODEX_HOME.
// Exercise a visible initial storage error and repair it from the test extension.
await writeFile(join(storage, 'executables.json'), 'incomplete desktop settings', { mode: 0o600 });
await mkdir(runtimeRoot, { recursive: true });
const bundledEngine = await bundledEnginePath(extension);
const configPath = join(runtimeRoot, process.platform === 'win32' ? 'codex-bridge.json' : 'bridge-config.json');
await writeFile(configPath, JSON.stringify({ engine: bundledEngine, helper: join(developmentPath, 'bin', helperName), dataRoot: storage, globalHome, runtimeRoot, nodeExecutable: code, bridgeScript: join(developmentPath, 'dist/bridge-main.cjs') }), { mode: 0o600 });
const wrapper = join(runtimeRoot, wrapperName);
const quote = value => `'${value.replace(/'/g, `'"'"'`)}'`;
if (process.platform === 'win32') await cp(join(developmentPath, 'bin', helperName), wrapper);
else await writeFile(wrapper, `#!/bin/sh\nexport ELECTRON_RUN_AS_NODE=1\nexec ${quote(code)} ${quote(join(developmentPath, 'dist/bridge-main.cjs'))} --bridge-config ${quote(configPath)} "$@"\n`, { mode: 0o700 });
await writeFile(join(testRoot, 'user-data', 'User', 'settings.json'), JSON.stringify({
  'multiCodex.dataDirectory': storage,
  ...(!liveAttach ? { 'chatgpt.cliExecutable': wrapper } : {}), 'chatgpt.openOnStartup': true,
  'chatgpt.apiEndpoint': 'localhost',
  'security.workspace.trust.enabled': false, 'telemetry.telemetryLevel': 'off', 'extensions.autoUpdate': false,
  'extensions.autoCheckUpdates': false, 'workbench.startupEditor': 'none',
}));
// The extension's storage root is fixed by VS Code; point discovery at the same fixture runtime.
await mkdir(ownStorage, { recursive: true });
if (process.platform !== 'win32') await symlink(runtimeRoot, join(ownStorage, 'runtime'), 'dir');
const env = Object.fromEntries(Object.entries(process.env).filter(([key]) => !key.startsWith('CODEX_') && !key.startsWith('OPENAI_')));
delete env.ELECTRON_RUN_AS_NODE; delete env.VSCODE_IPC_HOOK_CLI; delete env.VSCODE_PID;
const child = spawn(code, ['--user-data-dir', join(testRoot, 'user-data'), '--extensions-dir', join(testRoot, 'extensions'),
  '--extensionDevelopmentPath=' + developmentPath, '--extensionTestsPath=' + resolve('dist/vscode-tests.cjs'),
  '--skip-welcome', '--skip-release-notes', '--disable-gpu',
  ...(process.platform === 'linux' ? ['--no-sandbox', '--ozone-platform=x11'] : []), `--remote-debugging-port=${debugPort}`,
  '--disable-renderer-backgrounding', '--disable-background-timer-throttling', '--disable-backgrounding-occluded-windows',
  '--enable-proposed-api=openai.chatgpt', join(testRoot, 'workspace')],
  { env: { ...env, MULTI_CODEX_TEST_MODE: '1', MULTI_CODEX_TEST_EXTENSION_ID: extensionId, MULTI_CODEX_UI_TEST: '1', MULTI_CODEX_TEST_LIVE_ATTACH: liveAttach ? '1' : '0', MULTI_CODEX_VSCODE_TEST_ROOT: testRoot, CODEX_HOME: join(storage, 'profiles', profiles[0].id, 'codex-home') }, stdio: ['ignore', 'pipe', 'pipe'] });
let diagnostics = '';
for (const stream of [child.stdout, child.stderr]) stream.on('data', chunk => { diagnostics = (diagnostics + chunk.toString()).slice(-30000); });
const polling = setInterval(async () => {
  if (await access(join(testRoot, 'hold-response')).then(() => true, () => false)) { fixture.hold(); await writeFile(join(testRoot, 'hold-ready'), 'ready'); }
  if (await access(join(testRoot, 'release-response')).then(() => true, () => false)) fixture.release();
}, 20);
let browser; let uiBusy = false; const uiPhases = [];
await mkdir('.test-results', { recursive: true });
const uiPolling = setInterval(async () => {
  if (uiBusy) return; uiBusy = true;
  try {
    const phase = JSON.parse(await readFile(join(testRoot, 'ui-phase.json'), 'utf8').catch(() => '{}'));
    if (!phase.label || uiPhases.some(item => item.label === phase.label)) return;
    browser ??= await chromium.connectOverCDP(`http://127.0.0.1:${debugPort}`);
    const pages = browser.contexts().flatMap(context => context.pages()); const page = pages[0];
    const frames = [];
    for (const frame of pages.flatMap(page => page.frames())) {
      frames.push({ url: frame.url(), ...await frame.evaluate(() => ({ text: document.body?.innerText.slice(0, 6000), buttons: [...document.querySelectorAll('button')].map(button => ({ label: button.getAttribute('aria-label'), title: button.getAttribute('title'), text: button.innerText })) })).catch(() => ({})) });
    }
    frames.push(...await inspectWebview(browser));
    await writeFile('.test-results/ui-latest.json', JSON.stringify(frames, null, 2));
    if (!frames.some(frame => frame.buttons?.length && frame.text?.trim() && !frame.text.includes('hit a snag') && !frame.buttons.every(button => button.text === 'Try again'))) return;
    if (frames.some(frame => frame.buttons?.some(button => /^(Next|Get started|Done|Finish)$/.test(button.text)))) {
      await inspectWebview(browser, `(() => { const button = [...document.querySelectorAll('button')].find(button => button.getBoundingClientRect().width && !button.disabled && /^(Next|Get started|Done|Finish)$/.test(button.innerText)); button?.click(); return JSON.stringify({clicked: !!button}); })()`); return;
    }
    if (frames.some(frame => frame.buttons?.some(button => button.text === 'Continue with current model'))) {
      await inspectWebview(browser, `(() => { [...document.querySelectorAll('button')].find(button => button.innerText === 'Continue with current model')?.click(); return JSON.stringify({dismissed: true}); })()`); return;
    }
    const expectedLabel = phase.label.includes('Account A') ? 'Account A' : 'Account B';
    if (!frames.some(frame => !frame.targetId && frame.text?.includes(expectedLabel))) return;
    if (phase.accountsPanel) {
      const visible = await page.locator('[role="treeitem"]').allTextContents();
      if (!visible.some(text => text.includes('Account A')) || !visible.some(text => text.includes('Account B') && text.includes('90%'))) return;
      if (await page.getByText('Enable switching', { exact: true }).count() || await page.getByText('Add an account', { exact: true }).count()) throw new Error('Large welcome buttons are still present');
    }
    await page.screenshot({ path: `.test-results/panel-${phase.label.replace(/[^a-zA-Z0-9-]/g, '-')}.png` });
    const observation = { label: phase.label, frames };
    await writeFile('.test-results/panel-observations.json', JSON.stringify([...uiPhases, observation], null, 2));
    // Publish a complete acknowledgement: the extension host polls this file
    // concurrently, and reading a partially written JSON file is a test race.
    await writeFile(join(testRoot, 'ui-ack.tmp'), JSON.stringify({ label: phase.label }));
    await rename(join(testRoot, 'ui-ack.tmp'), join(testRoot, 'ui-ack.json'));
    // Mark complete only after publication succeeds. Windows can briefly hold
    // the old acknowledgement open; a failed replacement must remain retryable.
    uiPhases.push(observation);
  } catch (error) {
    await writeFile('.test-results/ui-inspector-last-error.json', JSON.stringify({ message: error.message, code: error.code })).catch(() => {});
    /* Renderer or an acknowledgement file may still be busy; retry. */
  }
  finally { uiBusy = false; }
}, 100);
console.log(`Testing Codex ${packageInfo.version} in ${testRoot}`);
const timeout = setTimeout(() => child.kill('SIGTERM'), 90000);
const result = await new Promise(resolve => child.once('exit', (code, signal) => resolve({ code, signal })));
clearTimeout(timeout); clearInterval(polling); clearInterval(uiPolling); await browser?.close().catch(() => {});
await mkdir('.test-results', { recursive: true });
await writeFile('.test-results/vscode-diagnostics.log', diagnostics);
try {
  const report = JSON.parse(await readFile(join(testRoot, 'result.json'), 'utf8'));
  report.codexVersion = packageInfo.version; report.fixtureRequests = fixture.requests.map(r => ({ path: r.path.split('?')[0], account: r.account })); report.panelPhases = uiPhases.map(phase => phase.label);
  report.generationAccounts = fixture.requests.filter(request => /responses(?:\?|$)/.test(request.path)).map(request => request.account);
  if (JSON.stringify(report.generationAccounts) !== JSON.stringify(['synthetic-account-a', 'synthetic-account-b'])) throw new Error('Continuing the chat used the wrong account');
  const targets = uiPhases.map(phase => phase.frames.find(frame => frame.targetId && frame.buttons?.length)?.targetId);
  if (new Set(targets).size !== 1) throw new Error('The Codex panel was recreated');
  report.panelStayedMounted = true;
  report.statusBarAccountVerified = true;
  report.accountsPanelVerified = uiPhases.some(phase => phase.label === 'accounts-list Account B');
  report.liveAttachment = liveAttach;
  report.platform = process.platform;
  report.architecture = process.arch;
  report.extensionId = extensionId;
  if (liveAttach && await readFile(join(globalHome, 'auth.json'), 'utf8') !== originalAuth) throw new Error('Live attachment changed the original login');
  report.originalLoginUnchanged = liveAttach ? true : null; // The startup case is covered by the persistence test.
  await writeFile('.test-results/vscode-result.json', JSON.stringify(report, null, 2));
  console.log(JSON.stringify({ ...report, fixtureRequests: report.fixtureRequests.length }, null, 2));
  if (!report.passed) process.exitCode = 1;
} catch {
  console.error('VS Code integration failed.', result); console.error(diagnostics.slice(-6000)); console.error('Fixture paths:', fixture.requests.map(r => r.path));
  process.exitCode = 1;
} finally {
  await fixture.close();
  if (process.exitCode !== 1) await cleanupFixture(testRoot, bundledEngine, join(developmentPath, 'bin', helperName));
  else console.error(`Failure artifacts retained in ${testRoot}`);
}

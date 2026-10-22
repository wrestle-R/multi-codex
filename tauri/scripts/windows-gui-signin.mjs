// Observe the real packaged WebView2 UI. This is not a browser-only/mock render.
// Only redacted screenshots and boolean evidence are public; never publish DOM,
// device instructions, private application logs, or a Playwright trace.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { constants, createHash, publicEncrypt } from 'node:crypto';
import { spawn, spawnSync } from 'node:child_process';
import { mkdir, readFile, stat, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';

if (process.platform !== 'win32') throw new Error('Requires native Windows');
const [binary, root, output, driver] = process.argv.slice(2).map(value => resolve(value));
const { chromium } = createRequire(join(driver, 'package.json'))('playwright');
const report = { source: process.env.GITHUB_SHA, platform: 'win32', architecture: process.arch, passed: false,
  candidateSource: process.env.MULTI_CODEX_GUI_CANDIDATE_SOURCE || process.env.GITHUB_SHA,
  checks: {}, authenticationCompletion: 'not tested; no user credentials used' };
let phase = 'prepare-disposable-account-root';
const progress = setInterval(() => writeFile(join(output, 'progress.json'), JSON.stringify({ source: report.source, phase })).catch(() => {}), 500);
let app; let browser; let busyConsumer; let busyConsumerTimer;
const data = join(root, 'roaming', 'multi-codex');
const globalHome = join(root, 'global');
const metadata = '[]\n';
const ownedAccountNames = new Set(['Native GUI cleanup fixture', 'Native GUI authentication acceptance']);
const deleteTemporaryAccount = async (page, id) => {
  const deadline = Date.now() + 30_000;
  while (true) {
    try {
      await page.evaluate(id => window.__TAURI_INTERNALS__.invoke('delete_profile', { id }), id);
      return;
    } catch (error) {
      // The GUI refreshes live usage immediately after saving an account. Respect
      // the native busy guard while that private backend exits; never bypass it.
      const busy = /Close this profile|running|busy|in use|lease/i.test(String(error));
      if (!busy || Date.now() >= deadline) {
        report.temporaryAccountDeletionFailure = busy ? 'native-busy-timeout' : 'native-delete-refused';
        throw new Error('Temporary native account removal failed');
      }
      report.nativeBusyDeletionRetryCount = (report.nativeBusyDeletionRetryCount || 0) + 1;
      await delay(500);
    }
  }
};
try {
  await mkdir(data, { recursive: true });
  await mkdir(globalHome, { recursive: true });
  await mkdir(output, { recursive: true });
  report.executableSha256 = createHash('sha256').update(await readFile(binary)).digest('hex');
  await writeFile(join(data, 'profiles.json'), metadata);
  await writeFile(join(data, 'executables.json'), JSON.stringify({
    codePath: process.env.MULTI_CODEX_TEST_VSCODE,
    codexPath: process.env.MULTI_CODEX_TEST_ENGINE,
    globalCodexHome: globalHome, onboardingCompleted: true,
  }));
  const env = Object.fromEntries(Object.entries(process.env).filter(([name]) =>
    !/^(CODEX_|OPENAI_|VSCODE_)/.test(name) && !['NODE_OPTIONS', 'ELECTRON_RUN_AS_NODE'].includes(name)));
  // CDP is loopback-only, temporary and test-only. No production flag changes.
  Object.assign(env, { APPDATA: join(root, 'roaming'), LOCALAPPDATA: join(root, 'local'),
    USERPROFILE: root, CODEX_HOME: globalHome,
    WEBVIEW2_USER_DATA_FOLDER: join(root, 'webview'),
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: '--remote-debugging-address=127.0.0.1 --remote-debugging-port=19384' });
  phase = 'launch-real-release-application';
  app = spawn(binary, [], { env, stdio: 'ignore', shell: false, windowsHide: false });
  app.on('error', () => {});
  // A stalled CDP call must still reach finally and native cleanup. Once fresh
  // encrypted instructions exist, the separate user-authentication wait applies.
  let watchedPhase = phase;
  let phaseStartedAt = Date.now();
  const watchdog = setInterval(() => {
    if (phase !== watchedPhase) { watchedPhase = phase; phaseStartedAt = Date.now(); }
    if (phase !== 'real-gui-await-user-authentication' && Date.now() - phaseStartedAt > 90_000 && app.exitCode === null) {
      report.driverWatchdogExpired = true;
      spawnSync('taskkill.exe', ['/PID', String(app.pid), '/T', '/F'], { windowsHide: true, stdio: 'ignore' });
    }
  }, 1000);
  app.once('exit', () => clearInterval(watchdog));
  const deadline = Date.now() + 60_000;
  while (!browser) {
    if (app.exitCode !== null || Date.now() >= deadline) throw new Error('Native application unavailable');
    try { browser = await chromium.connectOverCDP('http://127.0.0.1:19384', { timeout: 1500 }); }
    catch { await delay(250); }
  }
  const pages = browser.contexts().flatMap(context => context.pages());
  const page = pages[0];
  assert.ok(page);
  page.setDefaultTimeout(30_000);
  phase = 'verify-native-ipc-and-account-ui';
  await page.waitForFunction(() => Boolean(window.__TAURI_INTERNALS__));
  assert.equal(await page.evaluate(() => Boolean(window.__TAURI_INTERNALS__)), true);
  await page.getByRole('button', { name: 'Add account', exact: true }).waitFor();
  const screenshot = name => page.screenshot({ path: join(output, `${name}.png`),
    mask: [page.locator('.device-sign-in'), page.locator('.login-output')], animations: 'disabled' });
  await screenshot('native-account-ui');
  report.checks.realReleaseWebviewAndNativeIpc = true;
  for (let attempt = 1; attempt <= 2; attempt++) {
    phase = `real-gui-device-request-${attempt}`;
    await page.getByRole('button', { name: 'Add account', exact: true }).click();
    await page.getByPlaceholder('Personal', { exact: true }).fill(`Inert GUI cancellation ${attempt}`);
    await page.getByRole('button', { name: 'Get sign-in code', exact: true }).click();
    await page.locator('.device-sign-in code').waitFor({ timeout: 60_000 });
    assert.equal(await page.locator('.device-sign-in a').first().getAttribute('href'), 'https://auth.openai.com/codex/device');
    await screenshot(`device-request-${attempt}-redacted`);
    report.checks[`realGuiDeviceInstructions${attempt}`] = true;
    phase = `real-gui-browser-open-${attempt}`;
    await page.getByRole('button', { name: 'Open browser', exact: true }).dispatchEvent('click');
    await delay(3000);
    report.checks[`productionBrowserOpenInvoked${attempt}`] = true;
    phase = `real-gui-cancel-${attempt}`;
    await page.getByRole('button', { name: 'Cancel', exact: true }).click();
    await page.getByRole('dialog').waitFor({ state: 'hidden' });
    await delay(1500);
    assert.equal(await readFile(join(data, 'profiles.json'), 'utf8'), metadata);
    report.checks[`cancelPreservesEmptyProfileCatalog${attempt}`] = true;
  }
  await screenshot('after-cancel-and-retry');
  // Prove native credential persistence/removal without another user sign-in.
  // Padding exercises Windows' chunked credential format with an inert API key.
  phase = 'save-inert-native-gui-cleanup-account';
  await page.getByRole('button', { name: 'Add account', exact: true }).click();
  await page.getByRole('button', { name: 'Paste JSON', exact: true }).click();
  await page.getByPlaceholder('Personal', { exact: true }).fill('Native GUI cleanup fixture');
  await page.getByLabel('Auth JSON', { exact: true }).fill(JSON.stringify({
    auth_mode: 'apikey', OPENAI_API_KEY: 'inert-native-gui-cleanup-fixture', fixturePadding: 'x'.repeat(6000),
  }));
  await page.getByRole('dialog').getByRole('button', { name: 'Add account', exact: true }).click();
  await page.getByRole('dialog').waitFor({ state: 'hidden', timeout: 60_000 });
  const inertProfiles = JSON.parse(await readFile(join(data, 'profiles.json'), 'utf8'));
  assert.equal(inertProfiles.length, 1);
  const inertProfile = inertProfiles[0];
  assert.equal(inertProfile.name, 'Native GUI cleanup fixture');
  assert.match(inertProfile.id, /^[0-9a-f-]{36}$/i);
  report.checks.inertGuiLargeNativeCredentialSaved = true;
  const inertHome = join(data, 'profiles', inertProfile.id, 'codex-home');
  busyConsumer = spawn(process.env.MULTI_CODEX_TEST_ENGINE, ['-C', inertHome, 'app-server', '--stdio'], {
    env: { ...env, CODEX_HOME: inertHome, CODEX_SQLITE_HOME: inertHome },
    stdio: ['pipe', 'ignore', 'ignore'], windowsHide: true, shell: false,
  });
  await delay(1000);
  assert.equal(busyConsumer.exitCode, null, 'Owned native busy consumer exited before the guard test');
  busyConsumerTimer = setTimeout(() => {
    if (busyConsumer.exitCode === null && busyConsumer.signalCode === null) {
      spawnSync('taskkill.exe', ['/PID', String(busyConsumer.pid), '/T', '/F'], { windowsHide: true, stdio: 'ignore' });
    }
  }, 5000);
  phase = 'delete-inert-native-gui-cleanup-account';
  await deleteTemporaryAccount(page, inertProfile.id);
  assert.ok(report.nativeBusyDeletionRetryCount > 0, 'Native deletion must respect the live consumer before retry');
  assert.equal(JSON.parse(await readFile(join(data, 'profiles.json'), 'utf8')).length, 0);
  await assert.rejects(stat(join(data, 'profiles', inertProfile.id)), { code: 'ENOENT' });
  report.checks.inertGuiLargeNativeCredentialAndAccountRemoved = true;
  report.checks.nativeDeletionWaitedForOwnedBusyConsumer = true;
  if (process.env.GUI_AUTH_PUBLIC_KEY) {
    phase = 'real-gui-authenticated-device-request';
    await page.getByRole('button', { name: 'Add account', exact: true }).click();
    await page.getByPlaceholder('Personal', { exact: true }).fill('Native GUI authentication acceptance');
    await page.getByRole('button', { name: 'Get sign-in code', exact: true }).click();
    const code = page.locator('.device-sign-in code');
    await code.waitFor({ timeout: 60_000 });
    const verificationUrl = await page.locator('.device-sign-in a').first().getAttribute('href');
    assert.equal(verificationUrl, 'https://auth.openai.com/codex/device');
    const userCode = (await code.innerText()).trim();
    assert.match(userCode, /^[A-Z0-9-]{5,24}$/);
    await page.getByRole('button', { name: 'Open browser', exact: true }).dispatchEvent('click');
    const encrypted = publicEncrypt({ key: Buffer.from(process.env.GUI_AUTH_PUBLIC_KEY, 'base64'),
      padding: constants.RSA_PKCS1_OAEP_PADDING, oaepHash: 'sha256' },
    Buffer.from(JSON.stringify({ verificationUrl, userCode, createdAt: new Date().toISOString() })));
    await writeFile(join(output, 'signin.encrypted'), encrypted);
    phase = 'real-gui-await-user-authentication';
    await page.getByRole('dialog').waitFor({ state: 'hidden', timeout: 15 * 60_000 });
    const profiles = JSON.parse(await readFile(join(data, 'profiles.json'), 'utf8'));
    assert.equal(profiles.length, 1);
    const selected = profiles[0];
    assert.equal(selected.name, 'Native GUI authentication acceptance');
    assert.match(selected.id, /^[0-9a-f-]{36}$/i);
    await page.getByText(selected.name, { exact: true }).first().waitFor();
    const credential = JSON.parse(await readFile(join(data, 'profiles', selected.id, 'codex-home/auth.json'), 'utf8'));
    assert.equal(credential.auth_mode, 'chatgpt');
    assert.ok(credential.tokens?.access_token && credential.tokens?.refresh_token);
    report.checks.realGuiAuthenticationAndPrivateAccountSave = true;
    report.authenticationCompletion = 'completed through the real packaged GUI; private credentials never uploaded';
    phase = 'remove-only-created-real-runner-account';
    await deleteTemporaryAccount(page, selected.id);
    assert.equal(JSON.parse(await readFile(join(data, 'profiles.json'), 'utf8')).length, 0);
    report.checks.realGuiTemporaryAccountDeleted = true;
  }
  report.passed = true;
} catch {
  report.failedStage = phase;
  process.exitCode = 1;
} finally {
  clearTimeout(busyConsumerTimer);
  if (busyConsumer?.pid && busyConsumer.exitCode === null && busyConsumer.signalCode === null) {
    spawnSync('taskkill.exe', ['/PID', String(busyConsumer.pid), '/T', '/F'], { windowsHide: true, stdio: 'ignore' });
  }
  if (browser) {
    // Also remove a successfully saved account if a later assertion failed.
    // This catalog belongs only to the newly created disposable root.
    try {
      const profiles = JSON.parse(await readFile(join(data, 'profiles.json'), 'utf8'));
      const page = browser.contexts().flatMap(context => context.pages())[0];
      for (const profile of profiles) {
        assert.ok(ownedAccountNames.has(profile.name));
        await deleteTemporaryAccount(page, profile.id);
      }
    } catch {
      report.passed = false;
      report.accountCleanupFailed = true;
      process.exitCode = 1;
    }
  }
  if (browser) await Promise.race([browser.close().catch(() => {}), delay(2000)]);
  if (app?.pid && app.exitCode === null) {
    spawnSync('taskkill.exe', ['/PID', String(app.pid), '/T', '/F'], { windowsHide: true, stdio: 'ignore' });
  }
  await mkdir(output, { recursive: true });
  clearInterval(progress);
  await writeFile(join(output, 'gui-result.json'), JSON.stringify(report, null, 2));
  console.log(JSON.stringify(report, null, 2));
}

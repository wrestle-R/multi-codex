// Real Desktop GUI + passive proxy/deeplink proof with two inert local accounts.
// No stored account, keychain login, global home or browser profile is imported.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { mkdtemp, mkdir, readFile, writeFile, readdir, rm, symlink, readlink, realpath } from 'node:fs/promises';
import { randomUUID } from 'node:crypto';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { localService, syntheticAuth } from '../../extension/tests/fixtures/local-service.mjs';
const binary = process.env.MULTI_CODEX_TEST_STANDALONE || '/usr/lib/chatgpt/ChatGPT';
const engine = process.env.MULTI_CODEX_TEST_ENGINE || '/usr/lib/chatgpt/resources/codex';
const helper = process.env.MULTI_CODEX_TEST_SHARED_HELPER;
const navigation = process.env.MULTI_CODEX_TEST_HUD_NAVIGATION;
if (!helper || !helper.startsWith('/')) throw new Error('Supply an explicit freshly built native helper');
const root = await mkdtemp(join(tmpdir(), 'mc-hud-desktop-')); const service = await localService(); const windows = [];
const evidence = process.env.HUD_GUI_OUTPUT;
await writeFile(join(root, 'disposable-hud-fixture'), 'inert-fixture');
const dataRoot = join(root, 'data');
const profiles = [];
const aliasRoot = process.platform === 'darwin' ? await mkdtemp('/tmp/mc-hud-links-') : null;
let stage = 'initialization';
async function until(read, check) {
  const deadline = Date.now() + 30000;
  while (Date.now() < deadline) { const value = await read(); if (check(value)) return value; await new Promise(r => setTimeout(r, 100)); }
  throw new Error(`Disposable Desktop GUI did not reach the expected state: ${stage}`);
}
async function dismissFixtureAnnouncement(record) {
  // A first-launch product announcement can cover the selected transcript.
  // Exercise its normal UI in these disposable profiles; retain their model.
  return record.eval("(()=>{const button=[...document.querySelectorAll('button')].find(b=>b.textContent.trim()==='Continue with current model');if(button&&button.getBoundingClientRect().width){button.click();return true}return false})()");
}
const quote = value => `'${value.replaceAll("'", "'\"'\"'")}'`;
async function open(suffix, reopen) {
  const id = reopen?.id ?? randomUUID();
  const home = join(dataRoot, 'profiles', id); const codexHome = join(home, 'codex-home'); const desktop = join(home, 'desktop-data');
  if (!reopen) {
    profiles.push({ id, name: `Fixture ${suffix}`, authMode: 'ChatGPT', createdAt: new Date().toISOString(), updatedAt: new Date().toISOString() });
    await mkdir(dataRoot, { recursive: true });
    await writeFile(join(dataRoot, 'profiles.json'), JSON.stringify(profiles), { mode: 0o600 });
  }
  await mkdir(codexHome, { recursive: true }); await mkdir(desktop, { recursive: true });
  const auth = reopen ? await readFile(join(codexHome, 'auth.json'), 'utf8') : JSON.stringify(syntheticAuth(`synthetic-account-${suffix}`));
  if (!reopen) await writeFile(join(codexHome, 'auth.json'), auth, { mode: 0o600 });
  await writeFile(join(codexHome, 'config.toml'), service.config + '\ncli_auth_credentials_store = "file"\n');
  const wrapper = join(home, 'engine.sh'); await writeFile(wrapper, `#!/bin/sh\nexec ${quote(helper)} --hud-proxy ${quote(engine)} ${quote(dataRoot)} ${quote(id)} "$@"\n`, { mode: 0o700 });
  const desktopLaunch = aliasRoot ? join(aliasRoot, `${suffix}-desktop`) : desktop;
  const codexLaunch = aliasRoot ? join(aliasRoot, `${suffix}-codex`) : codexHome;
  if (aliasRoot && !reopen) { await symlink(desktop, desktopLaunch); await symlink(codexHome, codexLaunch); }
  const env = Object.fromEntries(Object.entries(process.env).filter(([key]) => !/^(CODEX_|OPENAI_|VSCODE_)/.test(key) && !['NODE_OPTIONS', 'ELECTRON_RUN_AS_NODE', 'APPIMAGE'].includes(key)));
  Object.assign(env, { HOME: home, CODEX_HOME: codexLaunch, CODEX_SQLITE_HOME: codexLaunch, CODEX_ELECTRON_USER_DATA_PATH: desktopLaunch, CODEX_CLI_PATH: wrapper, XDG_CONFIG_HOME: join(home, 'config'), XDG_DATA_HOME: join(home, 'data'), XDG_CACHE_HOME: join(home, 'cache') });
  const args = [`--user-data-dir=${desktopLaunch}`, '--remote-debugging-port=0', '--remote-debugging-address=127.0.0.1', '--disable-gpu', '--no-sandbox'];
  if (reopen?.uri) args.push(reopen.uri);
  await rm(join(desktop, 'DevToolsActivePort'), { force: true });
  const child = spawn(binary, args, { cwd: home, env, stdio: 'ignore', detached: true });
  const record = { child, id, env, args, codexHome, auth }; windows.push(record);
  const port = await until(async () => Number((await readFile(join(desktop, 'DevToolsActivePort'), 'utf8').catch(() => '')).split('\n')[0]), value => Number.isInteger(value) && value > 0 && value < 65536);
  const pages = await until(async () => fetch(`http://127.0.0.1:${port}/json`).then(response => response.json()).catch(() => []), pages => pages.some(page => page.type === 'page' && page.url.startsWith('app:')));
  const target = pages.find(page => page.type === 'page' && page.url.startsWith('app:'));
  const socket = new WebSocket(target.webSocketDebuggerUrl); await new Promise((resolve, reject) => { socket.onopen = resolve; socket.onerror = reject; });
  let next = 1; const replies = new Map(); socket.onmessage = event => { const message = JSON.parse(event.data); const pending = replies.get(message.id); if (pending) { replies.delete(message.id); pending(message); } };
  record.cdp = async (method, params) => {
    const id = next++;
    return new Promise((resolve, reject) => { const timeout = setTimeout(() => reject(new Error('Disposable GUI evaluation timeout')), 30000); replies.set(id, value => { clearTimeout(timeout); resolve(value); }); socket.send(JSON.stringify({ id, method, params })); });
  };
  record.eval = async expression => {
    const reply = await record.cdp('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true });
    if (reply.result?.exceptionDetails) throw new Error('Disposable GUI RPC evaluation failed');
    return reply.result?.result?.value;
  };
  record.rpc = (method, params = {}) => record.eval(`(async()=>{
    const id=${JSON.stringify('hud-test-' + randomUUID())};
    return await new Promise((resolve,reject)=>{
      const timer=setTimeout(()=>{window.removeEventListener('message',receive);reject(Error('RPC timeout'))},20000);
      function receive(e){if(e.data?.type==='mcp-response'&&e.data.message?.id===id){clearTimeout(timer);window.removeEventListener('message',receive);resolve(e.data.message)}}
      window.addEventListener('message',receive);
      window.electronBridge.sendMessageFromView({type:'mcp-request',hostId:'local',request:{id,method:${JSON.stringify(method)},params:${JSON.stringify(params)}},priority:'critical',timeoutMs:20000}).catch(reject);
    })
  })()`);
  record.socket = socket; return record;
}
try {
  stage = 'opening isolated Desktop instances';
  const a = await open('a'); const b = await open('b');
  for (const record of [a, b]) {
    const account = await record.rpc('account/read', { refreshToken: false }); assert.equal(account.error, undefined); assert.ok(account.result.account.email.includes('synthetic-account-'));
  }
  const started = await a.rpc('thread/start', { cwd: root, model: 'gpt-5.4' }); assert.equal(started.error, undefined); const id = started.result.thread.id;
  const snapshots = async () => Promise.all((await readdir(join(dataRoot, 'hud/instances')).catch(() => [])).filter(name => name.endsWith('.json')).map(async name => JSON.parse(await readFile(join(dataRoot, 'hud/instances', name), 'utf8').catch(() => 'null'))));
  await until(snapshots, snapshots => snapshots.some(s => s?.profileId === a.id && s.sessions[id]));
  const turn = await a.rpc('turn/start', { threadId: id, input: [{ type: 'text', text: 'HUD_EXACT_SESSION_A_FIXTURE' }] }); assert.equal(turn.error, undefined);
  await until(snapshots, snapshots => snapshots.some(s => s?.profileId === a.id && s.sessions[id]?.status === 'idle' && s.sessions[id]?.tokens));
  await a.rpc('thread/name/set', { threadId: id, name: 'Fixture A navigation' });
  const uri = `codex://threads/${id}`;
  await dismissFixtureAnnouncement(a);
  stage = 'exact-account warm navigation';
  const navigateWarm = async () => {
    const forward = navigation
      ? spawn(navigation, [root, a.id, id], { cwd: root, env: { ...a.env, MULTI_CODEX_TEST_HUD_ROUTING: '1' }, stdio: ['ignore', 'ignore', 'pipe'] })
      : spawn(binary, [`--user-data-dir=${a.env.CODEX_ELECTRON_USER_DATA_PATH}`, uri], { cwd: root, env: a.env, stdio: 'ignore' });
    let diagnostic = ''; forward.stderr?.on('data', chunk => { diagnostic += chunk });
    const [code] = await once(forward, 'exit');
    assert.equal(code, 0, `Production warm navigation failed: ${diagnostic}`);
  };
  await navigateWarm();
  // Desktop uses a memory router; its address bar stays index.html. Prove
  // the exact transcript is visible rather than mistaking URL stability for
  // a failed deeplink or matching a title in the sidebar.
  await until(async () => {
    // The announcement mounts asynchronously. If it covers navigation after
    // the initial URI, close it normally and deliver the scoped URI again.
    if (await dismissFixtureAnnouncement(a)) {
      await navigateWarm();
    }
    return a.eval("document.querySelector('main')?.innerText ?? document.body?.innerText ?? ''");
  }, value => value?.includes('HUD_EXACT_SESSION_A_FIXTURE'));
  assert.equal((await b.eval("document.querySelector('main')?.innerText ?? document.body?.innerText ?? ''")).includes('HUD_EXACT_SESSION_A_FIXTURE'), false);
  a.socket.close();
  const aExited = once(a.child, 'exit'); process.kill(-a.child.pid, 'SIGTERM'); await aExited;
  const reopened = await open('a', { id:a.id, uri });
  await dismissFixtureAnnouncement(reopened);
  stage = 'cold transcript reopen';
  await until(async () => {
    if (await dismissFixtureAnnouncement(reopened)) {
      const retry = spawn(binary, [`--user-data-dir=${reopened.env.CODEX_ELECTRON_USER_DATA_PATH}`, uri], { cwd: root, env: reopened.env, stdio: 'ignore' });
      await once(retry, 'exit');
    }
    return reopened.eval("document.querySelector('main')?.innerText ?? document.body?.innerText ?? ''");
  }, value => value?.includes('HUD_EXACT_SESSION_A_FIXTURE'));
  for (const record of [reopened, b]) assert.equal((await readFile(join(record.codexHome, 'auth.json'), 'utf8')) === record.auth, true, 'Disposable login bytes must remain unchanged');
  console.log('Verified actual Desktop GUI uses the passive proxy, exact thread URI reaches only the scoped account, cold reopening restores its transcript, and both inert logins stay unchanged.');
  if (evidence) { await mkdir(evidence, { recursive: true }); await writeFile(join(evidence, 'result.json'), JSON.stringify({ passed: true, source: process.env.GITHUB_SHA, stage, fixtureLoginsUnchanged: true, productionWarmRoute: Boolean(navigation), warmCredentialAccess: navigation ? 'forbidden by fixture store' : 'not instrumented' })); }
} catch (error) {
  if (evidence) {
    await mkdir(evidence, { recursive: true }); const states = [];
    for (const [index, record] of windows.entries()) {
      if (!record.eval || record.child.exitCode !== null) continue;
      states.push(await record.eval("({url:location.href,title:document.title,text:(document.body?.innerText??'').slice(0,4000)})").catch(() => ({ unavailable: true })));
      const capture = await record.cdp('Page.captureScreenshot', {}).catch(() => null);
      if (capture?.result?.data) await writeFile(join(evidence, `fixture-${index}.png`), Buffer.from(capture.result.data, 'base64'));
    }
    await writeFile(join(evidence, 'result.json'), JSON.stringify({ passed: false, source: process.env.GITHUB_SHA, stage, failure: error.message, states }, null, 2));
  }
  throw error;
} finally {
  for (const record of windows) { record.socket?.close(); try { process.kill(-record.child.pid, 'SIGTERM'); } catch {} }
  for (const record of windows) if (record.child.exitCode === null) await Promise.race([once(record.child, 'exit'), new Promise(resolve => setTimeout(resolve, 3000))]);
  for (const record of windows) { try { process.kill(-record.child.pid, 'SIGKILL'); } catch {} }
  await service.close();
  if (aliasRoot) {
    await rm(aliasRoot, { recursive: true, force: true });
    for (const profile of profiles) for (const [stem, target] of [['codex', 'codex-home'], ['desktop', 'desktop-data']]) {
      const alias = `/tmp/multi-codex-${stem}-${process.getuid()}/${profile.id}`;
      const expected = await realpath(join(dataRoot, 'profiles', profile.id, target));
      const actual = await readlink(alias).catch(() => null);
      if (actual === expected) await rm(alias);
    }
  }
  await rm(root, { recursive: true, force: true, maxRetries: 20, retryDelay: 100 });
}

// Real Desktop GUI + passive proxy/deeplink proof with two inert local accounts.
// No stored account, keychain login, global home or browser profile is imported.
import assert from 'node:assert/strict';
import { spawn, execFileSync } from 'node:child_process';
import { once } from 'node:events';
import { createRequire } from 'node:module';
import { mkdtemp, mkdir, readFile, writeFile, readdir, rm, symlink, readlink, realpath } from 'node:fs/promises';
import { randomUUID, createHash } from 'node:crypto';
import { createReadStream, openSync, closeSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { localService, syntheticAuth } from '../../extension/tests/fixtures/local-service.mjs';
const binary = process.env.MULTI_CODEX_TEST_STANDALONE || '/usr/lib/chatgpt/ChatGPT';
const engine = process.env.MULTI_CODEX_TEST_ENGINE || '/usr/lib/chatgpt/resources/codex';
const helper = process.env.MULTI_CODEX_TEST_SHARED_HELPER;
const navigation = process.env.MULTI_CODEX_TEST_HUD_NAVIGATION;
const runtimeArchive = process.platform === 'darwin' ? join(dirname(binary), '../Resources/app.asar') : join(dirname(binary), 'resources/app.asar');
async function digest(path) { const hash = createHash('sha256'); for await (const chunk of createReadStream(path)) hash.update(chunk); return hash.digest('hex'); }
const runtimeSha256 = await digest(runtimeArchive);
if (!helper || !helper.startsWith('/')) throw new Error('Supply an explicit freshly built native helper');
const helperSha256 = await digest(helper);
const { JsonLines, RpcPeer } = createRequire(import.meta.url)('../../extension/dist/core.cjs');
const responseMarker = 'HUD_EXACT_SESSION_A_RESPONSE_FIXTURE';
const root = await mkdtemp(join(tmpdir(), 'mc-hud-desktop-')); const service = await localService({ responseText: responseMarker }); const windows = [];
const evidence = process.env.HUD_GUI_OUTPUT;
await writeFile(join(root, 'disposable-hud-fixture'), 'inert-fixture');
const dataRoot = join(root, 'data');
const profiles = [];
const macAliases = process.platform === 'darwin';
let stage = 'initialization';
let activeThreadId = null;
let seededThreadId = null;
async function until(read, check) {
  const deadline = Date.now() + 90000;
  while (Date.now() < deadline) { const value = await read(); if (check(value)) return value; await new Promise(r => setTimeout(r, 100)); }
  throw new Error(`Disposable Desktop GUI did not reach the expected state: ${stage}`);
}
async function dismissFixtureAnnouncement(record) {
  // A first-launch product announcement can cover the selected transcript.
  // Exercise its normal UI in these disposable profiles; retain their model.
  return record.eval("(()=>{const button=[...document.querySelectorAll('button')].find(b=>b.textContent.trim()==='Continue with current model');if(button&&button.getBoundingClientRect().width){button.click();return true}return false})()");
}
const quote = value => `'${value.replaceAll("'", "'\"'\"'")}'`;
async function seedConversation(env) {
  const seedLog = openSync(join(root, 'seed-engine.log'), 'w', 0o600);
  const child = spawn(engine, ['app-server'], { cwd: root, env, stdio: ['pipe', 'pipe', seedLog] }); closeSync(seedLog);
  const lines = new JsonLines(child.stdout, child.stdin); const rpc = new RpcPeer(lines, 'hud-seed:');
  const completed = [];
  lines.on('message', message => { if (message.method === 'turn/completed') completed.push(message.params.threadId); });
  try {
    await rpc.request('initialize', { clientInfo: { name: 'hud_persisted_fixture', version: '1' }, capabilities: { experimentalApi: true } });
    const started = await rpc.request('thread/start', { cwd: root, model: 'gpt-5.4' });
    const id = started.thread.id;
    await rpc.request('turn/start', { threadId: id, input: [{ type: 'text', text: 'HUD_EXACT_SESSION_A_FIXTURE' }] });
    await until(async () => completed, completed => completed.includes(id));
    await rpc.request('thread/name/set', { threadId: id, name: 'Fixture A navigation' });
    const read = await rpc.request('thread/read', { threadId: id, includeTurns: true });
    assert.ok(JSON.stringify(read.thread.turns).includes('HUD_EXACT_SESSION_A_FIXTURE'));
    assert.ok(JSON.stringify(read.thread.turns).includes(responseMarker));
    return id;
  } finally {
    rpc.close(); child.stdin.end();
    if (child.exitCode === null) { const exited = once(child, 'exit'); child.kill('SIGTERM'); await exited; }
  }
}

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
  const trace = join(home, 'rpc-metadata.jsonl');
  const instrumented = join(home, 'native-engine.sh');
  await writeFile(instrumented, `#!/bin/sh\nexec ${quote(process.execPath)} ${quote(resolve('tauri/scripts/fixtures/hud-engine-trace.mjs'))} ${quote(engine)} ${quote(trace)} "$@"\n`, { mode: 0o700 });
  const wrapper = join(home, 'engine.sh'); await writeFile(wrapper, `#!/bin/sh\nexec ${quote(helper)} --hud-proxy ${quote(instrumented)} ${quote(dataRoot)} ${quote(id)} "$@"\n`, { mode: 0o700 });
  const desktopLaunch = macAliases ? `/tmp/multi-codex-desktop-${process.getuid()}/${id}` : desktop;
  const codexLaunch = macAliases ? `/tmp/multi-codex-codex-${process.getuid()}/${id}` : codexHome;
  if (macAliases && !reopen) {
    for (const alias of [desktopLaunch, codexLaunch]) await mkdir(dirname(alias), { recursive: true, mode: 0o700 });
    await symlink(await realpath(desktop), desktopLaunch); await symlink(await realpath(codexHome), codexLaunch);
  }
  const env = Object.fromEntries(Object.entries(process.env).filter(([key]) => !/^(CODEX_|OPENAI_|VSCODE_)/.test(key) && !['NODE_OPTIONS', 'ELECTRON_RUN_AS_NODE', 'APPIMAGE'].includes(key)));
  Object.assign(env, { HOME: home, CODEX_HOME: codexLaunch, CODEX_SQLITE_HOME: codexLaunch, CODEX_ELECTRON_USER_DATA_PATH: desktopLaunch, CODEX_CLI_PATH: wrapper, CODEX_SPARKLE_ENABLED: 'false', CODEX_ELECTRON_SKIP_COMPUTER_USE_CANONICAL_REFRESH: '1', MULTI_CODEX_TEST_ENGINE_TRACE: '1', XDG_CONFIG_HOME: join(home, 'config'), XDG_DATA_HOME: join(home, 'data'), XDG_CACHE_HOME: join(home, 'cache') });
  if (!reopen && suffix === 'a') seededThreadId = await seedConversation(env);
  const args = [`--user-data-dir=${desktopLaunch}`, '--remote-debugging-port=0', '--remote-debugging-address=127.0.0.1', '--disable-gpu', '--no-sandbox', '--password-store=basic'];
  if (reopen?.uri) args.push(reopen.uri);
  await rm(join(desktop, 'DevToolsActivePort'), { force: true });
  const startupLog = join(home, 'desktop-startup.log'); const log = openSync(startupLog, 'w', 0o600);
  const child = spawn(process.env.MULTI_CODEX_TEST_NO_VISIBILITY ? binary : helper, process.env.MULTI_CODEX_TEST_NO_VISIBILITY ? args : ['--hud-desktop', binary, dataRoot, id, ...args], { cwd: home, env, stdio: ['ignore', 'ignore', log], detached: true }); closeSync(log);
  const record = { child, id, env, args, codexHome, auth, trace, startupLog };
  record.targets = () => fetch(`http://127.0.0.1:${record.port}/json`).then(response => response.json()); windows.push(record);
  const port = await until(async () => Number((await readFile(join(desktop, 'DevToolsActivePort'), 'utf8').catch(() => '')).split('\n')[0]), value => Number.isInteger(value) && value > 0 && value < 65536);
  record.port = port;
  // Desktop also has a hidden avatar-overlay page; inspector target ordering
  // changes after reopen. Its empty body cannot establish main-window failure.
  const primary = page => page.type === 'page' && page.url === 'app://-/index.html';
  const pages = await until(async () => fetch(`http://127.0.0.1:${port}/json`).then(response => response.json()).catch(() => []), pages => pages.some(primary));
  const target = pages.find(primary);
  const socket = new WebSocket(target.webSocketDebuggerUrl); await new Promise((resolve, reject) => { socket.onopen = resolve; socket.onerror = reject; });
  let next = 1; const replies = new Map(); socket.onmessage = event => { const message = JSON.parse(event.data); const pending = replies.get(message.id); if (pending) { replies.delete(message.id); pending.resolve(message); } };
  socket.onclose = () => { for (const pending of replies.values()) pending.reject(new Error('Disposable Desktop inspector disconnected')); replies.clear(); };
  record.cdp = async (method, params) => {
    const id = next++;
    return new Promise((resolve, reject) => {
      if (socket.readyState !== WebSocket.OPEN) { reject(new Error('Disposable Desktop inspector disconnected')); return; }
      const timeout = setTimeout(() => { replies.delete(id); reject(new Error('Disposable GUI evaluation timeout')); }, 30000);
      replies.set(id, { resolve: value => { clearTimeout(timeout); resolve(value); }, reject: error => { clearTimeout(timeout); reject(error); } });
      socket.send(JSON.stringify({ id, method, params }));
    });
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
  const id = seededThreadId; assert.ok(id); activeThreadId = id;
  const snapshots = async () => Promise.all((await readdir(join(dataRoot, 'hud/instances')).catch(() => [])).filter(name => name.endsWith('.json')).map(async name => JSON.parse(await readFile(join(dataRoot, 'hud/instances', name), 'utf8').catch(() => 'null'))));
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
  }, value => value?.includes(responseMarker));
  assert.equal((await b.eval("document.querySelector('main')?.innerText ?? document.body?.innerText ?? ''")).includes(responseMarker), false);
  const visibleRoute = await a.eval(`(() => ({ selected: [...document.querySelectorAll('[data-app-action-sidebar-thread-selected="true"], [aria-current="page"]')].map(e => ({tag:e.tagName,attrs:[...e.attributes].filter(a=>/href|^data-|aria-current/.test(a.name)).map(a=>[a.name,a.value]),parents:[e.parentElement,e.parentElement?.parentElement].filter(Boolean).map(p=>({tag:p.tagName,href:p.getAttribute('href'),data:p.dataset}))})).slice(0,10), mainIds:[...document.querySelectorAll('main [data-thread-id],main [data-conversation-id]')].map(e=>({...e.dataset})).slice(0,10) }))()`);
  console.log('Disposable UI visibility attributes:', JSON.stringify(visibleRoute));
  const visibility = async () => {
    const states = await Promise.all((await readdir(join(dataRoot, 'hud/desktop')).catch(() => [])).filter(name => name.endsWith('.json')).map(async name => JSON.parse(await readFile(join(dataRoot, 'hud/desktop', name), 'utf8').catch(() => 'null'))));
    // A killed observer cannot remove its last file. Match production's live
    // process check rather than accepting the old PID during a cold reopen.
    return states.filter(state => { if (!state) return false; try { process.kill(state.pid, 0); return true; } catch { return false; } });
  };
  if (!process.env.MULTI_CODEX_TEST_NO_VISIBILITY) {
  stage = 'UI observer selects only the visible chat';
  await until(visibility, states => states.some(s => s?.profileId === a.id && s.threads.length === 1 && s.threads[0] === id));
  const other = await a.rpc('thread/start', { cwd: root, model: 'gpt-5.4' });
  await a.rpc('thread/read', { threadId: other.result.thread.id, includeTurns: false });
  await new Promise(r => setTimeout(r, 1800));
  const visibleA = (await visibility()).find(s => s?.profileId === a.id);
  assert.deepEqual(visibleA.threads, [id], 'loaded background chats must not count');
  const combinedDesktopCount = () => JSON.parse(execFileSync(process.env.MULTI_CODEX_TEST_ORBIT_DIAGNOSTIC, [dataRoot], { encoding: 'utf8' })).combined.filter(s => s.client === 'desktop').reduce((count, s) => count + s.sessionCount, 0);
  if (process.env.MULTI_CODEX_TEST_ORBIT_DIAGNOSTIC) assert.equal(combinedDesktopCount(), 1, 'production presence must follow the selected UI, not cached backend threads');
  const sidebarButton = await a.eval("[...document.querySelectorAll('button')].filter(b=>/sidebar/i.test(b.getAttribute('aria-label')??'')).map(b=>b.getAttribute('aria-label'))");
  console.log('Sidebar controls:', JSON.stringify(sidebarButton));
  await a.eval("document.querySelector('button[aria-label=\"Hide sidebar\"]')?.click()");
  await new Promise(r => setTimeout(r, 1800));
  const collapsed = (await visibility()).find(s => s?.profileId === a.id);
  console.log('Collapsed sidebar visibility count:', collapsed?.threads?.length);
  console.log('Collapsed public DOM attributes:', JSON.stringify(await a.eval(`Array.from(document.querySelectorAll('*')).flatMap(e=>Array.from(e.attributes).filter(a=>/thread|conversation|route/.test(a.name)||a.value.includes(${JSON.stringify(id)})).map(a=>({tag:e.tagName,name:a.name,value:a.value}))).slice(0,80)`)));

  assert.deepEqual(collapsed?.threads, [id], 'collapsing sidebar must not hide an open chat from Orbit');
  await a.eval("document.querySelector('button[aria-label=\"Show sidebar\"]')?.click()");
  stage = 'Home removes retained chats from visible sessions';
  // Reopening the sidebar mounts Home asynchronously on macOS. A missing
  // optional-chained click previously tested no navigation at all.
  await until(() => a.eval(`(() => {
    const home = document.querySelector('button[data-sidebar-destination="builtin:home"]');
    if (!home || home.disabled || !home.getBoundingClientRect().width) return false;
    home.click(); return true;
  })()`), Boolean);
  await until(visibility, states => states.some(s => s?.profileId === a.id && s.threads.length === 0));
  if (process.env.MULTI_CODEX_TEST_ORBIT_DIAGNOSTIC) assert.equal(combinedDesktopCount(), 0);
  await navigateWarm();
  await until(visibility, states => states.some(s => s?.profileId === a.id && s.threads.length === 1 && s.threads[0] === id));
  }

  const warmHistory = await a.rpc('thread/read', { threadId: id, includeTurns: true });
  assert.ok(JSON.stringify(warmHistory.result.thread.turns).includes('HUD_EXACT_SESSION_A_FIXTURE'));
  await until(snapshots, snapshots => snapshots.some(s => s?.profileId === a.id && s.sessions[id]));
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
  }, value => value?.includes(responseMarker));
  const coldHistory = await reopened.rpc('thread/read', { threadId: id, includeTurns: true });
  assert.deepEqual(coldHistory.result.thread.turns, warmHistory.result.thread.turns);
  for (const record of [reopened, b]) assert.equal((await readFile(join(record.codexHome, 'auth.json'), 'utf8')) === record.auth, true, 'Disposable login bytes must remain unchanged');
  assert.equal(await digest(runtimeArchive), runtimeSha256, 'Pinned Desktop runtime changed during acceptance');
  assert.equal(await digest(helper), helperSha256, 'The tested native helper changed during acceptance');
  if (!process.env.MULTI_CODEX_TEST_NO_VISIBILITY) {
    stage = 'observer disconnect preserves the open Desktop and transcript';
    const current = await until(visibility, states => states.some(s => s?.profileId === a.id && s.threads.includes(id)));
    const desktopPid = current.find(s => s?.profileId === a.id && s.threads.includes(id)).pid;
    const observerExited = once(reopened.child, 'exit');
    reopened.child.kill('SIGTERM'); await observerExited;
    await new Promise(resolve => setTimeout(resolve, 1600));
    process.kill(desktopPid, 0);
    assert.ok((await reopened.eval("document.querySelector('main')?.innerText ?? document.body?.innerText ?? ''")).includes(responseMarker));
    assert.equal(await readFile(join(reopened.codexHome, 'auth.json'), 'utf8'), reopened.auth);
  }
  console.log('Verified actual Desktop GUI uses the passive proxy, exact thread URI reaches only the scoped account, cold reopening restores its transcript, and both inert logins stay unchanged.');
  if (evidence) { await mkdir(evidence, { recursive: true }); await writeFile(join(evidence, 'result.json'), JSON.stringify({ passed: true, source: process.env.GITHUB_SHA, stage, runtimeSha256, helperSha256, pinnedRuntimeUnchanged: true, fixtureLoginsUnchanged: true, visibleChatsExcludeBackendCache: !process.env.MULTI_CODEX_TEST_NO_VISIBILITY, observerDisconnectPreservesDesktop: !process.env.MULTI_CODEX_TEST_NO_VISIBILITY, productionWarmRoute: Boolean(navigation), warmCredentialAccess: navigation ? 'forbidden by fixture store' : 'not instrumented' })); }
} catch (error) {
  if (evidence) {
    await mkdir(evidence, { recursive: true }); const states = [];
    const seedDiagnostic = await readFile(join(root, 'seed-engine.log'), 'utf8').catch(() => '');
    await writeFile(join(evidence, 'seed-engine.log'), seedDiagnostic, { mode: 0o600 });
    for (const [index, record] of windows.entries()) {
      if (!record.eval) {
        states.push({ startupExit: record.child.exitCode, pages: await record.targets().catch(() => []), startupLog: (await readFile(record.startupLog, 'utf8').catch(() => '')).slice(-5000) });
      }
      if (!record.eval || record.child.exitCode !== null) continue;
      const nativeRead = activeThreadId ? await record.rpc('thread/read', { threadId: activeThreadId, includeTurns: true }).catch(() => null) : null;
      const nativeThread = nativeRead?.result?.thread;
      states.push({ pages: await record.targets().then(pages => pages.map(p => ({id:p.id,type:p.type,title:p.title,url:p.url}))).catch(() => []), nativeReadError: nativeRead?.error?.code ?? null, nativeTurns: nativeThread?.turns?.length ?? null,
        fixtureInputInNativeHistory: JSON.stringify(nativeThread?.turns ?? []).includes('HUD_EXACT_SESSION_A_FIXTURE') });
      if (record.trace) {
        const metadata = await readFile(record.trace, 'utf8').catch(() => '');
        await writeFile(join(evidence, `rpc-metadata-${index}.jsonl`), metadata);
        console.log(`Native RPC metadata ${index}:`, metadata.split('\n').filter(Boolean).slice(-80).join('\n'));
      }
      states.push(await record.eval("({url:location.href,title:document.title,text:(document.body?.innerText??'').slice(0,4000),buttons:[...document.querySelectorAll('button')].filter(b=>b.getBoundingClientRect().width).map(b=>b.innerText)})").catch(() => ({ unavailable: true })));
      const capture = await record.cdp('Page.captureScreenshot', {}).catch(() => null);
      if (capture?.result?.data) await writeFile(join(evidence, `fixture-${index}.png`), Buffer.from(capture.result.data, 'base64'));
    }
    if (process.platform === 'darwin') { try { execFileSync('/usr/sbin/screencapture', ['-x', join(evidence, 'native-screen.png')]); } catch {} }
    const report = { passed: false, source: process.env.GITHUB_SHA, stage, runtimeSha256, failure: error.message, states };
    console.log(JSON.stringify(report, null, 2));
    await writeFile(join(evidence, 'result.json'), JSON.stringify(report, null, 2));
  }
  throw error;
} finally {
  for (const record of windows) { record.socket?.close(); try { process.kill(-record.child.pid, 'SIGTERM'); } catch {} }
  for (const record of windows) if (record.child.exitCode === null) await Promise.race([once(record.child, 'exit'), new Promise(resolve => setTimeout(resolve, 3000))]);
  for (const record of windows) { try { process.kill(-record.child.pid, 'SIGKILL'); } catch {} }
  await service.close();
  if (macAliases) {
    for (const profile of profiles) for (const [stem, target] of [['codex', 'codex-home'], ['desktop', 'desktop-data']]) {
      const alias = `/tmp/multi-codex-${stem}-${process.getuid()}/${profile.id}`;
      const expected = await realpath(join(dataRoot, 'profiles', profile.id, target));
      const actual = await readlink(alias).catch(() => null);
      if (actual === expected) await rm(alias);
    }
  }
  await rm(root, { recursive: true, force: true, maxRetries: 20, retryDelay: 100 });
}

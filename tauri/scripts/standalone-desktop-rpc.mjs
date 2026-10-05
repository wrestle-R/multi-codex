// Inspect only desktop instances created by standalone-isolation.py.
// Requires Node 22, or Node 20 with --experimental-websocket.
import fs from "node:fs"
import path from "node:path"
import crypto from "node:crypto"

const [directory, profile, operation = "account", refresh = "false"] = process.argv.slice(2)
if (!directory || !["a", "b"].includes(profile)) throw Error("Expected disposable root and profile a/b")
const root = fs.realpathSync(directory)
const state = JSON.parse(fs.readFileSync(path.join(root, "probe.json"), "utf8"))
if (state.marker !== "multi-codex-disposable-desktop-v1" || state.root !== root || !state.inspectable) {
  throw Error("Desktop inspection is restricted to an inspectable disposable probe")
}
const modes = { account: "account/read", limits: "account/rateLimits/read", logout: "account/logout" }
if (!Object.hasOwn(modes, operation) && !["status", "label", "signin"].includes(operation)) {
  throw Error("Unknown disposable test operation")
}
const data = path.join(root, profile, "desktop-data")
if (fs.lstatSync(data).isSymbolicLink()) throw Error("Refusing redirected desktop storage")
const port = Number(fs.readFileSync(path.join(data, "DevToolsActivePort"), "utf8").split("\n")[0])
if (!Number.isInteger(port) || port < 1 || port > 65535) throw Error("Invalid local inspection port")
const targets = await (await fetch(`http://127.0.0.1:${port}/json`)).json()
const target = targets.find(p => p.type === "page" && new URL(p.url).protocol === "app:" && new URL(p.url).pathname === "/index.html")
if (!target) throw Error("Disposable desktop main window is not available")
const endpoint = new URL(target.webSocketDebuggerUrl)
if (endpoint.protocol !== "ws:" || !["localhost", "127.0.0.1"].includes(endpoint.hostname)) throw Error("Inspection endpoint is not local")
const socket = new WebSocket(endpoint)
await new Promise((resolve, reject) => { socket.onopen = resolve; socket.onerror = reject })
let expression
if (operation === "status") {
  expression = `(()=>({signInVisible:[...document.querySelectorAll('button')].some(b=>/Continue to sign in/i.test(b.innerText)),profileMenuPresent:!!document.querySelector('[aria-label="Open profile menu"]'),bodyLength:document.body.innerText.length}))()`
} else if (operation === "label") {
  expression = `(()=>{document.title='Disposable test ${profile.toUpperCase()} — ChatGPT';let badge=document.getElementById('mc-test-label');if(!badge){badge=document.createElement('div');badge.id='mc-test-label';document.body.append(badge)}badge.textContent='DISPOSABLE TEST ${profile.toUpperCase()}';badge.style.cssText='position:fixed;bottom:12px;left:50%;transform:translateX(-50%);z-index:2147483647;padding:7px 12px;border:1px solid #f69c63;border-radius:8px;background:#151515;color:#fff;font:12px sans-serif;pointer-events:none';return {label:'${profile}'}})()`
} else if (operation === "signin") {
  expression = `(()=>{const b=[...document.querySelectorAll('button')].find(b=>/Continue to sign in/i.test(b.innerText));if(!b)return {clicked:false};b.click();return {clicked:true}})()`
} else {
  const id = "mc-isolation-" + crypto.randomUUID()
  const method = modes[operation]
  const params = operation === "account" ? { refreshToken: refresh === "true" } : {}
  expression = `(async()=>{
    const id=${JSON.stringify(id)};
    const reply=await new Promise((resolve,reject)=>{
      const timer=setTimeout(()=>{window.removeEventListener('message',receive);reject(Error('Desktop RPC timed out'))},25000);
      function receive(e){const d=e.data;if(d?.type==='mcp-response'&&d.message?.id===id){clearTimeout(timer);window.removeEventListener('message',receive);resolve(d.message)}}
      window.addEventListener('message',receive);
      window.electronBridge.sendMessageFromView({type:'mcp-request',hostId:'local',request:{id,method:${JSON.stringify(method)},params:${JSON.stringify(params)}},priority:'critical',timeoutMs:20000}).catch(reject);
    });
    if(reply.error)return {ok:false,errorCode:reply.error.code};
    const account=reply.result?.account;
    let identityHash=null;
    if(account?.email){const digest=await crypto.subtle.digest('SHA-256',new TextEncoder().encode(account.email.toLowerCase()));identityHash=[...new Uint8Array(digest)].map(x=>x.toString(16).padStart(2,'0')).join('')}
    return {ok:true,accountType:account?.type??null,identityHash};
  })()`
}
try {
  const result = await new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(Error("Desktop inspection timed out")), 35000)
    socket.onmessage = e => { const message = JSON.parse(e.data); if (message.id === 1) { clearTimeout(timer); resolve(message.result) } }
    socket.send(JSON.stringify({ id: 1, method: "Runtime.evaluate", params: { expression, awaitPromise: true, returnByValue: true } }))
  })
  if (result.exceptionDetails) throw Error("Disposable desktop evaluation failed")
  console.log(JSON.stringify(result.result?.value))
} finally { socket.close() }

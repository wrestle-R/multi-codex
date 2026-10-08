// Electron exposes VS Code's custom-scheme webview as an OOPIF. Playwright does not
// enumerate its inner document, so inspect its execution contexts through CDP.
export async function inspectWebview(browser, action) {
  const cdp = await browser.newBrowserCDPSession();
  const { targetInfos } = await cdp.send('Target.getTargets');
  const target = targetInfos.find(target => target.type === 'iframe' && target.url.includes('extensionId=openai.chatgpt'));
  if (!target) { await cdp.detach(); return []; }
  const { sessionId } = await cdp.send('Target.attachToTarget', { targetId: target.targetId, flatten: false });
  let nextId = 1; const pending = new Map(); const contexts = [];
  cdp.on('Target.receivedMessageFromTarget', event => {
    if (event.sessionId !== sessionId) return;
    const message = JSON.parse(event.message);
    if (message.method === 'Runtime.executionContextCreated') contexts.push(message.params.context);
    if (pending.has(message.id)) { pending.get(message.id)(message); pending.delete(message.id); }
  });
  async function request(method, params = {}) {
    const id = nextId++; const reply = new Promise(resolve => pending.set(id, resolve));
    await cdp.send('Target.sendMessageToTarget', { sessionId, message: JSON.stringify({ id, method, params }) });
    const result = await Promise.race([reply, new Promise(resolve => setTimeout(() => resolve({ error: { message: 'Inspector timeout' } }), 2000))]);
    if (result.error) throw new Error(result.error.message); return result.result;
  }
  try {
    await request('Runtime.enable');
    const documents = [];
    for (const context of contexts.filter(context => context.auxData?.isDefault)) {
      const result = await request('Runtime.evaluate', { contextId: context.id, returnByValue: true, expression: action ?? `JSON.stringify({ url: location.href, text: document.body?.innerText.slice(0,6000), buttons: [...document.querySelectorAll('button')].filter(button => button.getBoundingClientRect().width).map(button => ({ label: button.getAttribute('aria-label'), title: button.getAttribute('title'), text: button.innerText, disabled: button.disabled })) })` });
      if (typeof result.result?.value === 'string') documents.push({ targetId: target.targetId, ...JSON.parse(result.result.value) });
    }
    return documents;
  } finally { await cdp.send('Target.detachFromTarget', { sessionId }).catch(() => {}); await cdp.detach(); }
}

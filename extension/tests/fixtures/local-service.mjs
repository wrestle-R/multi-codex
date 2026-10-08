import { createServer } from 'node:http';

export function syntheticAuth(id) {
  const jwt = { exp: Math.floor(Date.now() / 1000) + 3600, sub: `user-${id}`, email: `${id}@example.invalid`,
    'https://api.openai.com/auth': { chatgpt_account_id: id, chatgpt_plan_type: 'plus', chatgpt_user_id: `user-${id}` },
    'https://api.openai.com/profile': { email: `${id}@example.invalid` } };
  const token = [Buffer.from('{"alg":"none"}').toString('base64url'), Buffer.from(JSON.stringify(jwt)).toString('base64url'), 'fixture'].join('.');
  return { auth_mode: 'chatgpt', tokens: { access_token: token, id_token: token, refresh_token: 'synthetic-refresh', account_id: id } };
}

export async function localService({ port = 0 } = {}) {
  const requests = []; let origin; let holdResponse = false; const held = new Set();
  const server = createServer(async (req, res) => {
    requests.push({ method: req.method, path: req.url, account: req.headers['chatgpt-account-id'] });
    for await (const chunk of req) { /* Drain request without retaining credentials or prompts. */ }
    res.setHeader('Content-Type', 'application/json');
    if (req.url.includes('statsig/bootstrap')) return res.end(JSON.stringify({ statsigPayload: JSON.stringify({ user: { userID: 'synthetic-user', customIDs: {} }, feature_gates: {}, dynamic_configs: {}, layer_configs: {}, has_updates: true, time: Date.now() }) }));
    if (req.url.includes('accounts/check') || req.url.includes('optimized/check')) {
      return res.end(JSON.stringify({ accounts: ['synthetic-account-a', 'synthetic-account-b'].map(id => ({
        id, plan_type: 'plus', structure: 'personal', workspace_backend_origin: 'https://chatgpt.com', account_routing_override: 'NO_CONSTRAINT',
      })), account_ordering: ['synthetic-account-a', 'synthetic-account-b'], default_account_id: 'synthetic-account-a' }));
    }
    if (/responses(?:\?|$)/.test(req.url)) {
      res.setHeader('Content-Type', 'text/event-stream'); res.setHeader('Cache-Control', 'no-cache');
      const emit = (type, data) => res.write(`event: ${type}\ndata: ${JSON.stringify({ type, ...data })}\n\n`);
      const response = { id: `resp-local-${requests.length}`, object: 'response', model: 'gpt-5.4', status: 'in_progress', output: [] };
      emit('response.created', { response });
      const complete = () => {
        if (res.destroyed) return;
        const item = { id: 'msg-local', type: 'message', role: 'assistant', status: 'completed', content: [{ type: 'output_text', text: 'local-fixture-ok', annotations: [] }] };
        emit('response.output_item.added', { output_index: 0, item: { ...item, status: 'in_progress', content: [] } });
        emit('response.output_text.delta', { item_id: item.id, output_index: 0, content_index: 0, delta: 'local-fixture-ok' });
        emit('response.output_item.done', { output_index: 0, item });
        emit('response.completed', { response: { ...response, status: 'completed', output: [item], usage: { input_tokens: 10, output_tokens: 5, total_tokens: 15, input_tokens_details: { cached_tokens: 0 }, output_tokens_details: { reasoning_tokens: 0 } } } });
        res.end(); held.delete(complete);
      };
      if (holdResponse) held.add(complete); else complete(); return;
    }
    if (req.url.includes('usage')) return res.end(JSON.stringify({ plan_type: 'plus', rate_limit: { allowed: true, limit_reached: false, primary_window: { used_percent: 10, limit_window_seconds: 18000, reset_at: 2000000000, reset_after_seconds: 1200 }, secondary_window: { used_percent: 20, limit_window_seconds: 604800, reset_at: 2000000000, reset_after_seconds: 604800 } } }));
    if (req.url.includes('rate-limit-reset-credits')) return res.end(JSON.stringify({ available_count: 0, credits: [] }));
    if (req.url.includes('tasks/list')) return res.end(JSON.stringify({ tasks: [], items: [], cursor: null, has_more: false }));
    if (req.url.includes('models')) return res.end(JSON.stringify({ models: [] }));
    if (req.url.includes('featured')) return res.end(JSON.stringify({ plugins: [] }));
    if (req.url.includes('plugins/suggested')) return res.end(JSON.stringify({ enabled: false, plugins: [] }));
    if (req.url.includes('plugins/installed')) return res.end(JSON.stringify({ plugins: [] }));
    res.end('{}');
  });
  await new Promise((resolve, reject) => { server.once('error', reject); server.listen(port, '127.0.0.1', resolve); }); origin = `http://127.0.0.1:${server.address().port}`;
  return { origin, requests, hold() { holdResponse = true; }, release() { holdResponse = false; for (const complete of [...held]) complete(); },
    config: `chatgpt_base_url = "${origin}"\nmodel_provider = "local_fixture"\nmodel = "gpt-5.4"\n[analytics]\nenabled = false\n[model_providers.local_fixture]\nname = "Local test fixture"\nbase_url = "${origin}/api/codex"\nwire_api = "responses"\nrequires_openai_auth = true\nsupports_websockets = false\n`,
    async close() { for (const complete of [...held]) complete(); server.closeAllConnections(); await new Promise(resolve => server.close(resolve)); } };
}

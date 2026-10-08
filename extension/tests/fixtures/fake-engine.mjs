#!/usr/bin/env node
import { createInterface } from 'node:readline';
import { syntheticAuth } from './local-service.mjs';
let token = null; let authMethod = null; let mode = ''; let held; let threadStatus = 'idle';
const send = message => process.stdout.write(JSON.stringify(message) + '\n');
for await (const line of createInterface({ input: process.stdin })) {
  const message = JSON.parse(line); const p = message.params ?? {};
  if (message.id === undefined) continue;
  let result = {};
  if (message.method === 'fixture/control') {
    if (p.mode !== undefined) mode = p.mode;
    if (p.threadStatus) threadStatus = p.threadStatus;
    if (p.notification) send(p.notification);
    if (p.release && held) {
      if (held.method === 'account/login/start') { token = held.params.accessToken; authMethod = 'chatgpt'; }
      send({ id: held.id, result: held.method === 'thread/loaded/list' ? { data: ['hidden-thread'] } : {} }); held = undefined;
    }
    result = { held: !!held };
  } else if (message.method === 'account/login/start') {
    if (p.type === 'apiKey' && authMethod === 'chatgpt') { send({ id: message.id, error: { code: -1, message: 'External auth is active' } }); continue; }
    if (mode === 'reject-all' || (mode === 'reject' && p.chatgptAccountId === 'synthetic-account-b')) { send({ id: message.id, error: { code: -1, message: 'Synthetic authentication failure' } }); continue; }
    if (mode === 'hold') { held = message; continue; }
    token = p.type === 'apiKey' ? p.apiKey : mode === 'wrong-identity' && p.chatgptAccountId === 'synthetic-account-b' ? syntheticAuth('synthetic-wrong').tokens.access_token : p.accessToken;
    authMethod = p.type === 'apiKey' ? 'apikey' : 'chatgpt';
    send({ method: 'account/updated', params: { authMode: 'chatgpt', planType: 'plus' } });
  } else if (message.method === 'account/logout') { token = null; authMethod = null; }
  else if (message.method === 'getAuthStatus') result = { authToken: token, authMethod };
  else if (message.method === 'thread/loaded/list') { if (mode === 'hold-scan') { held = message; continue; } result = { data: ['hidden-thread'] }; }
  else if (message.method === 'thread/read') result = { thread: { id: p.threadId, status: { type: threadStatus } } };
  else if (message.method === 'turn/start') { result = { turn: { id: 'turn-local', status: 'inProgress' } }; send({ method: 'turn/started', params: result }); }
  else if (message.method === 'fixture/crash') process.exit(0);
  send({ id: message.id, result });
}

#!/usr/bin/env node
import { createInterface } from 'node:readline';
import { syntheticAuth } from './local-service.mjs';
const leases = new Set();
for await (const line of createInterface({ input: process.stdin })) {
  const m = JSON.parse(line); const id = m.params?.id; let result = {};
  if (m.method === 'accounts/credential' || m.method === 'accounts/lease') {
    if (!['a', 'b', 'k'].includes(id)) { process.stdout.write(JSON.stringify({ id: m.id, error: { code: -1, message: 'Account not found' } }) + '\n'); continue; }
  }
  if (m.method === 'accounts/credential') result = { auth: JSON.stringify(id === 'k' ? { auth_mode: 'apikey', OPENAI_API_KEY: 'synthetic-api-key' } : syntheticAuth(`synthetic-account-${id}`)) };
  if (m.method === 'accounts/lease') leases.add(id);
  if (m.method === 'accounts/release') leases.delete(id);
  if (m.method === 'fixture/leases') result = [...leases];
  process.stdout.write(JSON.stringify({ id: m.id, result }) + '\n');
}

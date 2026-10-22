#!/usr/bin/env node
// Disposable native GUI fixture only. Preserve bytes; record RPC metadata,
// never message bodies, auth values, tool arguments or engine stderr.
import { spawn } from 'node:child_process';
import { appendFileSync } from 'node:fs';
import { StringDecoder } from 'node:string_decoder';
const [engine, trace, ...args] = process.argv.slice(2);
if (!engine || !trace || !process.env.MULTI_CODEX_TEST_ENGINE_TRACE) process.exit(2);
const child = spawn(engine, args, { stdio: ['pipe', 'pipe', 'inherit'] });
const requests = new Map();
const write = value => appendFileSync(trace, JSON.stringify(value) + '\n', { mode: 0o600 });
function observe(stream, direction) {
  const decoder = new StringDecoder('utf8'); let pending = '';
  stream.on('data', bytes => {
    pending += decoder.write(bytes);
    for (;;) {
      const end = pending.indexOf('\n'); if (end < 0) break;
      const line = pending.slice(0, end); pending = pending.slice(end + 1);
      try {
        const value = JSON.parse(line);
        if (direction === 'request' && value.method && value.id != null) requests.set(JSON.stringify(value.id), value.method);
        const method = value.method ?? requests.get(JSON.stringify(value.id));
        if (direction === 'response' && !value.method) requests.delete(JSON.stringify(value.id));
        write({ direction, method, reply: value.id != null && !value.method, errorCode: value.error?.code ?? null,
          turnCount: value.result?.thread?.turns?.length ?? null });
      } catch { /* An opaque frame is forwarded unchanged. */ }
    }
  });
}
observe(process.stdin, 'request'); observe(child.stdout, 'response');
process.stdin.pipe(child.stdin); child.stdout.pipe(process.stdout);
child.on('exit', (code, signal) => { if (signal) process.kill(process.pid, signal); else process.exit(code ?? 1); });
for (const signal of ['SIGTERM', 'SIGINT']) process.on(signal, () => child.kill(signal));

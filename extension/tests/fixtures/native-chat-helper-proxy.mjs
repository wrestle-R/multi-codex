// Test-only adapter: inert fixture credentials avoid touching the host keychain.
// All account leases and chat/storage requests use the actual packaged helper.
import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import { readFile } from 'node:fs/promises';
import { join, isAbsolute } from 'node:path';
const [engine, dataRoot, globalHome] = process.argv.slice(2);
const helper = process.env.MULTI_CODEX_TEST_SHARED_HELPER;
if (!helper || !isAbsolute(helper)) throw new Error('Native fixture helper must be explicit');
const child = spawn(helper, [engine, dataRoot, globalHome], { windowsHide: true, stdio: ['pipe', 'pipe', 'ignore'] });
child.stdout.pipe(process.stdout);
child.once('error', () => process.exit(1)); child.once('exit', code => process.exit(code ?? 1));
for await (const line of createInterface({ input: process.stdin })) {
  const request = JSON.parse(line);
  if (request.method === 'accounts/credential') {
    if (!/^[0-9a-f-]{36}$/.test(request.params.id)) throw new Error('Invalid fixture profile');
    const auth = await readFile(join(dataRoot, 'profiles', request.params.id, 'codex-home', 'auth.json'), 'utf8');
    process.stdout.write(JSON.stringify({ id: request.id, result: { auth } }) + '\n');
  } else child.stdin.write(line + '\n');
}
child.stdin.end();

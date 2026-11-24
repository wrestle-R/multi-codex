import { extensionVersion } from './version';
import { spawnExecutable, terminateExecutable } from './native';
import { JsonLines, RpcPeer } from './protocol';
import { privateBackendEnvironment } from './startup';

export async function withAccountBackend<T>(engine: string, home: string, action: (rpc: RpcPeer) => Promise<T>): Promise<T> {
  const child = spawnExecutable(engine, ['app-server'], { env: privateBackendEnvironment(home) });
  child.stderr.resume(); const lines = new JsonLines(child.stdout, child.stdin); const rpc = new RpcPeer(lines, 'worker:');
  child.on('error', () => rpc.close()); child.on('exit', () => rpc.close());
  try {
    await rpc.request('initialize', { clientInfo: { name: 'multi_codex_account', version: extensionVersion } }); lines.send({ method: 'initialized' });
    return await action(rpc);
  } finally { rpc.close(); await terminateExecutable(child); }
}

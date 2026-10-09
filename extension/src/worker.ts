import { extensionVersion } from './version';
import { spawn } from 'node:child_process';
import { spawnExecutable } from './native';
import { JsonLines, RpcPeer } from './protocol';

export async function withAccountBackend<T>(engine: string, home: string, action: (rpc: RpcPeer) => Promise<T>): Promise<T> {
  const env: NodeJS.ProcessEnv = { ...process.env, CODEX_HOME: home, CODEX_SQLITE_HOME: home };
  delete env.OPENAI_API_KEY; delete env.CODEX_API_KEY; delete env.ELECTRON_RUN_AS_NODE;
  const child = spawnExecutable(engine, ['app-server'], { env });
  child.stderr.resume(); const lines = new JsonLines(child.stdout, child.stdin); const rpc = new RpcPeer(lines, 'worker:');
  child.on('error', () => rpc.close()); child.on('exit', () => rpc.close());
  try {
    await rpc.request('initialize', { clientInfo: { name: 'multi_codex_account', version: extensionVersion } }); lines.send({ method: 'initialized' });
    return await action(rpc);
  } finally { rpc.close(); child.kill(); }
}

import { readFile, mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { randomBytes, randomUUID } from 'node:crypto';
import { CodexBridge, type BridgeConfig } from './bridge';
import { JsonLines } from './protocol';
import { startupHomes, prepareSessionHome } from './startup';
import { startupEnginePath } from './platform';

async function main() {
  const [flag, configPath, ...args] = process.argv.slice(2);
  if (flag !== '--bridge-config' || !configPath) throw new Error('Missing bridge configuration');
  const base = JSON.parse(await readFile(configPath, 'utf8'));
  if (!args.includes('app-server')) throw new Error('This bridge supports the Codex IDE backend only');
  const sessionId = randomUUID();
  const { sourceHome, historyHome } = startupHomes(base);
  const engine = await startupEnginePath(base.engine, base.officialExtensionPath);
  const config: BridgeConfig = { ...base,
    engine, sourceHome,
    sessionHome: join(base.runtimeRoot, 'windows', sessionId), workspace: process.cwd(),
    historyHome,
    socket: join(tmpdir(), `mc-${process.getuid?.() ?? 'user'}-${sessionId.slice(0, 12)}.sock`), token: randomBytes(32).toString('hex'),
  };
  await prepareSessionHome(sourceHome, config.sessionHome);
  const bridge = new CodexBridge(config, new JsonLines(process.stdin, process.stdout), args);
  await bridge.listen();
  await mkdir(join(base.runtimeRoot, 'connections'), { recursive: true, mode: 0o700 });
  await writeFile(join(base.runtimeRoot, 'connections', `${process.ppid}.json`), JSON.stringify({ socket: config.socket, token: config.token }), { mode: 0o600 });
  await writeFile(join(config.sessionHome, 'backend.json'), JSON.stringify({ pid: bridge.backend.pid, bridgePid: process.pid }), { mode: 0o600 });
  process.on('SIGTERM', () => void bridge.dispose().finally(() => process.exit(0)));
  process.on('SIGINT', () => void bridge.dispose().finally(() => process.exit(0)));
}
main().catch(error => { process.stderr.write('Multi Codex could not connect to the Codex backend. Check bridge setup.\n'); if (process.env.MULTI_CODEX_TEST_MODE === '1') process.stderr.write(String(error?.stack ?? error) + '\n'); process.exit(1); });

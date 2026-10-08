import { readFile, mkdir, writeFile, copyFile, symlink, access } from 'node:fs/promises';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { createHash, randomBytes, randomUUID } from 'node:crypto';
import { CodexBridge, type BridgeConfig } from './bridge';
import { JsonLines } from './protocol';

async function main() {
  const [flag, configPath, ...args] = process.argv.slice(2);
  if (flag !== '--bridge-config' || !configPath) throw new Error('Missing bridge configuration');
  const base = JSON.parse(await readFile(configPath, 'utf8'));
  if (!args.includes('app-server')) throw new Error('This bridge supports the Codex IDE backend only');
  const sessionId = randomUUID();
  const workspaceKey = createHash('sha256').update(process.cwd()).digest('hex');
  await mkdir(join(base.runtimeRoot, 'selections'), { recursive: true, mode: 0o700 });
  const selectionPath = join(base.runtimeRoot, 'selections', workspaceKey + '.json');
  const savedSelection = await readFile(selectionPath, 'utf8').then(text => JSON.parse(text)).catch(() => ({}));
  const config: BridgeConfig = { ...base,
    sessionHome: join(base.runtimeRoot, 'windows', sessionId), workspace: process.cwd(),
    historyHome: base.historyHome ?? base.globalHome, selectionPath, selectedId: savedSelection.id,
    socket: join(tmpdir(), `mc-${process.getuid?.() ?? 'user'}-${sessionId.slice(0, 12)}.sock`), token: randomBytes(32).toString('hex'),
  };
  await mkdir(config.sessionHome, { recursive: true, mode: 0o700 });
  // Authentication is private to this backend; conversations and user resources
  // retain the original Codex home so attaching the bridge does not hide history.
  await mkdir(config.historyHome!, { recursive: true, mode: 0o700 });
  for (const name of ['sessions', 'archived_sessions', 'skills', 'rules', 'plugins', 'memories', 'prompts']) {
    await mkdir(join(base.globalHome, name), { recursive: true, mode: 0o700 });
    await symlink(join(base.globalHome, name), join(config.sessionHome, name));
  }
  if (await access(join(base.globalHome, 'AGENTS.md')).then(() => true, () => false)) await symlink(join(base.globalHome, 'AGENTS.md'), join(config.sessionHome, 'AGENTS.md'));
  // Preserve the user's configuration while giving this backend its own credential store.
  try { await copyFile(join(config.globalHome, 'config.toml'), join(config.sessionHome, 'config.toml')); } catch {}
  const originalConfig = await readFile(join(config.sessionHome, 'config.toml'), 'utf8').catch(() => '');
  await writeFile(join(config.sessionHome, 'config.toml'), 'cli_auth_credentials_store = "file"\n' + originalConfig.replace(/^cli_auth_credentials_store\s*=.*$/gm, ''), { mode: 0o600 });
  const bridge = new CodexBridge(config, new JsonLines(process.stdin, process.stdout), args);
  await bridge.listen();
  await mkdir(join(base.runtimeRoot, 'connections'), { recursive: true, mode: 0o700 });
  await writeFile(join(base.runtimeRoot, 'connections', `${process.ppid}.json`), JSON.stringify({ socket: config.socket, token: config.token }), { mode: 0o600 });
  await writeFile(join(config.sessionHome, 'backend.json'), JSON.stringify({ pid: bridge.backend.pid, bridgePid: process.pid }), { mode: 0o600 });
  process.on('SIGTERM', () => void bridge.dispose().finally(() => process.exit(0)));
  process.on('SIGINT', () => void bridge.dispose().finally(() => process.exit(0)));
}
main().catch(error => { process.stderr.write('Multi Codex could not connect to the Codex backend. Check bridge setup.\n'); if (process.env.MULTI_CODEX_TEST_MODE === '1') process.stderr.write(String(error?.stack ?? error) + '\n'); process.exit(1); });

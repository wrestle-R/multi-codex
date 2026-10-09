import { spawn, type ChildProcess, type SpawnOptionsWithoutStdio } from 'node:child_process';
import { join } from 'node:path';
import { tmpdir } from 'node:os';

export const helperName = process.platform === 'win32' ? 'multi-codex-account-helper.exe' : 'multi-codex-account-helper';
export const wrapperName = process.platform === 'win32' ? 'codex-bridge.exe' : 'codex-bridge';
export function controlEndpoint(id: string, platform = process.platform) {
  return platform === 'win32' ? `\\\\.\\pipe\\multi-codex-${id}` : join(tmpdir(), `mc-${process.getuid?.() ?? 'user'}-${id.slice(0, 12)}.sock`);
}
/** Native production binaries and JavaScript protocol fixtures use identical streams. */
export function spawnExecutable(binary: string, args: string[], options: SpawnOptionsWithoutStdio = {}) {
  return binary.endsWith('.mjs')
    ? spawn(process.execPath, [binary, ...args], { ...options, env: { ...(options.env ?? process.env), ELECTRON_RUN_AS_NODE: '1' }, stdio: ['pipe', 'pipe', 'pipe'] })
    : spawn(binary, args, { ...options, windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'] });
}

/** Wait for handles to close before removing homes or replacing a Windows binary. */
export async function terminateExecutable(child: ChildProcess) {
  if (child.exitCode !== null || child.signalCode !== null) return;
  await new Promise<void>(resolve => {
    const done = () => { clearTimeout(timeout); resolve(); };
    child.once('close', done);
    const timeout = setTimeout(() => child.kill('SIGKILL'), 3000);
    child.kill();
  });
}

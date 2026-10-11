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

/** Reap the process and close our pipes before removing its private home. */
export async function terminateExecutable(child: ChildProcess) {
  if (child.pid && child.exitCode === null && child.signalCode === null) {
    await new Promise<void>(resolve => {
      const done = () => { clearTimeout(timeout); child.off('exit', done); child.off('close', done); resolve(); };
      child.once('exit', done); child.once('close', done);
      const timeout = setTimeout(() => child.kill('SIGKILL'), 3000);
      child.kill();
    });
  }
  // Descendants can inherit a pipe after the parent exits. They must not hold
  // extension disposal open indefinitely; these streams belong to this client.
  child.stdin?.destroy(); child.stdout?.destroy(); child.stderr?.destroy();
}

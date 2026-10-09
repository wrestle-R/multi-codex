import { execFileSync } from 'node:child_process';
import { mkdirSync, copyFileSync, chmodSync, renameSync } from 'node:fs';
import { resolve } from 'node:path';
if (!['linux', 'darwin', 'win32'].includes(process.platform)) throw new Error('The native account helper currently supports Linux and macOS.');
execFileSync('cargo', ['build', '--locked', '--release', '--bin', 'multi-codex-account-helper'], { cwd: resolve('../tauri/src-tauri'), stdio: 'inherit' });
mkdirSync('bin', { recursive: true });
const name = process.platform === 'win32' ? 'multi-codex-account-helper.exe' : 'multi-codex-account-helper';
const temporary = resolve(`bin/${name}.${process.pid}`);
copyFileSync(resolve('../tauri/src-tauri/target/release', name), temporary);
if (process.platform !== 'win32') { execFileSync('strip', [temporary]); chmodSync(temporary, 0o700); }
renameSync(temporary, resolve('bin', name));
// Stripping changes a Mach-O executable. Restore its ad-hoc signature so the
// packaged Apple Silicon helper is accepted by macOS executable verification.
if (process.platform === 'darwin') {
  execFileSync('codesign', ['--force', '--sign', '-', resolve('bin/multi-codex-account-helper')], { stdio: 'inherit' });
  execFileSync('codesign', ['--verify', '--strict', resolve('bin/multi-codex-account-helper')], { stdio: 'inherit' });
}

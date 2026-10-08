import { execFileSync } from 'node:child_process';
import { mkdirSync, copyFileSync, chmodSync, renameSync } from 'node:fs';
import { resolve } from 'node:path';
if (!['linux', 'darwin'].includes(process.platform)) throw new Error('The native account helper currently supports Linux and macOS.');
execFileSync('cargo', ['build', '--locked', '--release', '--bin', 'multi-codex-account-helper'], { cwd: resolve('../tauri/src-tauri'), stdio: 'inherit' });
mkdirSync('bin', { recursive: true });
const temporary = resolve(`bin/multi-codex-account-helper.${process.pid}`);
copyFileSync(resolve('../tauri/src-tauri/target/release/multi-codex-account-helper'), temporary);
execFileSync('strip', [temporary]); chmodSync(temporary, 0o700); renameSync(temporary, resolve('bin/multi-codex-account-helper'));

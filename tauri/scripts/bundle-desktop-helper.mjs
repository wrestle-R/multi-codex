import { spawnSync } from 'node:child_process';
import { copyFile, chmod, mkdir, readFile, writeFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const app = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const native = join(app, 'src-tauri');
const resource = join(app, 'resources', 'managed-extension');
const target = process.env.TAURI_ENV_TARGET_TRIPLE || process.env.CARGO_BUILD_TARGET;
if (target && !/^[a-z0-9_-]+$/.test(target)) throw new Error('Invalid native helper target');
const debug = process.env.TAURI_ENV_DEBUG === 'true';
const profile = debug ? 'debug' : 'release';
const name = process.platform === 'win32' ? 'multi-codex-account-helper.exe' : 'multi-codex-account-helper';
function run(command, args) {
  const result = spawnSync(command, args, { cwd: native, stdio: 'inherit', windowsHide: true });
  if (result.status !== 0) throw new Error('Could not prepare matching native account helper');
}
await readFile(join(resource, 'package.json'));
run('cargo', ['build', '--locked', ...(debug ? [] : ['--release']), '--bin', 'multi-codex-account-helper', ...(target ? ['--target', target] : [])]);
const buildRoot = process.env.CARGO_TARGET_DIR ? resolve(native, process.env.CARGO_TARGET_DIR) : join(native, 'target');
const executable = join(buildRoot, ...(target ? [target] : []), profile, name);
await mkdir(join(resource, 'bin'), { recursive: true });
const destination = join(resource, 'bin', name);
await copyFile(executable, destination);
if (process.platform !== 'win32') {
  await chmod(destination, 0o700);
  if (!debug) run('strip', [destination]);
}
if (process.platform === 'darwin') {
  run('codesign', ['--force', '--sign', '-', destination]);
  run('codesign', ['--verify', '--strict', destination]);
}
const os = { linux: 'linux', darwin: 'macos', win32: 'windows' }[process.platform];
const architecture = target?.startsWith('aarch64-') ? 'aarch64' : target?.startsWith('x86_64-') ? 'x86_64'
  : { x64: 'x86_64', arm64: 'aarch64' }[process.arch];
if (!os || !architecture) throw new Error('Unsupported helper platform');
const version = JSON.parse(await readFile(join(native, 'tauri.conf.json'), 'utf8')).version;
const digest = createHash('sha256').update(await readFile(destination)).digest('hex');
await writeFile(join(resource, 'desktop-helper.json'), JSON.stringify({ schema: 1, os, architecture, target: target || null, version, digest }) + '\n');
console.log('Bundled matching account helper and fingerprint without desktop GUI dependencies.');

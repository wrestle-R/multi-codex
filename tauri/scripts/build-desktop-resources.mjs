import { spawnSync } from 'node:child_process';
import { cp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const app = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const extension = resolve(app, '..', 'extension');
const destination = join(app, 'resources', 'managed-extension');
const npm = process.platform === 'win32' ? 'npm.cmd' : 'npm';
function run(args) {
  const result = spawnSync(npm, args, { cwd: extension, stdio: 'inherit', shell: process.platform === 'win32', windowsHide: true });
  if (result.status !== 0) throw new Error('Could not build the managed desktop bridge resource');
}
if (!existsSync(join(extension, 'node_modules', 'esbuild'))) run(['ci']);
run(['run', 'build']);
await mkdir(dirname(destination), { recursive: true });
await rm(destination, { recursive: true, force: true });
await mkdir(destination);
const manifest = JSON.parse(await readFile(join(extension, 'package.json'), 'utf8'));
delete manifest.scripts; delete manifest.devDependencies;
await writeFile(join(destination, 'package.json'), JSON.stringify(manifest, null, 2) + '\n');
for (const entry of ['dist', 'resources', 'LICENSE', 'README.md']) {
  await cp(join(extension, entry), join(destination, entry), { recursive: true });
}
// Populate the complete resource before Tauri expands its resource paths.
// beforeBundle verifies/refreshes the same target artifact again at packaging.
await import('./bundle-desktop-helper.mjs');
console.log('Built managed bridge and matching headless helper without private runtime data.');

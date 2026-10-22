#!/usr/bin/env node
import { createRequire } from 'node:module';
import { dirname, join } from 'node:path';
import { readFileSync, lstatSync, realpathSync, readdirSync } from 'node:fs';
import { createHash, verify } from 'node:crypto';
import { spawn } from 'node:child_process';
import { release } from 'node:os';
import { publicKey } from './public-key.mjs';

const require = createRequire(import.meta.url);
function fail(message) { console.error(`Multi Codex: ${message}`); process.exit(3); }
const platform = `${process.platform}-${process.arch}`;
if (!['linux-x64', 'linux-arm64', 'darwin-arm64'].includes(platform)) fail('Supported targets: Linux x64/ARM64 and Apple Silicon macOS 26+.');
if (process.platform === 'darwin' && Number(release().split('.')[0]) < 25) fail('macOS 26 or newer is required.');
if (process.platform === 'linux') {
  const glibc = process.report?.getReport().header.glibcVersionRuntime;
  const [major, minor] = (glibc || '').split('.').map(Number);
  if (!glibc || major < 2 || (major === 2 && minor < 39)) fail('This Linux build requires glibc 2.39+. musl-based distributions are not supported by this package.');
}
let root;
try { root = dirname(require.resolve(`multi-codex-${platform}/package.json`)); }
catch { fail(`The ${platform} package is unavailable or was omitted. Install with optional dependencies enabled. Check the CLI guide for published targets.`); }
try {
  const metadata = JSON.parse(readFileSync(join(root, 'package.json'), 'utf8'));
  const wrapper = JSON.parse(readFileSync(new URL('../package.json', import.meta.url), 'utf8'));
  if (metadata.version !== wrapper.version) throw new Error('Package versions differ; reinstall multi-codex.');
  const payload = readFileSync(join(root, 'payload.json'));
  const signature = Buffer.from(readFileSync(join(root, 'payload.sig'), 'utf8').trim(), 'base64');
  if (!verify(null, payload, publicKey, signature)) throw new Error('Platform payload signature is invalid.');
  const manifest = JSON.parse(payload);
  if (manifest.schema !== 1 || manifest.platform !== platform || manifest.version !== metadata.version) throw new Error('Platform manifest does not match this installation.');
  const canonicalRoot = realpathSync(root);
  const actualFiles = [];
  function walk(relative) {
    const path = join(root, relative);
    const stat = lstatSync(path);
    if (stat.isSymbolicLink()) throw new Error('Symlinked package payload is refused.');
    if (stat.isDirectory()) { for (const entry of readdirSync(path)) walk(`${relative}/${entry}`); }
    else if (stat.isFile()) actualFiles.push(relative);
    else throw new Error('Unsupported payload file type.');
  }
  walk('bin'); walk('resources');
  if (actualFiles.length !== Object.keys(manifest.files).length) throw new Error('Platform payload inventory changed.');
  for (const file of actualFiles) {
    if (!Object.hasOwn(manifest.files, file) || file.includes('..') || file.startsWith('/')) throw new Error('Unexpected platform payload file.');
    const resolved = realpathSync(join(root, file));
    if (!resolved.startsWith(`${canonicalRoot}/`)) throw new Error('Payload path escaped its package.');
    if (createHash('sha256').update(readFileSync(resolved)).digest('hex') !== manifest.files[file]) throw new Error(`Platform payload integrity check failed: ${file}`);
  }
} catch (error) { fail(error.message); }
const child = spawn(join(root, 'bin/multi-codex'), process.argv.slice(2), { stdio: 'inherit', env: process.env });
child.on('error', () => fail('Native executable could not start. Check the documented Linux runtime requirements or reinstall.'));
for (const signal of ['SIGINT', 'SIGTERM', 'SIGHUP']) process.on(signal, () => { if (!child.killed) child.kill(signal); });
child.on('exit', (code, signal) => {
  if (signal) { process.removeAllListeners(signal); process.kill(process.pid, signal); }
  else process.exit(code ?? 1);
});

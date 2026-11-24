import { access, readFile } from 'node:fs/promises';

const root = new URL('../../', import.meta.url);
const read = path => readFile(new URL(path, root), 'utf8');
const json = async path => JSON.parse(await read(path));

const app = await json('tauri/package.json');
const tag = process.argv[2] || `v${app.version}`;
if (!/^v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$/.test(tag)) {
  throw new Error(`Invalid stable release tag: ${tag}`);
}

const appLock = await json('tauri/package-lock.json');
const siteLock = await json('next/package-lock.json');
const cargo = await read('tauri/src-tauri/Cargo.toml');
const cargoLock = await read('tauri/src-tauri/Cargo.lock');
const versions = {
  'tauri/package.json': app.version,
  'tauri/package-lock.json': appLock.version,
  'tauri/package-lock.json (root package)': appLock.packages?.['']?.version,
  'tauri/src-tauri/tauri.conf.json': (await json('tauri/src-tauri/tauri.conf.json')).version,
  'tauri/src-tauri/Cargo.toml': cargo.match(/^version = "([^"]+)"/m)?.[1],
  'tauri/src-tauri/Cargo.lock': cargoLock.match(/^name = "multi-codex-desktop"\nversion = "([^"]+)"/m)?.[1],
  'next/package.json': (await json('next/package.json')).version,
  'next/package-lock.json': siteLock.version,
  'next/package-lock.json (root package)': siteLock.packages?.['']?.version,
};

const mismatches = Object.entries(versions)
  .filter(([, version]) => version !== tag.slice(1))
  .map(([path, version]) => `${path}: ${version ?? 'missing version'}`);
if (mismatches.length) {
  throw new Error(`Expected version ${tag.slice(1)} in:\n${mismatches.join('\n')}`);
}

await access(new URL(`next/content/releases/${tag}.md`, root));
console.log(`Package versions and website release notes agree for ${tag}`);

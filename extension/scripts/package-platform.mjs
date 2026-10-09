import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { dirname, resolve } from 'node:path';
const require = createRequire(import.meta.url);
const { name, version } = JSON.parse(readFileSync('package.json', 'utf8'));
const target = process.argv[2] || `${process.platform === 'win32' ? 'win32' : process.platform}-${process.arch}`;
if (!['linux-x64', 'darwin-arm64', 'darwin-x64', 'win32-x64'].includes(target)) throw new Error(`Unsupported package target ${target}`);
execFileSync(process.execPath, [resolve(dirname(require.resolve('@vscode/vsce/package.json')), 'vsce'), 'package', '--target', target, '--pre-release', '--out', `${name}-${target}-${version}.vsix`], { stdio: 'inherit' });

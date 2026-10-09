import { build } from 'esbuild';
import { readFileSync } from 'node:fs';
const version = JSON.parse(readFileSync('package.json', 'utf8')).version;
await build({ entryPoints: ['src/extension.ts', 'src/bridge-main.ts', 'src/core.ts', 'src/vscode-tests.ts'],
  outdir: 'dist', outExtension: { '.js': '.cjs' }, bundle: true, platform: 'node', target: 'node20',
  define: { __MULTI_CODEX_VERSION__: JSON.stringify(version) }, format: 'cjs', external: ['vscode'], sourcemap: true });

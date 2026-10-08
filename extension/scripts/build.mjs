import { build } from 'esbuild';
await build({ entryPoints: ['src/extension.ts', 'src/bridge-main.ts', 'src/core.ts', 'src/vscode-tests.ts'],
  outdir: 'dist', outExtension: { '.js': '.cjs' }, bundle: true, platform: 'node', target: 'node20',
  format: 'cjs', external: ['vscode'], sourcemap: true });

// Exercise the production broker/helper and migration service on the actual OS.
import { createRequire } from 'node:module';
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { mkdir, readdir, readFile, rm, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
const extension = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const repository = dirname(extension);
const { bundledEnginePath } = createRequire(import.meta.url)('../dist/core.cjs');
const source = resolve(process.env.MULTI_CODEX_TEST_EXTENSIONS);
const output = resolve(process.env.SHARED_GATE_OUTPUT);
const code = process.env.MULTI_CODEX_TEST_VSCODE;
if (!code) throw new Error('Native editor must be explicit');
await mkdir(output, { recursive: true });
const report = { source: process.env.GITHUB_SHA ?? 'local-uncommitted', platform: process.platform, architecture: process.arch, passed: false, checks: {} };
let phase = 'resolve-pinned-packages';
let exported;
try {
  let official;
  for (const entry of await readdir(source, { withFileTypes: true })) {
    if (!entry.isDirectory()) continue;
    const path = join(source, entry.name);
    const manifest = JSON.parse(await readFile(join(path, 'package.json'), 'utf8'));
    if (`${manifest.publisher}.${manifest.name}`.toLowerCase() === 'openai.chatgpt' && manifest.version === '26.1002.51308') official = path;
  }
  if (!official) throw new Error('Pinned official package unavailable');
  const engine = await bundledEnginePath(official);
  const bridge = join(repository, 'tauri/resources/managed-extension');
  const helper = join(bridge, 'bin', process.platform === 'win32' ? 'multi-codex-account-helper.exe' : 'multi-codex-account-helper');
  const stamp = JSON.parse(await readFile(join(bridge, 'desktop-helper.json'), 'utf8'));
  report.appVersion = stamp.version;
  report.editorVersion = '1.140.0';
  report.officialExtensionVersion = '26.1002.51308';
  report.engineVersion = spawnSync(engine, ['--version'], { encoding: 'utf8', windowsHide: true }).stdout?.trim();
  report.helperDigest = createHash('sha256').update(await readFile(helper)).digest('hex');
  if (report.helperDigest !== stamp.digest) throw new Error('Packaged helper fingerprint does not match');
  exported = join(process.env.RUNNER_TEMP ?? output, 'inert-native-sharing-rollout.jsonl');
  const env = { ...process.env, MULTI_CODEX_TEST_ENGINE: engine, MULTI_CODEX_TEST_SHARED_HELPER: helper,
    MULTI_CODEX_TEST_ROLLOUT_EXPORT: exported, MULTI_CODEX_TEST_OFFICIAL_PACKAGE: official,
    MULTI_CODEX_TEST_BRIDGE_PACKAGE: bridge, MULTI_CODEX_TEST_ROLLOUT: exported };
  function run(binary, args, cwd) {
    const result = spawnSync(binary, args, { env, cwd, stdio: 'inherit', windowsHide: true });
    if (result.status !== 0) throw new Error('Native acceptance stage failed');
  }
  phase = 'production-broker-handoff';
  run(process.execPath, ['--test', '--test-timeout=120000', 'tests/shared-native.test.mjs'], extension);
  report.checks.productionBrokerFullHistoryIdentityAndWriterHandoff = true;
  phase = 'production-migration-rollback';
  run('cargo', ['test', '--locked', '--lib', 'native_shared_migration', '--', '--ignored'], join(repository, 'tauri/src-tauri'));
  report.checks.productionMigrationBackupCopilotImportAndRollback = true;
  report.passed = true;
} catch {
  report.failedStage = phase;
  process.exitCode = 1;
} finally {
  if (exported) await rm(exported, { force: true });
  await writeFile(join(output, 'production-result.json'), JSON.stringify(report, null, 2));
  console.log(JSON.stringify(report, null, 2));
}

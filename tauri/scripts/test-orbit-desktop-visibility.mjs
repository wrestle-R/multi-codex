// Reuse the two-account native GUI acceptance for production UI visibility,
// background-cache exclusion, sidebar hiding, cold reopen and observer death.
// Run inside with-disposable-session.sh; no real profiles are attached.
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { fileURLToPath } from 'node:url';
import { join } from 'node:path';
const root = fileURLToPath(new URL('../../', import.meta.url));
const child = spawn(process.execPath, ['--experimental-websocket', join(root, 'tauri/scripts/test-hud-desktop.mjs')], {
  cwd: root,
  env: { ...process.env, MULTI_CODEX_TEST_SHARED_HELPER: process.env.MULTI_CODEX_TEST_SHARED_HELPER || join(root, 'tauri/src-tauri/target/debug/multi-codex-account-helper') },
  stdio: 'inherit',
});
const [code, signal] = await once(child, 'exit');
if (signal) throw new Error(`Desktop visibility acceptance ended with ${signal}`);
process.exitCode = code;

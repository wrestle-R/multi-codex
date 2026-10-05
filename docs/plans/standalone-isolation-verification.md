# Standalone isolation verification

The intended launch buttons share each existing profile's `codex-home` between VS Code and the standalone app. The desktop also gets its own `desktop-data` directory, separate from `vscode-user-data`. Existing account IDs and profile locations stay unchanged. Standalone launches remain disabled until authenticated isolation passes on Linux and macOS.

## Automated disposable probes

`tauri/scripts/standalone-isolation.py` creates private temporary homes, Codex homes, desktop data and XDG paths. It clears inherited Codex/OpenAI/VS Code overrides. It never imports saved Multi Codex credentials. Its process controls check the exact disposable desktop data argument and process group before stopping a process.

```sh
python3 tauri/scripts/standalone-isolation.py smoke /usr/lib/chatgpt/ChatGPT --output /tmp/standalone-result
```

The signed-out smoke test requires two concurrent instances, separate cookie databases, independent restarts and persistent per-instance storage markers. Linux checks fresh desktop window-reveal logs; macOS additionally requires visible Quartz windows belonging to each PID. This is startup/storage evidence only, not authenticated account-isolation proof.

The manually triggered **Standalone desktop isolation probe** GitHub workflow installs the current first-party Apple Silicon desktop package into a temporary Applications directory on `macos-26`, then runs the same probe. Only its sanitized `result.json` is uploaded. Desktop state, auth files and raw logs must never be uploaded.

## Interactive test accounts

```sh
python3 tauri/scripts/standalone-isolation.py start /usr/lib/chatgpt/ChatGPT
```

This leaves two disposable windows open and prints their private temporary root. Interactive stdout/stderr goes to `/dev/null`, so a full temporary log cannot cause a Node console write exception. Sign into a different disposable account in each window, then use the printed root:

```sh
python3 tauri/scripts/standalone-isolation.py status "$PROBE_ROOT"
python3 tauri/scripts/standalone-isolation.py checkpoint "$PROBE_ROOT"
python3 tauri/scripts/standalone-isolation.py restart "$PROBE_ROOT" a
python3 tauri/scripts/standalone-isolation.py check "$PROBE_ROOT" b
```

`status` reports only credential presence and SHA-256 fingerprints; it does not print tokens, emails or account IDs. `check` compares the peer credential, identity fingerprint and storage marker. A token changing from its own independent refresh needs investigation rather than being treated automatically as cross-account contamination.

Repeat symmetrically, force an authenticated refresh, and sign out through each actual desktop window. Confirm the other window still has the original identity and can make authenticated requests. Confirm sessions remain separate and restart does not silently restore the signed-out identity from another store. Repeat on macOS, including desktop cookies and Keychain behavior. CLI-only or signed-out tests cannot approve the desktop UI's identity behavior.

```sh
python3 tauri/scripts/standalone-isolation.py stop "$PROBE_ROOT"
```

Stopping retains private test data for inspection. Never commit that directory, copy production accounts into it, or reuse production tokens on a GitHub runner.

## Results on 2026-10-05

- Installed Linux desktop: `openai-codex-electron` version `26.930.51102`.
- Initial signed-out Linux probe passed concurrent startup, distinct cookie databases and both independent restarts.
- A second local smoke attempt was interrupted by a desktop exiting; this is not a passing result. The initial result remains the current signed-out evidence.
- During interactive setup, `/tmp` reached its quota and Node console writes raised `UNKNOWN: unknown error, write`. The earlier stopped signed-out probe was removed after checking it had no credentials or running desktop processes. Interactive test windows were restarted with logging redirected to `/dev/null`; the first disposable login's auth file was preserved byte for byte.
- Two interactive windows are running; one disposable login is present. Authenticated Linux and Mac refresh/logout isolation remain pending.
- The bundled Codex CLI recognized that disposable desktop login using the exact same Codex home; its status check left the credential file byte-for-byte unchanged. This shows desktop-created credentials are usable by the CLI, not yet that desktop startup automatically accepts every existing VS Code credential.
- [Mac GitHub runner probe 37338947965](https://github.com/wrestle-R/multi-codex/actions/runs/37338947965) passed on `macos-26`: two native Quartz-visible desktop windows, separate cookie databases and independent restarts with storage markers preserved. The sanitized report explicitly records authenticated isolation as **not tested**.
- All six existing Multi Codex account IDs matched the pre-update backup, and all six existing auth files were still present after these probes.
- This verification work does not change the installed Multi Codex executable or its saved profiles.

## Public documentation and implementation boundary

[Official authentication documentation](https://learn.chatgpt.com/docs/auth) documents file-based credentials under `CODEX_HOME` and device-code login for headless environments. [App-server documentation](https://learn.chatgpt.com/docs/app-server) documents `account/read` with `refreshToken: true` and `account/logout`.

Read-only inspection of the installed desktop found `CODEX_ELECTRON_USER_DATA_PATH` handling and explicit `CODEX_HOME` preservation for isolated desktop instances. This variable is an internal implementation detail, absent from the [stable public environment-variable list](https://learn.chatgpt.com/docs/config-file/environment-variables). `CODEX_HOME` alone does not isolate desktop cookies or prove separate authenticated sessions.

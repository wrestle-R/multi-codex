//! Private stdio interface for the VS Code companion. Credentials never enter log output.
use crate::profiles::{self, AuthRecognizer, KeyringSecretStore, ProfileService, SaveProfileInput};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::path::PathBuf;

struct BundledRecognizer(PathBuf);
impl AuthRecognizer for BundledRecognizer {
    fn recognize(&self, auth: &str) -> profiles::Result<()> {
        let directory =
            tempfile::tempdir().map_err(|_| "Could not prepare credential validation")?;
        profiles::write_private_file(&directory.path().join("auth.json"), auth.as_bytes())?;
        profiles::write_codex_config(directory.path())?;
        let output = crate::process::background(&mut std::process::Command::new(&self.0))
            .args(["login", "status"])
            .env("CODEX_HOME", directory.path())
            .env_remove("OPENAI_API_KEY")
            .env_remove("CODEX_API_KEY")
            .output()
            .map_err(|_| "The Codex extension's bundled engine could not validate the account")?;
        if output.status.success() {
            Ok(())
        } else {
            Err("Codex did not recognize this credential. Sign in again.".into())
        }
    }
}

pub fn run_account_helper() -> profiles::Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() == 2 && args[0] == "--protect-directory" {
        let path = PathBuf::from(&args[1]);
        let metadata = std::fs::symlink_metadata(&path)
            .map_err(|_| "The private runtime directory is unavailable")?;
        if !path.is_absolute() || !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err("The private runtime must be an absolute directory, not a link".into());
        }
        return crate::file_security::protect(&path, true);
    }
    if args.len() != 3 {
        return Err("Expected engine, data directory and global Codex home".into());
    }
    let engine = PathBuf::from(&args[0]);
    if !engine.is_absolute() || !profiles::is_executable_file(&engine) {
        return Err("Invalid bundled engine".into());
    }
    let data_root = PathBuf::from(&args[1]);
    let global_home = PathBuf::from(&args[2]);
    if !data_root.is_absolute() || !global_home.is_absolute() {
        return Err("Account paths must be absolute".into());
    }
    let service = ProfileService::new(
        data_root,
        global_home,
        PathBuf::new(),
        KeyringSecretStore,
        BundledRecognizer(engine),
    )?;
    let mut stdout = std::io::stdout().lock();
    let mut leases = HashMap::new();
    for line in std::io::stdin().lock().lines() {
        let line = line.map_err(|_| "Account helper input closed")?;
        if line.len() > 2 * 1024 * 1024 {
            return Err("Account request too large".into());
        }
        let request: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let id = request.get("id").cloned().unwrap_or(Value::Null);
        let result = dispatch(&service, &request, &mut leases);
        let reply = match result {
            Ok(result) => json!({"id":id,"result":result}),
            Err(message) => json!({"id":id,"error":{"code":-32000,"message":message}}),
        };
        serde_json::to_writer(&mut stdout, &reply).map_err(|_| "Account helper output closed")?;
        writeln!(stdout)
            .and_then(|_| stdout.flush())
            .map_err(|_| "Account helper output closed")?;
    }
    Ok(())
}

fn dispatch(
    service: &ProfileService<KeyringSecretStore, BundledRecognizer>,
    request: &Value,
    leases: &mut HashMap<String, std::fs::File>,
) -> profiles::Result<Value> {
    let params = &request["params"];
    let string = |key: &str| {
        params[key]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| format!("Missing {key}"))
    };
    match request["method"].as_str().unwrap_or_default() {
        "accounts/lockCredential" => {
            let id = string("id")?;
            if leases.contains_key(&format!("credential:{id}")) {
                return Err("Another request is checking this account. Retry in a moment.".into());
            }
            let session = service.extension_lease(&id)?;
            let credential = service.extension_credential_lock(&id)?;
            leases.insert(format!("credential-session:{id}"), session);
            leases.insert(format!("credential:{id}"), credential);
            Ok(json!({}))
        }
        "accounts/unlockCredential" => {
            let id = string("id")?;
            leases.remove(&format!("credential:{id}"));
            leases.remove(&format!("credential-session:{id}"));
            Ok(json!({}))
        }
        "accounts/lease" => {
            let id = string("id")?;
            if !leases.contains_key(&id) {
                leases.insert(id.clone(), service.extension_lease(&id)?);
            }
            Ok(json!({}))
        }
        "accounts/release" => {
            leases.remove(&string("id")?);
            Ok(json!({}))
        }
        "accounts/list" => serde_json::to_value(service.list_profiles()?)
            .map_err(|_| "Could not encode account list".into()),
        "accounts/credential" => Ok(json!({"auth":service.extension_credential(&string("id")?)?})),
        "accounts/add" => {
            let input: SaveProfileInput =
                serde_json::from_value(params.clone()).map_err(|_| "Invalid account details")?;
            serde_json::to_value(service.add_profile(input)?)
                .map_err(|_| "Could not encode account".into())
        }
        "accounts/import" => serde_json::to_value(service.import_current(string("name")?, None)?)
            .map_err(|_| "Could not encode account".into()),
        "accounts/rename" => {
            let id = string("id")?;
            let existing = service
                .list_profiles()?
                .into_iter()
                .find(|profile| profile.metadata.id == id)
                .ok_or("Account not found")?;
            serde_json::to_value(service.update_profile(
                &id,
                string("name")?,
                None,
                existing.metadata.notes,
            )?)
            .map_err(|_| "Could not encode account".into())
        }
        "accounts/delete" => {
            service.delete_profile(&string("id")?)?;
            Ok(json!({}))
        }
        _ => Err("Unknown account helper method".into()),
    }
}

/// Windows uses a native executable wrapper, avoiding cmd.exe and shell quoting.
pub fn run_bridge_launcher() -> profiles::Result<bool> {
    #[cfg(windows)]
    {
        let executable =
            std::env::current_exe().map_err(|_| "Could not resolve bridge launcher")?;
        let config = executable.with_extension("json");
        if !config.is_file() {
            return Ok(false);
        }
        let settings: Value = serde_json::from_slice(
            &std::fs::read(&config).map_err(|_| "Could not read bridge config")?,
        )
        .map_err(|_| "Invalid bridge config")?;
        let mut command = std::process::Command::new(
            settings["nodeExecutable"]
                .as_str()
                .ok_or("Missing bridge runtime")?,
        );
        command
            .arg(
                settings["bridgeScript"]
                    .as_str()
                    .ok_or("Missing bridge script")?,
            )
            .arg("--bridge-config")
            .arg(config)
            .args(std::env::args_os().skip(1))
            .env("ELECTRON_RUN_AS_NODE", "1");
        let mut system = sysinfo::System::new();
        system.refresh_processes(
            sysinfo::ProcessesToUpdate::Some(&[sysinfo::Pid::from_u32(std::process::id())]),
            true,
        );
        if let Some(parent) = system
            .process(sysinfo::Pid::from_u32(std::process::id()))
            .and_then(|p| p.parent())
        {
            command.env("MULTI_CODEX_HOST_PID", parent.as_u32().to_string());
        }
        let status = command
            .status()
            .map_err(|_| "Could not start bridge runtime")?;
        std::process::exit(status.code().unwrap_or(1));
    }
    #[cfg(not(windows))]
    Ok(false)
}

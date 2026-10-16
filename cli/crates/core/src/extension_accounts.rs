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

fn multi_hud_desktop(args: &[std::ffi::OsString]) -> profiles::Result<()> {
    crate::hud_desktop::launch(args)
}

pub fn run_account_helper() -> profiles::Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.first().is_some_and(|arg| arg == "--hud-desktop") {
        return multi_hud_desktop(&args[1..]);
    }
    if args.first().is_some_and(|arg| arg == "--hud-proxy") {
        return crate::hud_model::proxy(&args[1..]);
    }
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
        data_root.clone(),
        global_home,
        PathBuf::new(),
        KeyringSecretStore,
        BundledRecognizer(engine.clone()),
    )?;
    let mut stdout = std::io::stdout().lock();
    let mut leases = HashMap::new();
    let mut thread_store = None;
    let mut thread_leases = HashMap::new();
    let mut library = None;
    let mut readers: HashMap<String, (tempfile::TempDir, PathBuf)> = HashMap::new();
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
        let result = if request["method"] == "orbit/publish" {
            crate::hud_sessions::publish(&data_root, &engine, &request["params"])
        } else if request["method"] == "orbit/focus" {
            let params = &request["params"];
            crate::hud_sessions::focus(
                &data_root,
                params["profileId"].as_str().unwrap_or(""),
                params["sessionId"].as_str().unwrap_or(""),
                params["pid"]
                    .as_u64()
                    .and_then(|p| u32::try_from(p).ok())
                    .unwrap_or(0),
            )
            .map(|()| json!({"focused":true}))
        } else if request["method"]
            .as_str()
            .is_some_and(|method| method.starts_with("chats/"))
        {
            (|| -> profiles::Result<Value> {
                if thread_store.is_none() {
                    thread_store = Some(crate::shared_chat_leases::ThreadLeases::open(
                        &data_root, &engine,
                    )?);
                }
                if library.is_none() {
                    library = Some(crate::shared_chats::ChatLibrary::open(&data_root)?);
                }
                let result = dispatch_thread(
                    thread_store.as_ref().unwrap(),
                    library.as_ref().unwrap(),
                    &request,
                    &mut thread_leases,
                    &mut readers,
                )?;
                if matches!(
                    request["method"].as_str(),
                    Some(
                        "chats/checkpoint"
                            | "chats/checkpointStopped"
                            | "chats/register"
                            | "chats/registerStopped"
                    )
                ) {
                    library.as_ref().unwrap().collect_unreachable()?;
                }
                Ok(result)
            })()
        } else {
            dispatch(&service, &request, &mut leases)
        };
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

fn dispatch_thread(
    store: &crate::shared_chat_leases::ThreadLeases,
    library: &crate::shared_chats::ChatLibrary,
    request: &Value,
    leases: &mut HashMap<String, crate::shared_chat_leases::WriterLease>,
    readers: &mut HashMap<String, (tempfile::TempDir, PathBuf)>,
) -> profiles::Result<Value> {
    let params = &request["params"];
    if request["method"] == "chats/changes" {
        return library.changes(params["cursor"].as_i64());
    }
    if request["method"] == "chats/recoverable" {
        return serde_json::to_value(store.recoverable()?)
            .map_err(|_| "Cannot encode retained chat recovery".into());
    }
    if request["method"] == "chats/list" {
        let archived = params["archived"].as_bool().unwrap_or(false);
        let offset = params["offset"]
            .as_u64()
            .and_then(|value| u32::try_from(value).ok())
            .unwrap_or(0);
        let limit = params["limit"]
            .as_u64()
            .and_then(|value| u32::try_from(value).ok())
            .unwrap_or(100);
        return serde_json::to_value(library.list(archived, offset, limit)?)
            .map_err(|_| "Cannot encode shared chat list".into());
    }
    let thread = params["threadId"]
        .as_str()
        .ok_or("Missing shared thread identifier")?;
    match request["method"].as_str().unwrap_or_default() {
        "chats/get" => serde_json::to_value(library.get(thread)?)
            .map_err(|_| "Cannot encode shared chat".into()),
        "chats/recover" => serde_json::to_value(crate::shared_chat_import::recover_writer(
            &store.data_root,
            &store.engine,
            store,
            library,
            thread,
        )?)
        .map_err(|_| "Could not encode recovered chat".into()),
        "chats/read" | "chats/observe" | "chats/nativeRead" => {
            let saved = library.get(thread)?;
            // Keep revision-specific read views alive for this frontend's
            // helper lifetime. Returning paths into a dropped TempDir caused
            // fs/getMetadata errors and disappearing attachment previews.
            let key = format!("{}:{}", saved.id, saved.checkpoint);
            if !readers.contains_key(&key) {
                let home = store.read_home()?;
                let path = library.hydrate(thread, home.path())?;
                readers.insert(key.clone(), (home, path));
            }
            let (home, path) = readers.get(&key).ok_or("Snapshot reader unavailable")?;
            let mut server = crate::chat_cleanup::LocalServer::start(&store.engine, home.path())?;
            let mut resumed = server.request(
                "thread/resume",
                // Decode the copy without importing account/provider secrets.
                json!({"threadId":saved.native_id,"path":path,"modelProvider":"openai","sandbox":"read-only","approvalPolicy":"never"}),
            )?;
            if request["method"] == "chats/nativeRead" {
                let method = params["method"]
                    .as_str()
                    .ok_or("Missing native read method")?;
                if !matches!(
                    method,
                    "thread/turns/list"
                        | "thread/items/list"
                        | "thread/attachment/list"
                        | "thread/attachmentOwner/list"
                        | "thread/goal/get"
                ) {
                    return Err("This native shared read is not verified".into());
                }
                let mut arguments = params["params"].clone();
                let object = arguments
                    .as_object_mut()
                    .ok_or("Invalid native read parameters")?;
                object.insert("threadId".into(), Value::String(saved.native_id));
                return server.request(method, arguments);
            }
            let mut result = server.request("thread/read", json!({"threadId":saved.native_id,"includeTurns":params["includeTurns"].as_bool().unwrap_or(true)}))?;
            result["thread"]["id"] = Value::String(saved.id);
            result["thread"]["modelProvider"] = saved.metadata["modelProvider"].clone();
            // These are isolated snapshot reads, never a writable canonical backend.
            result["thread"]["status"] = json!({"type":"notLoaded"});
            if request["method"] == "chats/observe" {
                resumed["thread"] = result["thread"].clone();
                return Ok(resumed);
            }
            Ok(result)
        }
        "chats/archive" | "chats/unarchive" | "chats/delete" => {
            let _lease = leases
                .get(thread)
                .ok_or("Acquire the shared chat before changing its library state")?;
            store.verify_writer(_lease)?;
            if request["method"] == "chats/delete" {
                library.tombstone(store, _lease)?;
            } else {
                library.set_archived(store, _lease, request["method"] == "chats/archive")?;
            }
            Ok(json!({}))
        }
        "chats/hydrate" => {
            let lease = leases
                .get(thread)
                .ok_or("Acquire this chat before hydrating writable state")?;
            store.verify_writer(lease)?;
            let rollout = library.hydrate(thread, &lease.owner.session_home)?;
            Ok(json!({"path":rollout,"thread":library.get(thread)?}))
        }
        "chats/checkpoint" | "chats/checkpointStopped" => {
            let lease = leases
                .get(thread)
                .ok_or("This window does not own the shared chat")?;
            let path = PathBuf::from(params["path"].as_str().ok_or("Missing native rollout")?);
            let assets: Vec<(String, PathBuf)> =
                serde_json::from_value(params.get("assets").cloned().unwrap_or_else(|| json!([])))
                    .map_err(|_| "Invalid checkpoint attachment mappings")?;
            let committed = if request["method"] == "chats/checkpointStopped" {
                library.publish_stopped(store, lease, &path, &params["metadata"], &assets)?
            } else {
                library.publish_writer(store, lease, &path, &params["metadata"], &assets)?
            };
            serde_json::to_value(committed).map_err(|_| "Cannot encode committed checkpoint".into())
        }
        "chats/register" | "chats/registerStopped" => {
            let lease = leases
                .get(thread)
                .ok_or("Acquire this new chat before registering it")?;
            let verify = || {
                if request["method"] == "chats/registerStopped" {
                    store.verify_stopped(lease)
                } else {
                    store.verify_writer(lease)
                }
            };
            verify()?;
            let profile = params["profileId"]
                .as_str()
                .ok_or("Missing source account")?;
            let path = PathBuf::from(params["path"].as_str().ok_or("Missing native rollout")?);
            let assets: Vec<(String, PathBuf)> =
                serde_json::from_value(params.get("assets").cloned().unwrap_or_else(|| json!([])))
                    .map_err(|_| "Invalid chat attachment mappings")?;
            let committed = library.import_owned(
                store,
                lease,
                profile,
                &path,
                &params["metadata"],
                &assets,
                request["method"] == "chats/registerStopped",
            )?;
            verify()?;
            serde_json::to_value(committed).map_err(|_| "Cannot encode registered chat".into())
        }
        "chats/acquire" => {
            let instance = params["instanceId"]
                .as_str()
                .ok_or("Missing bridge identity")?;
            let pid = params["backendPid"]
                .as_u64()
                .and_then(|pid| u32::try_from(pid).ok())
                .ok_or("Missing native backend PID")?;
            let home = PathBuf::from(
                params["sessionHome"]
                    .as_str()
                    .ok_or("Missing private backend home")?,
            );
            if let Some(existing) = leases.get(thread) {
                if existing.owner.instance_id != instance
                    || existing.owner.backend_pid != pid
                    || existing.owner.session_home != home
                {
                    return Err("This helper already holds the chat for a different backend".into());
                }
                store.verify_writer(existing)?;
            } else {
                leases.insert(
                    thread.to_owned(),
                    store.acquire(thread, instance, pid, &home)?,
                );
            }
            if let Some(profile) = params["profileId"].as_str() {
                store.set_profile(
                    leases.get_mut(thread).ok_or("Missing acquired writer")?,
                    profile,
                )?;
            }
            serde_json::to_value(&leases[thread].owner)
                .map_err(|_| "Could not encode chat writer".into())
        }
        "chats/verify" => {
            store.verify_writer(
                leases
                    .get(thread)
                    .ok_or("This window does not own the shared chat")?,
            )?;
            Ok(json!({}))
        }
        "chats/release" => {
            if let Some(lease) = leases.get(thread) {
                store.release(lease)?;
            }
            leases.remove(thread);
            Ok(json!({}))
        }
        _ => Err("Unknown shared chat ownership method".into()),
    }
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
        "accounts/confirmSharedLaunch" => {
            let id = string("id")?;
            if !leases.contains_key(&id) {
                return Err(
                    "Acquire the saved account before confirming its managed launch".into(),
                );
            }
            service.confirm_shared_launch(&id)?;
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
            .env_remove("NODE_OPTIONS")
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
        let status = crate::process::background(&mut command)
            .status()
            .map_err(|_| "Could not start bridge runtime")?;
        std::process::exit(status.code().unwrap_or(1));
    }
    #[cfg(not(windows))]
    Ok(false)
}

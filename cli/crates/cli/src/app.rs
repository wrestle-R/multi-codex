use crate::{
    args::*,
    error::{CliError, Result},
};
use multi_codex_core::{
    chat_cleanup::ChatScope,
    profiles::{
        self, CodexCliRecognizer, KeyringSecretStore, ProfileService, ProfileView,
        SaveProfileInput, SecretStore,
    },
    settings, terminals,
};
use serde::Serialize;
use serde_json::{json, Value};
use std::{
    io::{self, IsTerminal, Read, Write},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

pub type Service = ProfileService<KeyringSecretStore, CodexCliRecognizer>;
pub fn interactive() -> bool {
    io::stdin().is_terminal() && io::stderr().is_terminal()
}
pub fn emit(json_output: bool, value: &Value) {
    if json_output {
        println!("{}", json!({"schemaVersion":1,"ok":true,"data":value}));
        return;
    }
    if let Some(rows) = value.as_array() {
        if rows.is_empty() {
            println!("No results.");
            return;
        }
        if rows.iter().all(|row| row.get("name").is_some()) {
            for row in rows {
                let name = row["name"].as_str().unwrap_or("");
                if row.get("usage").is_some() {
                    println!(
                        "{}{}",
                        name,
                        if row["stale"] == true {
                            " (stale saved result)"
                        } else {
                            ""
                        }
                    );
                    for (key, label) in [
                        ("fiveHour", "Five hours"),
                        ("weekly", "Weekly"),
                        ("monthly", "Monthly"),
                    ] {
                        let window = &row["usage"][key];
                        if !window.is_null() {
                            let reset = window["resetsAt"]
                                .as_i64()
                                .and_then(|at| chrono::DateTime::from_timestamp(at, 0))
                                .map(|at| at.to_rfc3339())
                                .unwrap_or_else(|| "not reported".into());
                            println!(
                                "  {label}: {}% remaining; resets {reset}",
                                window["remainingPercent"]
                            );
                        }
                    }
                    if let Some(count) = row["usage"]["resetCreditsAvailable"].as_u64() {
                        println!("  Available reset credits: {count}");
                    }
                    if let Some(error) = row["error"].as_str() {
                        println!("  Refresh failed: {error}");
                    }
                } else {
                    println!(
                        "{}  [{}]  {}",
                        name,
                        row["id"].as_str().unwrap_or(""),
                        row["status"].as_str().unwrap_or("")
                    );
                    if let Some(tier) = row["accountTier"].as_str() {
                        println!("  Plan: {tier}");
                    }
                    if let Some(error) = row["error"].as_str() {
                        println!("  {error}");
                    }
                }
            }
            return;
        }
    }
    println!(
        "{}",
        serde_json::to_string_pretty(value).unwrap_or_default()
    );
}

fn value<T: Serialize>(data: T) -> Result<Value> {
    Ok(serde_json::to_value(data)?)
}
fn confirm(yes: bool, prompt: &str) -> Result<()> {
    if yes {
        return Ok(());
    }
    if !interactive() {
        return Err(CliError::new(
            2,
            "This action requires --yes in non-interactive use.",
        ));
    }
    eprint!("{prompt} [y/N] ");
    io::stderr().flush()?;
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    if line.trim().eq_ignore_ascii_case("y") || line.trim().eq_ignore_ascii_case("yes") {
        Ok(())
    } else {
        Err(CliError::new(130, "Cancelled; no changes applied."))
    }
}
fn resolve(service: &Service, account: &str) -> Result<ProfileView> {
    let matches: Vec<_> = service
        .list_profiles()?
        .into_iter()
        .filter(|p| p.metadata.id == account || p.metadata.name == account)
        .collect();
    match matches.len() {
        1 => Ok(matches.into_iter().next().unwrap()),
        0 => Err(CliError::new(
            2,
            "Account not found. Use accounts list for exact names or IDs.",
        )),
        _ => Err(CliError::new(
            2,
            "Account name is ambiguous. Use its complete ID.",
        )),
    }
}
fn selected(service: &Service, selection: &Selection) -> Result<Vec<ProfileView>> {
    if let Some(account) = &selection.account {
        return Ok(vec![resolve(service, account)?]);
    }
    if selection.all {
        return Ok(service.list_profiles()?);
    }
    Err(CliError::new(
        2,
        "Select --account NAME or --all explicitly.",
    ))
}
fn cleanup_accounts(service: &Service, selection: &Cleanup) -> Result<Vec<ProfileView>> {
    selected(
        service,
        &Selection {
            account: selection.account.clone(),
            all: selection.all_accounts,
        },
    )
}
fn read_auth(path: Option<PathBuf>, stdin: bool) -> Result<String> {
    let mut bytes = Vec::new();
    if stdin {
        io::stdin().take(1024 * 1024 + 1).read_to_end(&mut bytes)?;
    } else if let Some(path) = path {
        let file = std::fs::File::open(path)?;
        file.take(1024 * 1024 + 1).read_to_end(&mut bytes)?;
    } else {
        return Err(CliError::new(
            2,
            "Use --file PATH or --stdin for credential import.",
        ));
    }
    if bytes.len() > 1024 * 1024 {
        return Err(CliError::new(2, "Credential file exceeds 1 MiB."));
    }
    String::from_utf8(bytes).map_err(|_| CliError::new(2, "Credential input must be UTF-8 JSON."))
}
fn bridge() -> Result<PathBuf> {
    let executable = std::env::current_exe()?;
    let root = executable
        .parent()
        .ok_or_else(|| CliError::new(3, "Cannot locate package resources."))?;
    let path = root.join("resources/bridge");
    if path.is_dir() {
        Ok(path)
    } else {
        Err(CliError::new(
            3,
            "Managed VS Code bridge is missing. Reinstall the platform package.",
        ))
    }
}
fn managed_codex_home(root: &std::path::Path, candidate: &std::path::Path) -> bool {
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let candidate = candidate
        .canonicalize()
        .unwrap_or_else(|_| candidate.to_path_buf());
    let Ok(relative) = candidate.strip_prefix(root) else {
        return false;
    };
    let parts: Vec<_> = relative.components().collect();
    let canonical_id = |part: &std::path::Component<'_>| {
        part.as_os_str().to_str().is_some_and(|id| {
            uuid::Uuid::parse_str(id).is_ok_and(|parsed| parsed.to_string() == id)
        })
    };
    if parts.len() < 3 || parts[0].as_os_str() != "profiles" || !canonical_id(&parts[1]) {
        return false;
    }
    if parts[2].as_os_str() == "codex-home" {
        return true;
    }
    let tail = &parts[2..];
    let runtime = if tail[0].as_os_str() == "bridge-runtime" {
        &tail[1..]
    } else {
        let prefix = [
            "vscode-user-data",
            "User",
            "globalStorage",
            "russeldanielpaul.multi-codex",
            "runtime",
        ];
        if tail.len() < prefix.len()
            || !tail
                .iter()
                .zip(prefix)
                .all(|(part, name)| part.as_os_str() == name)
        {
            return false;
        }
        &tail[prefix.len()..]
    };
    runtime.len() >= 2
        && matches!(runtime[0].as_os_str().to_str(), Some("windows" | "logins"))
        && canonical_id(&runtime[1])
}

fn initialize(cli: &Cli) -> Result<Service> {
    if let Some(path) = &cli.data_dir {
        if !path.is_absolute() {
            return Err(CliError::new(2, "--data-dir requires an absolute path."));
        }
        std::env::set_var("MULTI_CODEX_DATA_DIR", path);
    }
    let home = dirs::home_dir().ok_or_else(|| CliError::new(3, "Home directory unavailable."))?;
    let root = settings::data_root()?;
    let global = if let Some(path) = &cli.global_codex_home {
        if !path.is_absolute() {
            return Err(CliError::new(
                2,
                "--global-codex-home requires an absolute path.",
            ));
        }
        path.clone()
    } else {
        let environment = std::env::var_os("CODEX_HOME")
            .filter(|candidate| !managed_codex_home(&root, &PathBuf::from(candidate)));
        settings::global_codex_home(&home, environment, &settings::load()?)?
    };
    ProfileService::new(
        root,
        global,
        home.join(".vscode/extensions"),
        KeyringSecretStore,
        CodexCliRecognizer,
    )
    .map_err(Into::into)
}
fn menu(service: &Service) -> Result<Command> {
    let accounts = service.list_profiles()?;
    eprintln!("Multi Codex\n0. Add account");
    for (index, account) in accounts.iter().enumerate() {
        eprintln!(
            "{}. {} ({:?})",
            index + 1,
            account.metadata.name,
            account.status
        );
    }
    eprint!("Account: ");
    io::stderr().flush()?;
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    let index = line
        .trim()
        .parse::<usize>()
        .map_err(|_| CliError::new(2, "Enter an account number."))?;
    if index == 0 {
        eprint!("Account name: ");
        io::stderr().flush()?;
        line.clear();
        io::stdin().read_line(&mut line)?;
        return Ok(Command::Accounts {
            command: Accounts::Add {
                name: line.trim().into(),
                notes: None,
                no_browser: false,
            },
        });
    }
    let account = accounts
        .get(index - 1)
        .ok_or_else(|| CliError::new(2, "Account number unavailable."))?;
    let name = account.metadata.id.clone();
    eprintln!("1. Open new Codex terminal\n2. Open VS Code\n3. Show usage\n4. Show account");
    line.clear();
    io::stdin().read_line(&mut line)?;
    Ok(match line.trim() {
        "1" | "2" => Command::Launch(Launch {
            account: name,
            target: Some(if line.trim() == "1" {
                Target::Cli
            } else {
                Target::Vscode
            }),
            workspace: PathBuf::from("."),
            terminal: None,
            desktop: None,
        }),
        "3" => Command::Usage(Usage {
            selection: Selection {
                account: Some(name),
                all: false,
            },
            watch: false,
            interval: 60,
        }),
        "4" => Command::Accounts {
            command: Accounts::Show { account: name },
        },
        _ => return Err(CliError::new(2, "Select an action from 1 to 4.")),
    })
}

pub fn run(cli: &mut Cli, stop: &Arc<AtomicBool>) -> Result<()> {
    if !cfg!(any(target_os = "linux", target_os = "macos")) {
        return Err(CliError::new(3, "The CLI supports macOS and Linux only."));
    }
    // Package-independent inspection should not create an account store.
    let command = cli.command.take();
    match command {
        Some(Command::Update { check }) => {
            emit(cli.json, &update(check)?);
            return Ok(());
        }
        Some(Command::Terminals { .. }) => {
            emit(cli.json, &value(terminals::options())?);
            return Ok(());
        }
        Some(Command::Desktops { .. }) => {
            emit(
                cli.json,
                &value(multi_codex_core::desktop_environment::inventory()?)?,
            );
            return Ok(());
        }
        _ => {}
    }
    let service = initialize(cli)?;
    let command = match command {
        Some(command) => command,
        None => menu(&service)?,
    };
    let result = match command {
        Command::Accounts { command } => match command {
            Accounts::List => value(service.list_profiles()?)?,
            Accounts::Show { account } => value(resolve(&service, &account)?)?,
            Accounts::Import {
                name,
                notes,
                file,
                stdin,
            } => value(service.add_profile(SaveProfileInput {
                name,
                notes,
                auth_json: read_auth(file, stdin)?,
            })?)?,
            Accounts::ImportCurrent { name, notes } => value(service.import_current(name, notes)?)?,
            Accounts::Add {
                name,
                notes,
                no_browser,
            } => value(crate::login::sign_in(
                &service,
                service.prepare_device_login(name, notes)?,
                no_browser,
                stop,
            )?)?,
            Accounts::Reconnect {
                account,
                no_browser,
            } => {
                let account = resolve(&service, &account)?;
                value(crate::login::sign_in(
                    &service,
                    service.prepare_profile_reauthentication(&account.metadata.id)?,
                    no_browser,
                    stop,
                )?)?
            }
            Accounts::Edit {
                account,
                name,
                notes,
                clear_notes,
            } => {
                let old = resolve(&service, &account)?;
                value(service.update_profile(
                    &old.metadata.id,
                    name.unwrap_or(old.metadata.name),
                    None,
                    if clear_notes {
                        None
                    } else {
                        notes.or(old.metadata.notes)
                    },
                )?)?
            }
            Accounts::Remove { account, yes } => {
                let account = resolve(&service, &account)?;
                confirm(
                    yes,
                    "Remove this managed account and its private data? Shared chats remain.",
                )?;
                service.delete_profile(&account.metadata.id)?;
                json!({"removed":account.metadata.id})
            }
        },
        Command::Launch(launch) => launch_account(&service, launch)?,
        Command::Usage(usage) => loop {
            let (rows, failed) = usage_rows(&service, &usage.selection, false)?;
            if usage.watch || !failed {
                emit(cli.json, &rows);
            }
            if !usage.watch {
                if failed {
                    let mut error = CliError::new(5, "Some usage checks failed.");
                    error.details = Some(rows);
                    return Err(error);
                }
                return Ok(());
            }
            let deadline = Instant::now() + Duration::from_secs(usage.interval);
            while Instant::now() < deadline {
                if stop.load(Ordering::SeqCst) {
                    return Err(CliError::new(130, "Usage watch cancelled."));
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        },
        Command::Credits(selection) => {
            let (rows, failed) = usage_rows(&service, &selection, true)?;
            if failed {
                let mut error = CliError::new(5, "Some credit checks failed.");
                error.details = Some(rows);
                return Err(error);
            }
            rows
        }
        Command::Status(selection) => {
            let accounts = if selection.account.is_none() && !selection.all {
                service.list_profiles()?
            } else {
                selected(&service, &selection)?
            };
            value(
                accounts
                    .iter()
                    .map(|p| service.runtime_status(&p.metadata.id))
                    .collect::<std::result::Result<Vec<_>, _>>()?,
            )?
        }
        Command::Storage { command, account } => match command {
            None => {
                let mut usage = value(service.storage_usage()?)?;
                if let Some(name) = account {
                    let id = resolve(&service, &name)?.metadata.id;
                    if let Some(rows) = usage["profiles"].as_array_mut() {
                        rows.retain(|p| p["id"] == id);
                    }
                }
                usage
            }
            Some(Storage::Cache {
                command: Cache::Clear(selection),
            }) => {
                let accounts = cleanup_accounts(&service, &selection)?;
                confirm(
                    selection.yes,
                    "Clear removable caches for the selected accounts?",
                )?;
                bulk(&accounts, |p| {
                    Ok(json!({"reclaimedBytes":service.clear_profile_cache(&p.metadata.id)?}))
                })?
            }
            Some(Storage::Chats { command }) => chat_command(&service, command)?,
            Some(Storage::Migration { command }) => migration(&service, command)?,
            Some(Storage::Backups { command }) => backups(&service, command)?,
            Some(Storage::Extensions { command }) => extensions(&service, command)?,
        },
        Command::Settings { command } => settings_command(command)?,
        Command::Integration { command } => integration(command)?,
        Command::Doctor => doctor(&service)?,
        Command::Version => json!({"version":env!("CARGO_PKG_VERSION")}),
        Command::Update { .. } | Command::Terminals { .. } | Command::Desktops { .. } => {
            unreachable!()
        }
    };
    emit(cli.json, &result);
    Ok(())
}
fn launch_account(service: &Service, launch: Launch) -> Result<Value> {
    let account = resolve(service, &launch.account)?;
    let preferences = settings::load()?;
    let target = match launch.target {
        Some(target) => target,
        None => match preferences.launch_mode {
            settings::LaunchMode::Cli => Target::Cli,
            settings::LaunchMode::Vscode => Target::Vscode,
            settings::LaunchMode::Standalone => Target::Codex,
            settings::LaunchMode::Both => {
                return Err(CliError::new(
                    2,
                    "Set --target explicitly when the stored preference is both.",
                ))
            }
        },
    };
    let workspace = launch.workspace.canonicalize()?;
    if !workspace.is_dir() {
        return Err(CliError::new(2, "Workspace must be an existing directory."));
    }
    if matches!(target, Target::Vscode) {
        service.set_managed_bridge(bridge()?)?;
    }
    if cfg!(target_os = "linux")
        && std::env::var_os("DISPLAY").is_none()
        && std::env::var_os("WAYLAND_DISPLAY").is_none()
    {
        return Err(CliError::new(
            3,
            "Opening a new terminal or application requires a graphical session.",
        ));
    }
    if let Some(desktop) = launch.desktop {
        if !matches!(target, Target::Vscode) {
            return Err(CliError::new(
                2,
                "--desktop is supported for verified VS Code placement.",
            ));
        }
        let home = service.vscode_home(&account.metadata.id)?;
        let workspace_text = workspace.to_string_lossy();
        let result = multi_codex_core::launch::LaunchCoordinator::default().execute(
            multi_codex_core::launch::LaunchRequest {
                profile: &account.metadata.id,
                workspace: &workspace_text,
                home: &home,
                desktop: Some(&desktop),
                retry: None,
            },
            || service.launch_profile(&account.metadata.id, &workspace),
            &multi_codex_core::launch::NativeDesktopControl,
        )?;
        if !result.completed {
            let mut error=CliError::new(3,"VS Code started but requested placement could not be verified. The account was not launched again.");
            error.details = Some(value(result)?);
            return Err(error);
        }
    } else {
        match target {
            Target::Cli => {
                let terminal = if let Some(name) = launch.terminal {
                    terminal(&name)?
                } else {
                    preferences.cli_terminal
                };
                service.launch_cli_profile_with_terminal(
                    &account.metadata.id,
                    &workspace,
                    terminal,
                )?;
            }
            Target::Vscode => service.launch_profile(&account.metadata.id, &workspace)?,
            Target::Codex => service.launch_standalone_profile(&account.metadata.id, &workspace)?,
        }
    }
    Ok(
        json!({"account":account.metadata.id,"target":format!("{target:?}").to_lowercase(),"workspace":workspace,"started":true,"identityVerified":false}),
    )
}
fn usage_rows(service: &Service, selection: &Selection, credits: bool) -> Result<(Value, bool)> {
    let mut failed = false;
    let mut rows = Vec::new();
    let root = settings::data_root()?.join("usage-cache");
    multi_codex_core::managed_files::directory(&settings::data_root()?, &root)?;
    for account in selected(service, selection)? {
        let path = root.join(format!("{}.json", account.metadata.id));
        match service.check_profile_limits(&account.metadata.id) {
            Ok(limits) => {
                let data = value(limits)?;
                profiles::write_private_file(&path, &serde_json::to_vec(&data)?)?;
                rows.push(json!({"id":account.metadata.id,"name":account.metadata.name,"ok":true,"stale":false,"usage":if credits{json!({"count":data["resetCreditsAvailable"],"credits":data["resetCredits"],"checkedAt":data["checkedAt"]})}else{data}}));
            }
            Err(error) => {
                failed = true;
                let cache =
                    multi_codex_core::managed_files::checked(&settings::data_root()?, &path, false)
                        .ok()
                        .and_then(|p| std::fs::read(p).ok())
                        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok());
                rows.push(json!({"id":account.metadata.id,"name":account.metadata.name,"ok":false,"stale":cache.is_some(),"usage":cache,"error":error}));
            }
        }
    }
    Ok((value(rows)?, failed))
}
fn bulk(
    accounts: &[ProfileView],
    mut operation: impl FnMut(&ProfileView) -> Result<Value>,
) -> Result<Value> {
    let mut failed = false;
    let mut rows = Vec::new();
    for account in accounts {
        match operation(account) {
            Ok(data) => rows.push(json!({"id":account.metadata.id,"ok":true,"data":data})),
            Err(error) => {
                failed = true;
                rows.push(json!({"id":account.metadata.id,"ok":false,"error":error.message}));
            }
        }
    }
    let rows = value(rows)?;
    if failed {
        let mut error = CliError::new(
            1,
            "Some selected account operations failed; inspect individual results.",
        );
        error.details = Some(rows);
        Err(error)
    } else {
        Ok(rows)
    }
}
fn scope(selection: &ChatSelection) -> Result<ChatScope> {
    if selection.all {
        Ok(ChatScope::All)
    } else if selection.older_than.as_deref() == Some("30d") {
        Ok(ChatScope::OlderThan30Days)
    } else {
        Err(CliError::new(
            2,
            "Choose --all or --older-than 30d explicitly.",
        ))
    }
}
fn chat_command(service: &Service, command: Chats) -> Result<Value> {
    let (selection, clear, token) = match command {
        Chats::Preview(s) => (s, false, None),
        Chats::Clear {
            selection,
            preview_token,
        } => (selection, true, preview_token),
    };
    let scope = scope(&selection)?;
    if clear {
        confirm(
            selection.cleanup.yes,
            "Delete the previewed local chats? Shared scope affects every account.",
        )?;
    }
    if selection.shared {
        if clear {
            let token = match token {
                Some(token) => token,
                None => service.preview_shared_chats(scope)?.token,
            };
            value(service.clear_shared_chats(&token)?)
        } else {
            value(service.preview_shared_chats(scope)?)
        }
    } else {
        let accounts = cleanup_accounts(service, &selection.cleanup)?;
        if token.is_some() && accounts.len() != 1 {
            return Err(CliError::new(
                2,
                "A preview token requires exactly one account.",
            ));
        }
        bulk(&accounts, |account| {
            if clear {
                let token = match &token {
                    Some(token) => token.clone(),
                    None => {
                        service
                            .preview_profile_chats(&account.metadata.id, scope)?
                            .token
                    }
                };
                value(service.clear_profile_chats(&account.metadata.id, &token)?)
            } else {
                value(service.preview_profile_chats(&account.metadata.id, scope)?)
            }
        })
    }
}
fn migration(service: &Service, command: Migration) -> Result<Value> {
    match command {
        Migration::Queue { process } => {
            if process {
                service.set_managed_bridge(bridge()?)?;
                value(service.process_shared_storage_queue()?)
            } else {
                value(service.shared_storage_queue()?)
            }
        }
        Migration::Enable { yes } => {
            confirm(
                yes,
                "Enable shared storage and queue migrations after verified backups?",
            )?;
            service.set_managed_bridge(bridge()?)?;
            value(service.set_shared_storage_enabled(true)?)
        }
        Migration::Preview { account } => {
            value(service.preview_profile_migration(&resolve(service, &account)?.metadata.id)?)
        }
        Migration::Prepare { account } => {
            value(service.prepare_profile_migration(&resolve(service, &account)?.metadata.id)?)
        }
        Migration::Apply { account, yes } => {
            confirm(yes, "Migrate this account after a verified backup?")?;
            value(service.migrate_profile_shared_storage(
                &resolve(service, &account)?.metadata.id,
                &bridge()?,
            )?)
        }
        Migration::Rollback { account, yes } => {
            confirm(
                yes,
                "Roll back this account's layout while retaining shared data?",
            )?;
            value(
                service
                    .rollback_profile_shared_storage(&resolve(service, &account)?.metadata.id)?,
            )
        }
        Migration::Restore { account, yes } => {
            confirm(yes,"Restore verified profile data, preserving current credentials and displaced newer work?")?;
            value(service.restore_profile_backup(&resolve(service, &account)?.metadata.id)?)
        }
    }
}
fn backups(service: &Service, command: Backups) -> Result<Value> {
    let store = multi_codex_core::migration::MigrationStore::open(&settings::data_root()?)?;
    match command {
        Backups::List => {
            let mut records = Vec::new();
            for account in service.list_profiles()? {
                if let Some(record) = store.record(&account.metadata.id)? {
                    records.push(value(record)?);
                }
            }
            value(records)
        }
        Backups::Verify { account } => {
            let id = resolve(service, &account)?.metadata.id;
            let record = store
                .record(&id)?
                .ok_or_else(|| CliError::new(2, "No managed backup for this account."))?;
            store.verify(&record)?;
            Ok(json!({"verified":true,"snapshot":record}))
        }
        Backups::Restore { account, yes } => {
            migration(service, Migration::Restore { account, yes })
        }
    }
}
fn extensions(service: &Service, command: Extensions) -> Result<Value> {
    match command {
        Extensions::Prepare => value(service.prepare_shared_extensions(&bridge()?)?),
        Extensions::PreviewLegacy { account } => {
            value(service.preview_legacy_extensions(&resolve(service, &account)?.metadata.id)?)
        }
        Extensions::ClearLegacy {
            account,
            token,
            yes,
        } => {
            confirm(yes, "Remove previewed retained legacy extension copies?")?;
            value(
                service
                    .clear_legacy_extensions(&resolve(service, &account)?.metadata.id, &token)?,
            )
        }
        Extensions::PreviewGenerations => value(service.preview_shared_generations()?),
        Extensions::ClearGenerations { token, yes } => {
            confirm(yes, "Remove only verified unused extension generations?")?;
            let preview = service.preview_shared_generations()?;
            let actual = value(&preview)?;
            if actual["digest"] != token {
                return Err(CliError::new(
                    4,
                    "Extension preview changed. Preview again.",
                ));
            }
            value(service.clear_shared_generations(preview)?)
        }
    }
}
fn terminal(name: &str) -> Result<terminals::Terminal> {
    // Accept the same serialized IDs as the desktop settings.
    let normalized = match name {
        "gnome-terminal" => "gnomeTerminal",
        "xfce-terminal" => "xfceTerminal",
        other => other,
    };
    serde_json::from_value(json!(normalized))
        .map_err(|_| CliError::new(2, "Unknown terminal. Use terminals list for supported IDs."))
}
fn setting_key(key: &str) -> Result<&str> {
    Ok(match key {
        "workspace" => "preferredWorkspace",
        "color-theme" => "colorTheme",
        "terminal" => "cliTerminal",
        "launch-target" => "launchMode",
        "code-path" => "codePath",
        "codex-path" => "codexPath",
        "global-codex-home" => "globalCodexHome",
        "hide-desktop-picker" => "hideDesktopPicker",
        "preferredWorkspace" | "cliTerminal" | "launchMode" | "codePath" | "codexPath"
        | "globalCodexHome" | "hideDesktopPicker" | "colorTheme" => key,
        _ => {
            return Err(CliError::new(
                2,
                "Unknown setting. Use settings get to inspect supported settings.",
            ))
        }
    })
}
fn settings_command(command: Settings) -> Result<Value> {
    let mut stored = value(settings::load()?)?;
    match command {
        Settings::Get { key } => {
            if let Some(key) = key {
                Ok(stored[setting_key(&key)?].clone())
            } else {
                Ok(stored)
            }
        }
        Settings::Set { key, value: input } => {
            let key = setting_key(&key)?;
            stored[key] = match key {
                "cliTerminal" => value(terminal(&input)?)?,
                "hideDesktopPicker" => json!(input
                    .parse::<bool>()
                    .map_err(|_| CliError::new(2, "Use true or false."))?),
                "launchMode" => json!(match input.as_str() {
                    "codex" => "standalone",
                    other => other,
                }),
                _ => json!(input),
            };
            value(settings::save(serde_json::from_value(stored)?)?)
        }
        Settings::Reset { key, yes } => {
            confirm(yes, "Reset the selected shared launcher settings?")?;
            let defaults = value(settings::ExecutableSettings::default())?;
            if let Some(key) = key {
                let key = setting_key(&key)?;
                stored[key] = defaults[key].clone();
            } else {
                stored = defaults;
            }
            value(settings::save(serde_json::from_value(stored)?)?)
        }
    }
}
fn integration(command: Integration) -> Result<Value> {
    let integration = multi_codex_core::desktop_integration::DesktopIntegration::discover()?;
    match command {
        Integration::Status => value(integration.status()),
        Integration::Install { desktop_shortcut } => value(integration.install(desktop_shortcut)?),
        Integration::Remove { yes } => {
            confirm(
                yes,
                "Remove only Multi Codex desktop shortcuts? Account data and executables remain.",
            )?;
            value(integration.remove_shortcuts()?)
        }
    }
}
fn doctor(service: &Service) -> Result<Value> {
    let id = uuid::Uuid::new_v4().to_string();
    let store = KeyringSecretStore;
    let keyring = match store.set(&id, "multi-codex-doctor-inert") {
        Ok(()) => {
            let read = store
                .get(&id)
                .is_ok_and(|v| v == "multi-codex-doctor-inert");
            let removed = store.delete(&id).is_ok();
            json!({"ok":read&&removed,"cleanupComplete":removed})
        }
        Err(_) => {
            json!({"ok":false,"reason":"Native credential store unavailable; unlock or configure the OS keyring."})
        }
    };
    Ok(
        json!({"version":env!("CARGO_PKG_VERSION"),"os":std::env::consts::OS,"arch":std::env::consts::ARCH,
        "engineAvailable":profiles::resolve_codex_command().is_ok(),"terminals":terminals::options(),"clients":multi_codex_core::launch_targets::detect(),
        "keyring":keyring,"accountCount":service.list_profiles()?.len(),"desktopCapabilities":multi_codex_core::desktop_environment::capabilities(),"bridgeAvailable":bridge().is_ok(),"updates":"npm"}),
    )
}
fn update(check: bool) -> Result<Value> {
    let output = multi_codex_core::process::output(
        std::process::Command::new("npm").args([
            "view",
            "@russeldp/multi-codex",
            "version",
            "--json",
            "--ignore-scripts",
        ]),
        Duration::from_secs(30),
    )?;
    if !output.status.success() {
        return Err(CliError::new(
            5,
            "Could not query the npm registry. Check your connection and npm registry settings.",
        ));
    }
    let latest = registry_version(&output.stdout)?;
    Ok(
        json!({"currentVersion":env!("CARGO_PKG_VERSION"),"latestVersion":latest,"updateAvailable":newer_version(&latest, env!("CARGO_PKG_VERSION"))?,"checkOnly":check,"action":"npm install --global @russeldp/multi-codex@latest","automaticallyInstalled":false}),
    )
}

fn registry_version(output: &[u8]) -> Result<String> {
    let value: Value = serde_json::from_slice(output)
        .map_err(|_| CliError::new(5, "npm returned an invalid package version."))?;
    let version = match &value {
        Value::String(version) => Some(version.as_str()),
        Value::Array(versions) if versions.len() == 1 => versions[0].as_str(),
        _ => None,
    }
    .ok_or_else(|| CliError::new(5, "npm returned an invalid package version."))?;
    newer_version(version, env!("CARGO_PKG_VERSION"))?;
    Ok(version.to_owned())
}

fn newer_version(latest: &str, current: &str) -> Result<bool> {
    fn parse(version: &str) -> Option<(u64, u64, u64)> {
        let mut parts = version.split('-').next()?.split('.');
        let tuple = (
            parts.next()?.parse().ok()?,
            parts.next()?.parse().ok()?,
            parts.next()?.parse().ok()?,
        );
        if parts.next().is_some() {
            None
        } else {
            Some(tuple)
        }
    }
    let latest =
        parse(latest).ok_or_else(|| CliError::new(5, "Registry package version is invalid."))?;
    let current =
        parse(current).ok_or_else(|| CliError::new(1, "Installed version is invalid."))?;
    Ok(latest > current)
}
#[cfg(test)]
mod update_tests {
    #[test]
    fn npm_version_accepts_string_and_single_result_array() {
        for bytes in [br#""1.0.3""#.as_slice(), br#"["1.0.3"]"#.as_slice()] {
            assert_eq!(super::registry_version(bytes).unwrap(), "1.0.3");
        }
        for bytes in [
            br#"[]"#.as_slice(),
            br#"["1.0.2","1.0.3"]"#.as_slice(),
            br#"null"#.as_slice(),
            br#""invalid""#.as_slice(),
        ] {
            assert!(super::registry_version(bytes).is_err());
        }
    }

    #[test]
    fn older_registry_version_is_not_an_upgrade() {
        assert!(!super::newer_version("1.0.0", "1.0.1").unwrap());
        assert!(!super::newer_version("1.0.0", "1.0.0").unwrap());
        assert!(super::newer_version("1.1.0", "1.0.0").unwrap());
        assert!(super::newer_version("unparseable", "1.0.0").is_err());
    }
}

#[cfg(test)]
mod managed_home_tests {
    #[test]
    fn only_managed_profile_homes_are_ignored_as_global_import_sources() {
        let directory = tempfile::tempdir().unwrap();
        let managed = directory
            .path()
            .join("profiles")
            .join(uuid::Uuid::new_v4().to_string())
            .join("codex-home");
        assert!(super::managed_codex_home(directory.path(), &managed));
        for layout in [
            "bridge-runtime",
            "vscode-user-data/User/globalStorage/russeldanielpaul.multi-codex/runtime",
        ] {
            let runtime = managed
                .parent()
                .unwrap()
                .join(layout)
                .join("windows")
                .join(uuid::Uuid::new_v4().to_string());
            assert!(super::managed_codex_home(directory.path(), &runtime));
            assert!(!super::managed_codex_home(
                directory.path(),
                &runtime.parent().unwrap().join("unknown")
            ));
        }
        assert!(!super::managed_codex_home(
            directory.path(),
            &directory.path().join("profiles/unknown/codex-home")
        ));
        assert!(!super::managed_codex_home(
            directory.path(),
            &directory.path().join("global")
        ));
    }
}

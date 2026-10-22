//! Shared native account services. Basic CLI operations have no webview dependency.
#[cfg(all(target_os = "linux", feature = "desktop-icons"))]
pub mod application_icons;
pub mod chat_cleanup;
pub mod desktop_environment;
#[cfg(unix)]
pub mod desktop_integration;
#[cfg(windows)]
#[path = "windows_integration.rs"]
pub mod desktop_integration;
pub mod extension_accounts;
pub mod file_security;
pub mod launch;
pub mod launch_targets;
pub mod legacy_cleanup;
#[cfg(target_os = "linux")]
pub mod linux_desktops;
pub mod managed_files;
pub mod managed_launch;
pub mod managed_settings;
pub mod migration;
pub mod migration_dependencies;
pub mod migration_runtime;
pub mod process;
pub mod profile_layout;
pub mod profile_restore;
pub mod profiles;
pub mod settings;
pub mod shared_chat_cleanup;
pub mod shared_chat_import;
pub mod shared_chat_leases;
pub mod shared_chats;
pub mod shared_storage_queue;
pub mod shared_vscode;
pub mod terminals;
pub mod usage;
#[cfg(windows)]
pub mod windows_platform;

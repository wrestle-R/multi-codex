use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "multi-codex",
    about = "Manage isolated Codex accounts from macOS and Linux",
    disable_version_flag = true
)]
pub struct Cli {
    #[arg(long, global = true)]
    pub json: bool,
    #[arg(long, global = true)]
    pub no_color: bool,
    #[arg(long, global = true)]
    pub quiet: bool,
    #[arg(long, global = true)]
    pub data_dir: Option<PathBuf>,
    #[arg(long, global = true)]
    pub global_codex_home: Option<PathBuf>,
    #[arg(short = 'V', long, global = true)]
    pub version: bool,
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Add, reconnect, inspect, edit, or remove saved accounts
    Accounts {
        #[command(subcommand)]
        command: Accounts,
    },
    /// Open a new isolated terminal or application
    Launch(Launch),
    /// Refresh account quota windows; --watch emits successive results
    Usage(Usage),
    /// Inspect reset credits and their actual expiry
    Credits(Selection),
    /// Inspect running clients and account errors
    Status(Selection),
    /// Inspect and manage local storage with explicit deletion scopes
    Storage {
        #[command(subcommand)]
        command: Option<Storage>,
        #[arg(long)]
        account: Option<String>,
    },
    /// List supported terminals and whether they are installed
    Terminals {
        #[command(subcommand)]
        command: List,
    },
    /// Inspect verified desktop-placement capabilities
    Desktops {
        #[command(subcommand)]
        command: List,
    },
    /// Inspect and configure shared launcher preferences
    Settings {
        #[command(subcommand)]
        command: Settings,
    },
    /// Inspect, install or remove owned desktop shortcuts
    Integration {
        #[command(subcommand)]
        command: Integration,
    },
    /// Check the npm version and show the package-manager upgrade command
    Update {
        #[arg(long)]
        check: bool,
    },
    /// Diagnose capabilities without exposing credentials or conversations
    Doctor,
    /// Print CLI version and platform
    Version,
}

#[derive(Subcommand, Debug)]
pub enum List {
    List,
}

#[derive(Subcommand, Debug)]
pub enum Accounts {
    List,
    Show {
        account: String,
    },
    Add {
        #[arg(long)]
        name: String,
        #[arg(long)]
        notes: Option<String>,
        #[arg(long)]
        no_browser: bool,
    },
    Import {
        #[arg(long)]
        name: String,
        #[arg(long)]
        notes: Option<String>,
        #[arg(long, conflicts_with = "stdin")]
        file: Option<PathBuf>,
        #[arg(long)]
        stdin: bool,
    },
    ImportCurrent {
        #[arg(long)]
        name: String,
        #[arg(long)]
        notes: Option<String>,
    },
    Edit {
        account: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        notes: Option<String>,
        #[arg(long, conflicts_with = "notes")]
        clear_notes: bool,
    },
    Reconnect {
        account: String,
        #[arg(long)]
        no_browser: bool,
    },
    Remove {
        account: String,
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Args, Debug, Default)]
pub struct Selection {
    #[arg(long, conflicts_with = "all")]
    pub account: Option<String>,
    #[arg(long)]
    pub all: bool,
}

#[derive(Args, Debug)]
pub struct Usage {
    #[command(flatten)]
    pub selection: Selection,
    #[arg(long)]
    pub watch: bool,
    #[arg(long, default_value_t = 60, value_parser = clap::value_parser!(u64).range(5..=3600))]
    pub interval: u64,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum Target {
    Cli,
    Vscode,
    Codex,
}

#[derive(Args, Debug)]
pub struct Launch {
    #[arg(long)]
    pub account: String,
    #[arg(long)]
    pub target: Option<Target>,
    #[arg(long, default_value = ".")]
    pub workspace: PathBuf,
    #[arg(long)]
    pub terminal: Option<String>,
    #[arg(long)]
    pub desktop: Option<String>,
}

#[derive(Subcommand, Debug)]
pub enum Storage {
    Cache {
        #[command(subcommand)]
        command: Cache,
    },
    Chats {
        #[command(subcommand)]
        command: Chats,
    },
    Migration {
        #[command(subcommand)]
        command: Migration,
    },
    Backups {
        #[command(subcommand)]
        command: Backups,
    },
    Extensions {
        #[command(subcommand)]
        command: Extensions,
    },
}
#[derive(Subcommand, Debug)]
pub enum Cache {
    Clear(Cleanup),
}
#[derive(Args, Debug)]
pub struct Cleanup {
    #[arg(long, conflicts_with = "all_accounts")]
    pub account: Option<String>,
    #[arg(long)]
    pub all_accounts: bool,
    #[arg(long)]
    pub yes: bool,
}
#[derive(Args, Debug)]
pub struct ChatSelection {
    #[command(flatten)]
    pub cleanup: Cleanup,
    #[arg(long, conflicts_with = "older_than")]
    pub all: bool,
    #[arg(long, value_parser = ["30d"])]
    pub older_than: Option<String>,
    /// Shared library deletion affects every saved account
    #[arg(long, conflicts_with_all = ["account", "all_accounts"])]
    pub shared: bool,
}
#[derive(Subcommand, Debug)]
pub enum Chats {
    Preview(ChatSelection),
    Clear {
        #[command(flatten)]
        selection: ChatSelection,
        #[arg(long)]
        preview_token: Option<String>,
    },
}
#[derive(Subcommand, Debug)]
pub enum Migration {
    Preview {
        #[arg(long)]
        account: String,
    },
    Prepare {
        #[arg(long)]
        account: String,
    },
    Apply {
        #[arg(long)]
        account: String,
        #[arg(long)]
        yes: bool,
    },
    Rollback {
        #[arg(long)]
        account: String,
        #[arg(long)]
        yes: bool,
    },
    Restore {
        #[arg(long)]
        account: String,
        #[arg(long)]
        yes: bool,
    },
    Enable {
        #[arg(long)]
        yes: bool,
    },
    Queue,
}
#[derive(Subcommand, Debug)]
pub enum Extensions {
    Prepare,
    PreviewLegacy {
        #[arg(long)]
        account: String,
    },
    ClearLegacy {
        #[arg(long)]
        account: String,
        #[arg(long)]
        token: String,
        #[arg(long)]
        yes: bool,
    },
    PreviewGenerations,
    ClearGenerations {
        #[arg(long)]
        token: String,
        #[arg(long)]
        yes: bool,
    },
}
#[derive(Subcommand, Debug)]
pub enum Settings {
    Get {
        key: Option<String>,
    },
    Set {
        key: String,
        value: String,
    },
    Reset {
        key: Option<String>,
        #[arg(long)]
        yes: bool,
    },
}
#[derive(Subcommand, Debug)]
pub enum Integration {
    Status,
    Install {
        #[arg(long)]
        desktop_shortcut: bool,
    },
    Remove {
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum Backups {
    List,
    Verify {
        #[arg(long)]
        account: String,
    },
    Restore {
        #[arg(long)]
        account: String,
        #[arg(long)]
        yes: bool,
    },
}

#[cfg(test)]
mod tests {
    use super::Cli;
    use clap::{CommandFactory, Parser};
    #[test]
    fn command_tree_has_no_conflicting_arguments() {
        Cli::command().debug_assert();
    }
    #[test]
    fn shared_cleanup_refuses_an_account_scope() {
        assert!(Cli::try_parse_from([
            "multi-codex",
            "storage",
            "chats",
            "clear",
            "--shared",
            "--account",
            "Work",
            "--all",
            "--yes"
        ])
        .is_err());
    }
}

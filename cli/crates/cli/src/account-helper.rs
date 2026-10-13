fn main() {
    if multi_codex_core::extension_accounts::run_bridge_launcher().is_err() {
        std::process::exit(1);
    }
    if multi_codex_core::extension_accounts::run_account_helper().is_err() {
        eprintln!("Multi Codex account helper could not start.");
        std::process::exit(1);
    }
}

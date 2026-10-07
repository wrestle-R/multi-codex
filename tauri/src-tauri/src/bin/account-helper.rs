fn main() {
    if multi_codex_lib::run_account_helper().is_err() {
        eprintln!("Multi Codex account helper could not start.");
        std::process::exit(1);
    }
}

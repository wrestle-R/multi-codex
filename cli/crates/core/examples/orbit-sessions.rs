//! Read-only Orbit diagnostics; never emits account names, paths or chat text.
fn main() {
    let root = std::env::args_os()
        .nth(1)
        .map(std::path::PathBuf::from)
        .expect("account data directory required");
    let summarize = |sessions: Vec<multi_codex_core::hud::Snapshot>| {
        sessions.into_iter().map(|s| serde_json::json!({"pid":s.pid,"client":s.client,"sessionCount":s.sessions.len(),"states":s.sessions.values().map(|s| &s.status).collect::<Vec<_>>()})).collect::<Vec<_>>()
    };
    println!(
        "{}",
        serde_json::json!({"native":summarize(multi_codex_core::hud_sessions::snapshots(&root)),"combined":summarize(multi_codex_core::hud::live_snapshots(&root))})
    );
}

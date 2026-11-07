//! Disposable native T3 smoke fixture. Never reads real account state.
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
fn main() {
    if std::env::args().nth(1).as_deref() == Some("--version") {
        println!("codex-cli 0.115.0");
        return;
    }
    if std::env::args().nth(1).as_deref() == Some("app-server") {
        std::process::exit(1);
    }

    let binary = PathBuf::from(std::env::args_os().nth(1).expect("T3 executable"));
    let root = PathBuf::from(std::env::args_os().nth(2).expect("owned fixture root"));
    std::fs::create_dir_all(&root).unwrap();
    let root = root.canonicalize().unwrap();
    let mut children = Vec::new();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        for account in ["one", "two"] {
            let home = root.join(account);
            let codex_home = root.join(format!("codex-{account}"));
            std::fs::create_dir_all(&codex_home).unwrap();
            multi_codex_core::t3::prepare(
                &root,
                &home,
                &codex_home,
                &std::env::current_exe().unwrap(),
            )
            .unwrap();
            // The fixture never starts an agent or provides a real credential.
            let child = multi_codex_core::t3::command(&binary, &home, &codex_home, &root)
                .spawn()
                .unwrap();
            println!("fixture_pid={}", child.id());
            children.push(child);
        }
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            assert!(
                children.iter_mut().all(|c| c.try_wait().unwrap().is_none()),
                "T3 exited or forwarded to another account"
            );
            let ready = ["one", "two"]
                .iter()
                .all(|id| root.join(id).join("config/t3code-v2/Local State").exists());
            if ready {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "Both isolated Electron profiles must start"
            );
            std::thread::sleep(Duration::from_millis(500));
        }
        std::thread::sleep(Duration::from_secs(5));
        assert!(children.iter_mut().all(|c| c.try_wait().unwrap().is_none()));
        println!("Two T3 desktop instances started with distinct Electron and Codex homes.");
    }));
    for mut child in children {
        #[cfg(windows)]
        {
            let _ = std::process::Command::new("taskkill")
                .args(["/F", "/T", "/PID", &child.id().to_string()])
                .output();
        }
        let _ = child.kill();
        let _ = child.wait();
    }
    if let Err(error) = result {
        std::panic::resume_unwind(error);
    }
}

mod app;
mod args;
mod error;
mod login;

use clap::{CommandFactory, Parser};
use serde_json::json;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

fn main() {
    let json_output = std::env::args_os().any(|arg| arg == "--json");
    let mut cli = match args::Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => {
            let code = error.exit_code();
            if json_output && code != 0 {
                println!(
                    "{}",
                    json!({"schemaVersion":1,"ok":false,"error":{"code":code,"message":error.to_string()}})
                );
            } else {
                let _ = error.print();
            }
            std::process::exit(code);
        }
    };
    if cli.version || matches!(cli.command, Some(args::Command::Version)) {
        app::emit(
            cli.json,
            &json!({"version":env!("CARGO_PKG_VERSION"),"os":std::env::consts::OS,"arch":std::env::consts::ARCH}),
        );
        return;
    }
    if cli.command.is_none() && cli.json {
        cli.command = Some(args::Command::Accounts {
            command: args::Accounts::List,
        });
    }
    if cli.command.is_none() && !app::interactive() {
        let mut help = args::Cli::command();
        let _ = help.print_help();
        println!();
        return;
    }
    let stop = Arc::new(AtomicBool::new(false));
    let signal = stop.clone();
    if ctrlc::set_handler(move || {
        signal.store(true, Ordering::SeqCst);
    })
    .is_err()
    {
        eprintln!("Could not initialize cancellation handling.");
        std::process::exit(3);
    }
    let result = app::run(&mut cli, &stop);
    if let Err(error) = result {
        if cli.json {
            println!(
                "{}",
                json!({"schemaVersion":1,"ok":false,"error":{"code":error.code,"message":error.message,"details":error.details}})
            );
        } else {
            eprintln!("{}", error.message);
            if let Some(details) = error.details {
                eprintln!(
                    "{}",
                    serde_json::to_string_pretty(&details).unwrap_or_default()
                );
            }
        }
        std::process::exit(error.code);
    }
}

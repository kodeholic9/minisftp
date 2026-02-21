// miniSFTP CLI
// author: kodeholic (powered by Claude)
//
// Usage: minisftp user@host [port]

use std::env;
use tracing_subscriber::{EnvFilter, fmt};

mod commands;
mod handler;

#[tokio::main]
async fn main() {
    // RUST_LOG=debug cargo run -p minisftp-cli -- user@host
    // RUST_LOG=trace cargo run -p minisftp-cli -- user@host  (hex dump 포함)
    fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info"))
        )
        .with_target(false)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .init();

    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage();
        return;
    }

    // user@host 파싱
    let target = &args[1];
    let port = if args.len() >= 3 {
        args[2].parse::<u16>().unwrap_or(22)
    } else {
        22
    };

    let (username, host) = match target.split_once('@') {
        Some((u, h)) => (u.to_string(), h.to_string()),
        None => {
            println!("Format: minisftp user@host [port]");
            return;
        }
    };

    let password = read_password(&username, &host);

    if let Err(e) = handler::run(host, port, username, password).await {
        println!("Error: {}", e);
    }
}

fn print_usage() {
    println!("miniSFTP - SSH File Transfer Client");
    println!();
    println!("Usage: minisftp user@host [port]");
    println!();
    println!("Example:");
    println!("  minisftp admin@192.168.1.100");
    println!("  minisftp admin@192.168.1.100 2222");
}

fn read_password(username: &str, host: &str) -> String {
    eprint!("{}@{}'s password: ", username, host);
    let mut password = String::new();
    std::io::stdin().read_line(&mut password).unwrap();
    password.trim().to_string()
}

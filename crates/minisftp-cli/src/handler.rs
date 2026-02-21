// miniSFTP CLI Handler
// author: kodeholic (powered by Claude)
//
// core 호출 + interactive shell
// Tauri 전환 시 이 로직을 거의 그대로 가져갈 수 있음

use std::io::{self, Write};

use minisftp_core::config::{AuthMethod, ConnectConfig};
use minisftp_core::sftp::{CancellationToken, ProgressInfo, TransferResult};
use minisftp_core::state::{ConnectionObserver, ConnectionState};
use minisftp_core::session::SftpSession;
use minisftp_core::utils::{fmt_size, local_ls, print_progress, resolve_path, resolve_local_path};

use crate::commands::Command;

struct CliObserver;

impl ConnectionObserver for CliObserver {
    fn on_state_changed(&self, _prev: &ConnectionState, next: &ConnectionState) {
        println!("[state] → {:?}", next);
    }
}

pub async fn run(
    host: String,
    port: u16,
    username: String,
    password: String,
) -> minisftp_core::error::Result<()> {
    let config = ConnectConfig { host, port, username, auth: AuthMethod::Password(password) };

    println!("Connecting to {}:{}...", config.host, config.port);

    let mut session = SftpSession::new(Box::new(CliObserver));
    let mut sftp    = session.connect(&config).await?;

    // 방식 A: 접속 직후 서버에 실제 홈 디렉토리 확인
    let mut remote_dir = sftp.realpath(".").await
        .unwrap_or_else(|_| ".".to_string());

    println!("Connected. Remote: {}  Type 'help' for commands.", remote_dir);
    let mut local_dir  = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| ".".to_string());

    loop {
        print!("sftp> ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();

        match Command::parse(&input) {
            // ── 리모트 명령 ──────────────────────────────────────
            Command::Ls { path } => {
                let target = resolve_path(&remote_dir, &path);
                match sftp.ls(&target).await {
                    Ok(entries) => {
                        for e in &entries {
                            println!("{:10}  {:>10}  {:>5}  {:>5}  {}  {}",
                                e.permission_str(),
                                e.size,
                                e.uid.map(|u| u.to_string()).unwrap_or_else(|| "?".to_string()),
                                e.gid.map(|g| g.to_string()).unwrap_or_else(|| "?".to_string()),
                                e.mtime_str(),
                                e.name,
                            );
                        }
                        println!("Total: {} entries", entries.len());
                    }
                    Err(e) => println!("Error: {}", e),
                }
            }
            Command::Cd { path } => {
                remote_dir = resolve_path(&remote_dir, &path);
                println!("Remote: {}", remote_dir);
            }
            Command::Pwd => {
                // 방식 B: 서버에 직접 확인 (심볼릭 링크 해소 등)
                match sftp.pwd(&remote_dir).await {
                    Ok(path) => println!("Remote: {}", path),
                    Err(e)   => println!("Error: {}", e),
                }
            }
            Command::Get { remote, local } => {
                let remote_path = resolve_path(&remote_dir, &remote);
                let local_path  = resolve_local_path(&local_dir, &local);
                let token       = CancellationToken::new();
                // ^C 태스크: 신호 수신 시 token.cancel()만 호출
                // get()은 다음 청크에서 Cancelled(n)을 반환하고 정상 종료
                let token_clone = token.clone();
                tokio::spawn(async move {
                    let _ = tokio::signal::ctrl_c().await;
                    token_clone.cancel();
                });
                match sftp.get(&remote_path, &local_path,
                    |p: ProgressInfo| print_progress(p.transferred, p.total, p.elapsed_secs),
                    token,
                ).await {
                    Ok(TransferResult::Skipped)      => println!("Skipped (identical): {}", remote_path),
                    Ok(TransferResult::Resumed(n))   => { println!(); println!("Resumed: {} → {} ({} total)", remote_path, local_path, fmt_size(n)); }
                    Ok(TransferResult::Completed(n)) => { println!(); println!("Downloaded: {} → {} ({})", remote_path, local_path, fmt_size(n)); }
                    Ok(TransferResult::Cancelled(n)) => { println!(); println!("Cancelled. ({} transferred)", fmt_size(n)); }
                    Err(e) => { println!(); println!("Error: {}", e); }
                }
            }
            Command::Put { local, remote } => {
                let local_path  = resolve_local_path(&local_dir, &local);
                let remote_path = resolve_path(&remote_dir, &remote);
                let token       = CancellationToken::new();
                let token_clone = token.clone();
                tokio::spawn(async move {
                    let _ = tokio::signal::ctrl_c().await;
                    token_clone.cancel();
                });
                match sftp.put(&local_path, &remote_path,
                    |p: ProgressInfo| print_progress(p.transferred, p.total, p.elapsed_secs),
                    token,
                ).await {
                    Ok(TransferResult::Skipped)      => println!("Skipped (identical): {}", local_path),
                    Ok(TransferResult::Resumed(n))   => { println!(); println!("Resumed: {} → {} ({} total)", local_path, remote_path, fmt_size(n)); }
                    Ok(TransferResult::Completed(n)) => { println!(); println!("Uploaded: {} → {} ({})", local_path, remote_path, fmt_size(n)); }
                    Ok(TransferResult::Cancelled(n)) => { println!(); println!("Cancelled. ({} transferred)", fmt_size(n)); }
                    Err(e) => { println!(); println!("Error: {}", e); }
                }
            }
            Command::Mkdir { path } => {
                let target = resolve_path(&remote_dir, &path);
                match sftp.mkdir(&target).await {
                    Ok(()) => println!("Created: {}", target),
                    Err(e) => println!("Error: {}", e),
                }
            }
            Command::Rm { path } => {
                let target = resolve_path(&remote_dir, &path);
                match sftp.rm(&target).await {
                    Ok(()) => println!("Removed: {}", target),
                    Err(e) => println!("Error: {}", e),
                }
            }

            // ── 로컬 명령 (!ls, !cd, !pwd) ───────────────────────
            Command::LocalLs { path } => {
                let target = resolve_local_path(&local_dir, &path);
                match local_ls(&target) {
                    Ok(entries) => {
                        print_local_ls(&entries);
                        println!("Total: {} entries", entries.len());
                    }
                    Err(e) => println!("Error: {}", e),
                }
            }
            Command::LocalCd { path } => {
                let next = resolve_local_path(&local_dir, &path);
                match std::fs::metadata(&next) {
                    Ok(m) if m.is_dir() => {
                        local_dir = next;
                        println!("Local: {}", local_dir);
                    }
                    Ok(_) => println!("Not a directory: {}", next),
                    Err(e) => println!("Error: {}", e),
                }
            }
            Command::LocalPwd => println!("Local: {}", local_dir),

            // ── 기타 ─────────────────────────────────────────────
            Command::Help => {
                println!("Remote commands:");
                println!("  ls [path]             List remote directory");
                println!("  get <remote> [local]  Download file");
                println!("  put <local> [remote]  Upload file");
                println!("  mkdir <path>          Create remote directory");
                println!("  rm <path>             Remove remote file");
                println!("  cd <path>             Change remote directory");
                println!("  pwd                   Show remote directory");
                println!();
                println!("Local commands:");
                println!("  !ls [path]            List local directory");
                println!("  !cd <path>            Change local directory");
                println!("  !pwd                  Show local directory");
                println!();
                println!("  help                  Show this help");
                println!("  quit                  Exit");
            }
            Command::Quit => {
                println!("Goodbye.");
                break;
            }
            Command::Unknown(msg) => {
                if !msg.is_empty() { println!("{}", msg); }
            }
        }
    }
    Ok(())
}

/// 로컈 ls 출력: 플랫폼별 포맷
///
/// Windows: dir 스타일 (2026-02-21  오후 01:35    <DIR>  .cargo)
/// Unix:    ls -la 스타일 (drwxr-xr-x  4096  1000  1000  Jun 10  .config)
fn print_local_ls(entries: &[minisftp_core::sftp::FileEntry]) {
    #[cfg(windows)]
    {
        use chrono::{Local, LocalResult, TimeZone, Utc};
        for e in entries {
            let dt_str = e.mtime
                .and_then(|ts| match Utc.timestamp_opt(ts as i64, 0) {
                    LocalResult::Single(utc) => {
                        let local = utc.with_timezone(&Local);
                        // 오전/오후는 한국어 로켈 대신 영어로 고정
                        let ampm = if local.format("%p").to_string() == "AM" { "오전" } else { "오후" };
                        Some(format!("{} {}  {}",
                            local.format("%Y-%m-%d"),
                            ampm,
                            local.format("%I:%M"),
                        ))
                    }
                    _ => None,
                })
                .unwrap_or_else(|| "                      ".to_string());

            if e.is_dir {
                println!("{:22}    {:<14}  {}", dt_str, "<DIR>", e.name);
            } else {
                println!("{:22}    {:>14}  {}", dt_str, fmt_comma(e.size), e.name);
            }
        }
    }

    #[cfg(unix)]
    for e in entries {
        println!("{:10}  {:>10}  {:>5}  {:>5}  {}  {}",
            e.permission_str(),
            e.size,
            e.uid.map(|u| u.to_string()).unwrap_or_else(|| "?".to_string()),
            e.gid.map(|g| g.to_string()).unwrap_or_else(|| "?".to_string()),
            e.mtime_str(),
            e.name,
        );
    }

    #[cfg(not(any(windows, unix)))]
    for e in entries {
        println!("{:>12}  {}", e.size, e.name);
    }
}

/// 숫자에 천 단위 콤마 추가 (1234567 → "1,234,567")
#[cfg(windows)]
fn fmt_comma(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 { result.push(','); }
        result.push(c);
    }
    result.chars().rev().collect()
}

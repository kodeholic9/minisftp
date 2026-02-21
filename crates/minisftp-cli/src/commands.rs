// miniSFTP CLI Commands
// author: kodeholic (powered by Claude)
//
// 순수 파싱만 담당 (IO 없음, 테스트 용이)

pub enum Command {
    // 리모트 명령
    Ls { path: String },
    Get { remote: String, local: String },
    Put { local: String, remote: String },
    Mkdir { path: String },
    Rm { path: String },
    Pwd,
    Cd { path: String },
    // 로컬 명령 (!ls, !cd, !pwd)
    // 쉘 명령 실행이 아닌 std::fs로 직접 구현 → 플랫폼 독립적
    LocalLs { path: String },
    LocalCd { path: String },
    LocalPwd,
    Help,
    Quit,
    Unknown(String),
}

impl Command {
    pub fn parse(input: &str) -> Self {
        let parts: Vec<&str> = input.trim().split_whitespace().collect();

        if parts.is_empty() {
            return Command::Unknown(String::new());
        }

        // !cmd 형식: 로컬 명령
        if let Some(local_cmd) = parts[0].strip_prefix('!') {
            let cmd = if local_cmd.is_empty() {
                // "! ls" 형식
                match parts.get(1) {
                    Some(c) => *c,
                    None => return Command::Unknown("Usage: !<command>".to_string()),
                }
            } else {
                local_cmd
            };
            let args_start = if local_cmd.is_empty() { 2 } else { 1 };

            return match cmd {
                "ls" => Command::LocalLs {
                    path: parts.get(args_start).unwrap_or(&".").to_string(),
                },
                "cd" => {
                    let path = parts.get(args_start)
                        .unwrap_or(&".")
                        .to_string();
                    Command::LocalCd { path }
                }
                "pwd" => Command::LocalPwd,
                other => Command::Unknown(format!("Unsupported local command: !{}", other)),
            };
        }

        match parts[0] {
            "ls" => Command::Ls {
                path: parts.get(1).unwrap_or(&".").to_string(),
            },
            "get" => {
                if parts.len() < 2 {
                    return Command::Unknown("Usage: get <remote> [local]".to_string());
                }
                let remote = parts[1].to_string();
                let local = parts.get(2)
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| extract_filename(&remote));
                Command::Get { remote, local }
            }
            "put" => {
                if parts.len() < 2 {
                    return Command::Unknown("Usage: put <local> [remote]".to_string());
                }
                let local = parts[1].to_string();
                let remote = parts.get(2)
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| extract_filename(&local));
                Command::Put { local, remote }
            }
            "mkdir" => {
                if parts.len() < 2 {
                    return Command::Unknown("Usage: mkdir <path>".to_string());
                }
                Command::Mkdir { path: parts[1].to_string() }
            }
            "rm" => {
                if parts.len() < 2 {
                    return Command::Unknown("Usage: rm <path>".to_string());
                }
                Command::Rm { path: parts[1].to_string() }
            }
            "pwd"           => Command::Pwd,
            "cd"            => Command::Cd {
                path: parts.get(1).unwrap_or(&"~").to_string(),
            },
            "help" | "?"    => Command::Help,
            "quit" | "exit" => Command::Quit,
            other => Command::Unknown(format!("Unknown command: {}", other)),
        }
    }
}

/// 경로에서 파일명만 추출 ("/remote/path/file.txt" → "file.txt")
fn extract_filename(path: &str) -> String {
    path.rsplit('/')
        .next()
        .and_then(|s| s.rsplit('\\').next())
        .unwrap_or(path)
        .to_string()
}

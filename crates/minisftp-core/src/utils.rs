// miniSFTP Utils
// author: kodeholic (powered by Claude)
//
// 공통 유틸 함수 모음
// - permission_str  : unix permission bits → "drwxr-xr-x" 문자열
// - mtime_str       : unix timestamp → ls -la 스타일 날짜 문자열
// - fmt_size        : 바이트 → 사람이 읽기 좋은 단위 (1.2MB 등)
// - print_progress  : 전송 진척률 표시
// - local_ls        : 로컬 디렉토리 목록 (플랫폼 독립적)
// - resolve_path    : 리모트 상대경로 → 절대경로
// - resolve_local_path : 로컬 상대경로 → 절대경로 (OS 구분자 처리)

use chrono::{DateTime, Datelike, Local, LocalResult, TimeZone, Timelike, Utc};

// ── 포맷 유틸 ────────────────────────────────────────────────────────────────

/// unix permission bits → "drwxr-xr-x" 형식 문자열
pub fn permission_str(p: u32) -> String {
    let file_type = match p & 0o170000 {
        0o040000 => 'd',  // S_IFDIR
        0o120000 => 'l',  // S_IFLNK
        0o060000 => 'b',  // S_IFBLK
        0o020000 => 'c',  // S_IFCHR
        0o010000 => 'p',  // S_IFIFO
        0o140000 => 's',  // S_IFSOCK
        _        => '-',  // S_IFREG or unknown
    };

    const BITS: [(u32, char); 9] = [
        (0o400, 'r'), (0o200, 'w'), (0o100, 'x'),  // owner
        (0o040, 'r'), (0o020, 'w'), (0o010, 'x'),  // group
        (0o004, 'r'), (0o002, 'w'), (0o001, 'x'),  // other
    ];

    let mut s = String::with_capacity(10);
    s.push(file_type);
    for (bit, ch) in BITS {
        s.push(if p & bit != 0 { ch } else { '-' });
    }
    s
}

/// unix timestamp → ls -la 스타일 날짜 문자열
///
/// - 현재 기준 6개월 이내: "Jun 22 15:30"
/// - 그 이상:             "Jun 22  2025"
pub fn mtime_str(ts: u64) -> String {
    let dt: DateTime<Local> = match Utc.timestamp_opt(ts as i64, 0) {
        LocalResult::Single(utc) => utc.with_timezone(&Local),
        _ => return "?".to_string(),
    };

    let now = Local::now();
    let six_months_ago = now - chrono::Duration::days(180);

    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun",
        "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let mon = MONTHS[(dt.month() - 1) as usize];
    let day = dt.day();

    if dt > six_months_ago {
        format!("{} {:2} {:02}:{:02}", mon, day, dt.hour(), dt.minute())
    } else {
        format!("{} {:2}  {}", mon, day, dt.year())
    }
}

/// 바이트 → 사람이 읽기 좋은 단위 문자열
pub fn fmt_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB      { format!("{:.1}GB", bytes as f64 / GB as f64) }
    else if bytes >= MB { format!("{:.1}MB", bytes as f64 / MB as f64) }
    else if bytes >= KB { format!("{:.1}KB", bytes as f64 / KB as f64) }
    else                { format!("{}B",     bytes) }
}

/// 전송 진척률 표시 (\r로 같은 줄 덮어쓰기)
///
/// [=================>    ] 75%  3.2MB / 4.3MB  1.2MB/s
pub fn print_progress(transferred: u64, total: u64, elapsed_secs: f64) {
    let percent = if total > 0 { transferred * 100 / total } else { 0 };
    let speed   = if elapsed_secs > 0.0 { transferred as f64 / elapsed_secs } else { 0.0 };

    let filled = (percent as usize * 20 / 100).min(20);
    let arrow  = if filled < 20 { ">" } else { "" };
    let bar    = format!("{}{}{}", "=".repeat(filled), arrow,
                         " ".repeat(20usize.saturating_sub(filled + 1)));

    print!("\r[{:<20}] {:3}%  {} / {}  {}/s",
        bar, percent,
        fmt_size(transferred),
        fmt_size(total),
        fmt_size(speed as u64),
    );
    use std::io::Write;
    let _ = std::io::stdout().flush();
}

// ── 파일시스템 유틸 ──────────────────────────────────────────────────────────

/// 로컬 디렉토리 목록 조회 (플랫폼 독립적)
///
/// std::fs::read_dir() 사용 → Windows/Linux/macOS 모두 동작
/// FileEntry 반환 (디렉토리 먼저, 이름 오름차순)
pub fn local_ls(path: &str) -> std::io::Result<Vec<crate::sftp::FileEntry>> {
    let mut entries: Vec<crate::sftp::FileEntry> = std::fs::read_dir(path)?
        .filter_map(|res| res.ok())
        .map(|entry| {
            let name   = entry.file_name().to_string_lossy().to_string();
            let meta   = entry.metadata().ok();
            let is_dir = meta.as_ref().map(|m| m.is_dir()).unwrap_or(false);
            let size   = meta.as_ref().map(|m| m.len()).unwrap_or(0);
            let mtime  = meta.as_ref()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs());

            // unix permission bits는 unix 전용 trait으로만 접근 가능
            // Windows에서는 None → permission_str이 "d?????????" / "----------" 반환
            #[cfg(unix)]
            let (permissions, uid, gid) = {
                use std::os::unix::fs::MetadataExt;
                match &meta {
                    Some(m) => (Some(m.mode()), Some(m.uid()), Some(m.gid())),
                    None    => (None, None, None),
                }
            };
            #[cfg(not(unix))]
            let (permissions, uid, gid): (Option<u32>, Option<u32>, Option<u32>) = (None, None, None);

            crate::sftp::FileEntry { name, is_dir, size, permissions, uid, gid, mtime }
        })
        .collect();

    entries.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.cmp(&b.name)));
    Ok(entries)
}

/// 리모트 상대경로 → 절대경로 변환
///
/// - 절대경로 입력 → 그대로 반환
/// - ".." → 부모 디렉토리
/// - "."  → 현재 디렉토리
/// - 나머지 → current/path 조합
pub fn resolve_path(current: &str, path: &str) -> String {
    if path.starts_with('/') {
        path.to_string()
    } else if path == "." {
        current.to_string()
    } else if path == ".." {
        match current.rsplit_once('/') {
            Some((parent, _)) if !parent.is_empty() => parent.to_string(),
            _ => "/".to_string(),
        }
    } else if current == "/" {
        format!("/{}", path)
    } else {
        format!("{}/{}", current, path)
    }
}

/// 로컬 상대경로 → 절대경로 변환 (OS 구분자 처리)
///
/// std::path::Path 사용 → Windows(\), Unix(/) 모두 지원
pub fn resolve_local_path(current: &str, path: &str) -> String {
    use std::path::Path;
    let p = Path::new(path);
    if p.is_absolute() {
        path.to_string()
    } else {
        Path::new(current).join(p).to_string_lossy().to_string()
    }
}

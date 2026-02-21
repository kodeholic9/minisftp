// miniSFTP SFTP (russh-sftp 기반)
// author: kodeholic (powered by Claude)
//
// FileEntry    : 리모트/로컬 공통 파일 엔트리
// ProgressInfo : 전송 진척 정보 (콜백으로 전달)
// TransferResult : get/put 결과
// SftpClient   : ls, get, put, mkdir, rm

use russh_sftp::client::SftpSession;
use russh_sftp::protocol::OpenFlags;
use crate::error::{Error, Result};
use crate::utils::{mtime_str, permission_str};

const CHUNK_SIZE: usize = 64 * 1024; // 64KB

// ── 공통 타입 ─────────────────────────────────────────────────────────────────

/// 파일/디렉토리 엔트리 (리모트/로컬 공통)
///
/// ls, !ls 모두 이 타입으로 반환 → handler에서 동일하게 출력
pub struct FileEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub permissions: Option<u32>,  // unix permission bits (Windows 로컬은 None)
    pub uid: Option<u32>,
    pub gid: Option<u32>,
    pub mtime: Option<u64>,        // unix timestamp
}

impl FileEntry {
    /// "drwxr-xr-x" 형식. permission bits 없으면 is_dir로 최소 표현
    pub fn permission_str(&self) -> String {
        match self.permissions {
            Some(p) => permission_str(p),
            None    => if self.is_dir { "d?????????" } else { "----------" }.to_string(),
        }
    }

    /// ls -la 스타일 날짜 문자열
    pub fn mtime_str(&self) -> String {
        self.mtime.map(mtime_str).unwrap_or_else(|| "?".to_string())
    }
}

/// 전송 진척 정보
///
/// get/put의 on_progress 콜백으로 전달됨
/// CLI: 터미널 진척률 바 출력
/// Tauri: app.emit("transfer_progress", info) 으로 프론트엔드에 전달
#[derive(Debug, Clone)]
pub struct ProgressInfo {
    pub transferred: u64,   // 현재까지 전송된 바이트
    pub total: u64,         // 전체 파일 크기
    pub elapsed_secs: f64,  // 경과 시간 (초)
}

impl ProgressInfo {
    /// 전송률 (0.0 ~ 1.0)
    pub fn ratio(&self) -> f64 {
        if self.total > 0 { self.transferred as f64 / self.total as f64 } else { 0.0 }
    }

    /// 퍼센트 (0 ~ 100)
    pub fn percent(&self) -> u64 {
        (self.ratio() * 100.0) as u64
    }

    /// 전송 속도 (bytes/sec)
    pub fn speed(&self) -> u64 {
        if self.elapsed_secs > 0.0 {
            (self.transferred as f64 / self.elapsed_secs) as u64
        } else {
            0
        }
    }

    /// 남은 시간 추정 (초), 속도가 0이면 None
    pub fn eta_secs(&self) -> Option<u64> {
        let speed = self.speed();
        if speed > 0 {
            let remaining = self.total.saturating_sub(self.transferred);
            Some(remaining / speed)
        } else {
            None
        }
    }
}

/// get/put 전송 결과
pub enum TransferResult {
    Skipped,        // 크기 + mtime 동일 → 건너뜀
    Resumed(u64),   // 이어받기/이어올리기 완료 → 총 바이트
    Completed(u64), // 새로 전송 완료 → 총 바이트
}

// ── SftpClient ────────────────────────────────────────────────────────────────

pub struct SftpClient {
    sftp: SftpSession,
}

impl SftpClient {
    pub fn new(sftp: SftpSession) -> Self {
        Self { sftp }
    }

    pub async fn ls(&mut self, path: &str) -> Result<Vec<FileEntry>> {
        let dir = self.sftp.read_dir(path)
            .await
            .map_err(|e| Error::Protocol(e.to_string()))?;

        let mut entries: Vec<FileEntry> = dir.into_iter()
            .filter(|e| e.file_name() != "." && e.file_name() != "..")
            .map(|e| {
                let attrs = e.metadata();
                let permissions = attrs.permissions;
                let is_dir = permissions
                    .map(|p| p & 0o170000 == 0o040000)
                    .unwrap_or(false);
                FileEntry {
                    name: e.file_name().to_string(),
                    is_dir,
                    size: attrs.size.unwrap_or(0),
                    permissions,
                    uid: attrs.uid,
                    gid: attrs.gid,
                    mtime: attrs.mtime.map(|t| t as u64),
                }
            })
            .collect();

        entries.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.cmp(&b.name)));
        Ok(entries)
    }

    /// 리모트 파일 다운로드 (FileZilla 방식)
    ///
    /// 1. 크기 + mtime 모두 같음 → Skip
    /// 2. 로컬 < 서버             → Resume (서버에서 seek)
    /// 3. 그 외                   → Overwrite
    ///
    /// on_progress: 청크마다 호출되는 콜백
    /// - CLI:   |p| print_progress(p)
    /// - Tauri: |p| app.emit("transfer_progress", p)
    pub async fn get<F>(&mut self, remote: &str, local: &str, on_progress: F) -> Result<TransferResult>
    where
        F: Fn(ProgressInfo),
    {
        use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

        let remote_meta  = self.sftp.metadata(remote).await
            .map_err(|e| Error::Protocol(e.to_string()))?;
        let remote_size  = remote_meta.size.unwrap_or(0);
        let remote_mtime = remote_meta.mtime.unwrap_or(0) as u64;

        let local_meta   = tokio::fs::metadata(local).await.ok();
        let local_size   = local_meta.as_ref().map(|m| m.len()).unwrap_or(0);
        let local_mtime  = local_meta.as_ref()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        if local_size == remote_size && local_mtime == remote_mtime {
            return Ok(TransferResult::Skipped);
        }

        let offset    = if local_size > 0 && local_size < remote_size { local_size } else { 0 };
        let is_resume = offset > 0;

        let mut remote_file = self.sftp.open(remote).await
            .map_err(|e| Error::Protocol(e.to_string()))?;

        if offset > 0 {
            remote_file.seek(std::io::SeekFrom::Start(offset)).await
                .map_err(|e| Error::Protocol(e.to_string()))?;
        }

        let mut local_file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(is_resume)
            .write(!is_resume)
            .truncate(!is_resume)
            .open(local).await
            .map_err(|e| Error::Io(e))?;

        let mut buf         = vec![0u8; CHUNK_SIZE];
        let mut transferred = offset;
        let start           = std::time::Instant::now();

        loop {
            let n = remote_file.read(&mut buf).await
                .map_err(|e| Error::Protocol(e.to_string()))?;
            if n == 0 { break; }

            local_file.write_all(&buf[..n]).await
                .map_err(|e| Error::Io(e))?;

            transferred += n as u64;
            on_progress(ProgressInfo {
                transferred,
                total: remote_size,
                elapsed_secs: start.elapsed().as_secs_f64(),
            });
        }

        if is_resume { Ok(TransferResult::Resumed(transferred)) }
        else         { Ok(TransferResult::Completed(transferred)) }
    }

    /// 로컬 파일 업로드 (FileZilla 방식)
    ///
    /// 1. 크기 + mtime 모두 같음 → Skip
    /// 2. 서버 < 로컬             → Resume (로컬에서 seek)
    /// 3. 그 외                   → Overwrite
    ///
    /// on_progress: 청크마다 호출되는 콜백
    pub async fn put<F>(&mut self, local: &str, remote: &str, on_progress: F) -> Result<TransferResult>
    where
        F: Fn(ProgressInfo),
    {
        use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

        let local_meta   = tokio::fs::metadata(local).await
            .map_err(|e| Error::Io(e))?;
        let local_size   = local_meta.len();
        let local_mtime  = local_meta.modified().ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let remote_meta  = self.sftp.metadata(remote).await.ok();
        let remote_size  = remote_meta.as_ref().and_then(|m| m.size).unwrap_or(0);
        let remote_mtime = remote_meta.as_ref().and_then(|m| m.mtime).unwrap_or(0) as u64;

        if remote_size == local_size && remote_mtime == local_mtime {
            return Ok(TransferResult::Skipped);
        }

        let offset    = if remote_size > 0 && remote_size < local_size { remote_size } else { 0 };
        let is_resume = offset > 0;

        let mut local_file = tokio::fs::File::open(local).await
            .map_err(|e| Error::Io(e))?;

        if offset > 0 {
            local_file.seek(std::io::SeekFrom::Start(offset)).await
                .map_err(|e| Error::Io(e))?;
        }

        // APPEND 단독으로는 기존 파일을 못 여는 서버가 있음 → WRITE | APPEND 조합
        let mut remote_file = if is_resume {
            self.sftp.open_with_flags(remote, OpenFlags::WRITE | OpenFlags::APPEND).await
        } else {
            self.sftp.open_with_flags(remote,
                OpenFlags::CREATE | OpenFlags::WRITE | OpenFlags::TRUNCATE).await
        }.map_err(|e| Error::Protocol(e.to_string()))?;

        let mut buf         = vec![0u8; CHUNK_SIZE];
        let mut transferred = offset;
        let start           = std::time::Instant::now();

        loop {
            let n = local_file.read(&mut buf).await
                .map_err(|e| Error::Io(e))?;
            if n == 0 { break; }

            remote_file.write_all(&buf[..n]).await
                .map_err(|e| Error::Protocol(e.to_string()))?;

            transferred += n as u64;
            on_progress(ProgressInfo {
                transferred,
                total: local_size,
                elapsed_secs: start.elapsed().as_secs_f64(),
            });
        }

        if is_resume { Ok(TransferResult::Resumed(transferred)) }
        else         { Ok(TransferResult::Completed(transferred)) }
    }

    pub async fn mkdir(&mut self, path: &str) -> Result<()> {
        self.sftp.create_dir(path).await
            .map_err(|e| Error::Protocol(e.to_string()))?;
        Ok(())
    }

    pub async fn rm(&mut self, path: &str) -> Result<()> {
        self.sftp.remove_file(path).await
            .map_err(|e| Error::Protocol(e.to_string()))?;
        Ok(())
    }
}

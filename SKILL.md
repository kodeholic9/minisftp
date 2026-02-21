# minisftp 개발 컨텍스트 Skill

이 파일은 새로운 채팅창에서 minisftp 프로젝트를 이어서 작업할 때
Claude(김대리)가 빠르게 컨텍스트를 파악하기 위한 skill입니다.

---

## 프로젝트 개요

**목적**: Rust로 SSH/SFTP 클라이언트를 직접 구현하는 학습 프로젝트  
**특징**: raw SSH 구현이 아니라 russh/russh-sftp 라이브러리 기반  
**최종 목표**: CLI + Tauri GUI 두 가지 인터페이스 제공

---

## 작업 경로

```
C:\work\github\minisftp\       ← 메인 프로젝트 (허용된 디렉토리)
C:\work\github\minisftp-gui\   ← Tauri GUI 프로젝트 (미착수)
```

---

## 프로젝트 구조

```
minisftp/
├── Cargo.toml                  ← workspace 정의
├── SKILL.md                    ← 이 파일
├── README.md
├── LICENSE-MIT
├── LICENSE-APACHE
└── crates/
    ├── minisftp-core/          ← 핵심 라이브러리 (crates.io 배포됨)
    │   └── src/
    │       ├── lib.rs
    │       ├── config.rs       ← ConnectConfig, AuthMethod
    │       ├── error.rs        ← Error, Result
    │       ├── state.rs        ← ConnectionState, ConnectionObserver trait
    │       ├── session.rs      ← SftpSession (russh 연결 관리)
    │       ├── sftp.rs         ← SftpClient (핵심 SFTP 동작)
    │       └── utils.rs        ← fmt_size, print_progress, local_ls 등
    └── minisftp-cli/           ← CLI 바이너리 (crates.io 배포됨)
        └── src/
            ├── main.rs         ← 인자 파싱, 패스워드 입력
            ├── commands.rs     ← Command enum, parse()
            └── handler.rs      ← interactive shell 루프
```

---

## workspace Cargo.toml 핵심 의존성

```toml
[workspace.dependencies]
tokio      = { version = "1",   features = ["full"] }
tokio-util = { version = "0.7", features = ["rt"] }
russh      = "0.45.0"
russh-sftp = "2.0"
chrono     = { version = "0.4", features = ["std"] }
```

minisftp-cli에만 추가:
```toml
ctrlc = { version = "3", features = ["termination"] }   # 현재 미사용, 추후 필요시
```

---

## 구현 완료 기능

### minisftp-core / sftp.rs

| 타입 | 설명 |
|------|------|
| `FileEntry` | 파일/디렉토리 엔트리 (리모트/로컬 공통) |
| `ProgressInfo` | 전송 진척 정보. ratio/percent/speed/eta_secs 메서드 포함 |
| `TransferResult` | Skipped / Resumed(u64) / Completed(u64) / Cancelled(u64) |
| `CancellationToken` | tokio_util re-export. CLI는 ctrl_c, Tauri는 버튼으로 cancel() |
| `SftpClient::ls()` | 디렉토리 목록. 디렉토리 먼저, 알파벳 정렬 |
| `SftpClient::get()` | 다운로드. FileZilla 방식 (skip/resume/overwrite) + cancel 지원 |
| `SftpClient::put()` | 업로드. 동일 방식 + cancel 지원 |
| `SftpClient::mkdir()` | 디렉토리 생성 |
| `SftpClient::rm()` | 파일 삭제 |
| `SftpClient::realpath()` | SSH_FXP_REALPATH → 절대경로 반환 (sftp.canonicalize 래핑) |
| `SftpClient::pwd()` | realpath(current_dir) 래핑. 의도 명확화용 |

### get/put cancel 동작 방식
```
handler.rs: tokio::spawn → ctrl_c 수신 → token.cancel()
sftp.rs:    loop 내 select! { biased; cancel.cancelled() => drop(remote_file); return Cancelled(n) }
```
`drop(remote_file)` 명시 → SSH_FXP_CLOSE 전송 → "channel closed" 경고 방지

### minisftp-cli / handler.rs

| 명령 | 동작 |
|------|------|
| `ls [path]` | 리모트 디렉토리 목록 |
| `cd <path>` | 리모트 디렉토리 이동 (클라이언트 추적) |
| `pwd` | 서버에 직접 확인 (방식 B: realpath 호출) |
| `get <remote> [local]` | 다운로드 + ctrl_c 취소 |
| `put <local> [remote]` | 업로드 + ctrl_c 취소 |
| `mkdir <path>` | 리모트 디렉토리 생성 |
| `rm <path>` | 리모트 파일 삭제 |
| `!ls [path]` | 로컬 디렉토리 목록 (Windows: dir 스타일, Unix: ls -la 스타일) |
| `!cd <path>` | 로컬 디렉토리 이동 |
| `!pwd` | 로컬 현재 경로 출력 |

### 접속 직후 remote_dir 초기화 (방식 A)
```rust
let mut remote_dir = sftp.realpath(".").await.unwrap_or_else(|_| ".".to_string());
```
서버 홈 디렉토리를 바로 알 수 있음 (이전엔 "."으로 고정)

---

## 핵심 설계 원칙

### 1. core는 플랫폼 무관
- `println!()` 없음. 모든 출력은 콜백으로
- `on_progress: F` 콜백 → CLI는 터미널 출력, Tauri는 app.emit()
- `CancellationToken` → CLI는 ctrl_c, Tauri는 invoke("sftp_cancel")

### 2. Tauri 연동을 항상 염두
```rust
// CLI
sftp.get(remote, local, |p| print_progress(...), token).await

// Tauri (예정)
sftp.get(remote, local, |p| app.emit("progress", p), token).await
```

### 3. FileZilla 방식 skip/resume
- 크기 + mtime 동일 → Skipped
- 로컬 < 서버 → Resumed (이어받기)
- 그 외 → Completed (덮어쓰기)

---

## crates.io 배포 현황

| 크레이트 | 버전 | 상태 |
|---------|------|------|
| minisftp-core | 0.1.0 | 배포 완료 |
| minisftp-cli  | 0.1.0 | 배포 완료 |

**주의**: 로컬 개발 시 minisftp-cli/Cargo.toml은 path 참조 사용
```toml
minisftp-core = { path = "../minisftp-core" }   # 개발 중
# minisftp-core = { version = "0.1.0" }          # publish 직전에만 교체
```

GitHub: https://github.com/kodeholic9/minisftp

---

## 실행 방법

```powershell
# 빌드
cargo build

# 실행
cargo run -p minisftp-cli -- tgkang@192.168.0.29 17932

# 디버그 로그
$env:RUST_LOG="debug"; cargo run -p minisftp-cli -- tgkang@192.168.0.29 17932
```

---

## 다음 작업 후보

### 우선순위 높음
- [ ] **디렉토리 get/put** — 재귀 전송
  - `get_dir` / `put_dir` 추가 또는 `get()`에서 자동 분기
  - 취소 시 루프 바깥에서도 `cancel.is_cancelled()` 체크 필요
  - `ProgressInfo`에 `total_files`, `current_file_index` 추가 고려
  - 에러 처리 정책: 파일 하나 실패 시 전체 중단 vs 목록 수집 후 보고

### 우선순위 중간
- [ ] **Tauri GUI 연동** — minisftp-gui 프로젝트 시작
  - AppState에 `cancel_token: Mutex<Option<CancellationToken>>` 패턴
  - `sftp_cancel` tauri command → token.cancel()
  - progress 이벤트: `app.emit("transfer_progress", ProgressInfo)`

### 기타
- [ ] `ls -la` 옵션 파싱 (현재 `ls -la`는 에러)
- [ ] `rename` / `mv` 명령
- [ ] 공개키 인증 (현재 패스워드만)
- [ ] v0.1.1 배포 (realpath, pwd, CancellationToken 추가분)

---

## 알려진 이슈 / 주의사항

1. `ls -la` → "No such file: -la" 에러. 옵션 파싱 미구현
2. `cd` 후 `pwd` → 심볼릭 링크 해소됨 (의도된 동작)
3. Windows에서 `!ls` 날짜의 AM/PM이 "오전/오후"로 하드코딩됨
4. SFTP 파일 핸들을 명시적으로 drop하지 않으면 "channel closed" WARN 발생
   → Cancelled 반환 시 `drop(remote_file)` 필수

---

## 개발자 정보

- author: kodeholic (powered by Claude)
- Claude 역할: 김대리 (부장님 지시에 따라 코딩)
- 스타일: 이론보다 실용, 현장에서 통하는 코드

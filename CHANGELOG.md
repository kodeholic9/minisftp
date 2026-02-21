# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Added
- `SftpClient::realpath()` — SSH_FXP_REALPATH 기반 절대경로 확인
- `SftpClient::pwd()` — realpath() 래핑, 의도 명확화
- `TransferResult::Cancelled(u64)` — 취소 시점까지 전송된 바이트 반환
- `CancellationToken` re-export — tokio_util::sync::CancellationToken 외부 노출
- 전송 취소 지원 — get()/put()에 CancellationToken 파라미터 추가
- CLI ctrl_c 취소 — 전송 중 ^C 시 Cancelled 반환 후 sftp> 프롬프트 복귀
- `SKILL.md` — 새 채팅창에서 프로젝트 컨텍스트 복원용 문서

### Changed
- 접속 직후 `remote_dir` 초기화 방식 변경
  - 이전: `String::from(".")` (클라이언트 하드코딩)
  - 이후: `sftp.realpath(".")` (서버에 실제 홈 디렉토리 확인)
- `pwd` 명령 동작 변경
  - 이전: 클라이언트 변수 출력
  - 이후: `sftp.pwd()` 호출 → 서버에 직접 확인 (심볼릭 링크 해소)
- get()/put() 시그니처 변경 — `cancel: CancellationToken` 파라미터 추가
- Cancelled 반환 시 `drop(remote_file)` 명시 → SSH_FXP_CLOSE 전송, "channel closed" 경고 방지

### Dependencies
- `tokio-util = { version = "0.7", features = ["rt"] }` workspace에 추가
- `ctrlc = { version = "3", features = ["termination"] }` minisftp-cli에 추가

---

## [0.1.0] - 2026-02-14

### Added
- 최초 릴리즈
- SSH 연결 및 패스워드 인증
- SFTP 기본 명령: ls, cd, pwd, get, put, mkdir, rm
- 로컬 명령: !ls, !cd, !pwd
- FileZilla 방식 파일 전송: Skip / Resume / Overwrite
- 청크 전송 + 진척률 콜백 (ProgressInfo)
- 플랫폼별 로컬 ls 포맷 (Windows: dir 스타일, Unix: ls -la 스타일)
- crates.io 배포: minisftp-core v0.1.0, minisftp-cli v0.1.0
- GitHub: https://github.com/kodeholic9/minisftp

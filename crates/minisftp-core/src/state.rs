// miniSFTP Connection State Machine + Observer
// author: kodeholic (powered by Claude)
//
// SSH 연결 라이프사이클을 상태 머신으로 관리
// can_transition_to()로 허용된 전이만 가능하게 강제
//
// 상태 흐름:
//   Idle → TcpConnecting → VersionExchange → KeyExchange
//     → Encrypted → Authenticating → Authenticated
//     → ChannelOpening → SftpReady → Disconnecting → Disconnected
//
//   어느 상태에서든 → Disconnecting, Error 전이 가능

#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    Idle,
    TcpConnecting,
    VersionExchange,
    KeyExchange,
    Encrypted,
    Authenticating,
    Authenticated,
    ChannelOpening,
    SftpReady,
    Disconnecting,
    Disconnected,
    Error {
        state: Box<ConnectionState>,  // 에러 발생 시점의 상태
        message: String,
    },
}

impl ConnectionState {
    /// 허용된 다음 상태인지 검증
    pub fn can_transition_to(&self, next: &ConnectionState) -> bool {
        use ConnectionState::*;
        matches!(
            (self, next),
            (Idle, TcpConnecting)
            | (TcpConnecting, VersionExchange)
            | (VersionExchange, KeyExchange)
            | (KeyExchange, Encrypted)
            | (Encrypted, Authenticating)
            | (Authenticating, Authenticated)
            | (Authenticated, ChannelOpening)
            | (ChannelOpening, SftpReady)
            | (_, Disconnecting)
            | (Disconnecting, Disconnected)
            | (_, Error { .. })
        )
    }
}

/// 상태 변경 알림 trait
///
/// CLI: println으로 상태 출력
/// Tauri: app_handle.emit()으로 프론트엔드 통지
/// 동일한 trait을 구현하면 UI 교체 가능
pub trait ConnectionObserver: Send + Sync {
    fn on_state_changed(&self, prev: &ConnectionState, next: &ConnectionState);
}

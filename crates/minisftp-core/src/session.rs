// miniSFTP Session (russh 기반)
// author: kodeholic (powered by Claude)
//
// russh로 SSH 연결/인증을 처리하고
// russh-sftp로 SFTP 세션을 수립합니다.

use std::sync::Arc;
use russh::client;
use russh_sftp::client::SftpSession as RusshSftpSession;

use crate::config::{ConnectConfig, AuthMethod};
use crate::error::{Error, Result};
use crate::state::{ConnectionState, ConnectionObserver};
use crate::sftp::SftpClient;

// russh 클라이언트 핸들러 (서버 이벤트 처리)
struct ClientHandler;

#[async_trait::async_trait]
impl client::Handler for ClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &russh::keys::key::PublicKey,
    ) -> std::result::Result<bool, Self::Error> {
        // 학습용: 호스트키 검증 생략 (실서비스에선 known_hosts 확인 필요)
        Ok(true)
    }
}

pub struct SftpSession {
    state: ConnectionState,
    observer: Box<dyn ConnectionObserver>,
}

impl SftpSession {
    pub fn new(observer: Box<dyn ConnectionObserver>) -> Self {
        Self { state: ConnectionState::Idle, observer }
    }

    pub fn state(&self) -> &ConnectionState { &self.state }

    fn transition(&mut self, next: ConnectionState) -> Result<()> {
        if !self.state.can_transition_to(&next) {
            return Err(Error::InvalidTransition { from: self.state.clone(), to: next });
        }
        let prev = std::mem::replace(&mut self.state, next);
        self.observer.on_state_changed(&prev, &self.state);
        Ok(())
    }

    fn transition_to_error(&mut self, message: String) {
        let current = self.state.clone();
        let prev = std::mem::replace(
            &mut self.state,
            ConnectionState::Error { state: Box::new(current), message }
        );
        self.observer.on_state_changed(&prev, &self.state);
    }

    pub async fn connect(&mut self, config: &ConnectConfig) -> Result<SftpClient> {
        // ---- TCP 연결 ----
        self.transition(ConnectionState::TcpConnecting)?;

        let russh_config = Arc::new(client::Config::default());
        let handler = ClientHandler;

        let addr = config.addr();
        tracing::info!("[session] connecting to {}", addr);

        let mut ssh = client::connect(russh_config, addr, handler)
            .await
            .map_err(|e| {
                self.transition_to_error(e.to_string());
                Error::Protocol(e.to_string())
            })?;

        self.transition(ConnectionState::VersionExchange)?;
        self.transition(ConnectionState::KeyExchange)?;

        // ---- 인증 ----
        self.transition(ConnectionState::Encrypted)?;
        self.transition(ConnectionState::Authenticating)?;

        let authed = match &config.auth {
            AuthMethod::Password(pw) => {
                ssh.authenticate_password(&config.username, pw)
                    .await
                    .map_err(|e| Error::Auth(e.to_string()))?
            }
            AuthMethod::PublicKey { .. } => {
                return Err(Error::Auth("PublicKey auth not implemented".to_string()));
            }
        };

        if !authed {
            self.transition_to_error("Authentication failed".to_string());
            return Err(Error::Auth("Authentication failed".to_string()));
        }
        self.transition(ConnectionState::Authenticated)?;

        // ---- 채널 + SFTP ----
        self.transition(ConnectionState::ChannelOpening)?;

        let channel = ssh.channel_open_session()
            .await
            .map_err(|e| Error::Protocol(e.to_string()))?;

        channel.request_subsystem(true, "sftp")
            .await
            .map_err(|e| Error::Protocol(e.to_string()))?;

        let sftp = RusshSftpSession::new(channel.into_stream())
            .await
            .map_err(|e| Error::Protocol(e.to_string()))?;

        self.transition(ConnectionState::SftpReady)?;

        tracing::info!("[session] SFTP ready");
        Ok(SftpClient::new(sftp))
    }
}

// miniSFTP Error Types
// author: kodeholic (powered by Claude)
//
// thiserror를 사용하지 않고 직접 구현하여 Rust 에러 처리 학습
// Display: 에러 메시지 포맷팅
// From<io::Error>: ? 연산자로 IO 에러 자동 변환

use std::fmt;
use crate::state::ConnectionState;

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    InvalidTransition {
        from: ConnectionState,
        to: ConnectionState,
    },
    Protocol(String),
    Auth(String),
    Sftp(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e)                          => write!(f, "IO error: {}", e),
            Error::InvalidTransition { from, to } => write!(f, "Invalid state transition: {:?} → {:?}", from, to),
            Error::Protocol(s)                    => write!(f, "Protocol error: {}", s),
            Error::Auth(s)                        => write!(f, "Auth failed: {}", s),
            Error::Sftp(s)                        => write!(f, "SFTP error: {}", s),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

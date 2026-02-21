// miniSFTP Connection Config
// author: kodeholic (powered by Claude)
//
// Tauri 연계 시 serde derive 추가하면 JSON 직렬화 가능

use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct ConnectConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth: AuthMethod,
}

#[derive(Debug, Clone)]
pub enum AuthMethod {
    Password(String),
    PublicKey { private_key_path: PathBuf },
}

impl ConnectConfig {
    pub fn addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

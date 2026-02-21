# minisftp

A minimal SFTP client library and CLI written in Rust, built on [russh](https://github.com/warp-tech/russh) and [russh-sftp](https://github.com/AspectUnk/russh-sftp).

## Crates

| Crate | Description |
|---|---|
| `minisftp-core` | Core SFTP library (platform-agnostic) |
| `minisftp-cli` | Interactive CLI client |

## Features

- SSH connection with password authentication
- Directory listing (`ls`) with Unix-style permissions and timestamps
- File transfer (`get` / `put`) with progress callback
- Resume support (FileZilla-style: size + mtime comparison)
- Local filesystem commands (`!ls`, `!cd`, `!pwd`)
- Platform-independent local listing (Windows / Linux / macOS)
- Connection state machine with observer pattern

## Usage

### CLI

```bash
cargo run -p minisftp-cli -- user@host [port]
```

```
sftp> ls
sftp> get remote_file.txt
sftp> put local_file.txt
sftp> !ls
sftp> help
```

### Library

```rust
use minisftp_core::config::{AuthMethod, ConnectConfig};
use minisftp_core::session::SftpSession;
use minisftp_core::state::ConnectionObserver;

let config = ConnectConfig {
    host: "192.168.0.1".to_string(),
    port: 22,
    username: "user".to_string(),
    auth: AuthMethod::Password("password".to_string()),
};

let mut session = SftpSession::new(Box::new(observer));
let mut sftp = session.connect(&config).await?;

// List directory
let entries = sftp.ls(".").await?;

// Download with progress
sftp.get("remote.zip", "local.zip", |p| {
    println!("{}/{} bytes ({:.1}%)", p.transferred, p.total, p.ratio() * 100.0);
}).await?;

// Upload with progress
sftp.put("local.zip", "remote.zip", |p| {
    println!("{}/s", p.speed());
}).await?;
```

## License

MIT OR Apache-2.0

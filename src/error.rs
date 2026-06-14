use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Connection timed out after {0}s")]
    ConnectionTimeout(u32),

    #[error("Connection refused to {0}:{1}")]
    ConnectionRefused(String, u16),

    #[error("Host not found: {0}")]
    HostNotFound(String),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Authentication error: {0}")]
    Auth(String),

    #[error("Maximum authentication retries ({0}) exceeded")]
    AuthMaxRetries(u8),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Session closed: {0}")]
    SessionClosed(String),

    #[error("Invalid IAC sequence byte: {0}")]
    InvalidIacSequence(u8),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Terminal error: {0}")]
    Terminal(String),
}

pub type Result<T> = std::result::Result<T, Error>;

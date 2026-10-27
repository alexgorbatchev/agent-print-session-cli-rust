pub mod aggregate;
pub mod cli;
pub mod render;
pub mod resolve;
pub mod transcript;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Harness {
    Claude,
    Codex,
    Pi,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Arguments(#[from] clap::Error),
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Json(#[from] serde_json::Error),
    #[error("session not found: {0}")]
    NotFound(String),
}

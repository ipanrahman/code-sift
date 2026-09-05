//! Error types for CodeSift operations.

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("repository error: {0}")]
    Repository(String),

    #[error("index error: {0}")]
    Index(String),

    #[error("parse error: {0}")]
    Parse(String),

    #[error("search error: {0}")]
    Search(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("symbol not found: {0}")]
    SymbolNotFound(String),

    #[error("file not found: {0}")]
    FileNotFound(String),
}

pub type Result<T> = std::result::Result<T, Error>;

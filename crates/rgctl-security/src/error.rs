//! Security crate errors.

use std::io;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SecurityError {
    #[error("OSV parse error: {0}")]
    OsvParse(String),
    #[error("unsupported ecosystem: {0}")]
    UnsupportedEcosystem(String),
    #[error("IO error on {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: io::Error,
    },
    #[error("{0}")]
    Msg(String),
}

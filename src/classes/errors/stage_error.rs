use std::fmt;

/// Errors returned by stages inside the retry loop.
///
/// Error messages are freeform strings. The classification of retryable vs
/// fatal is decided at the call site, not enforced by the type system.
pub enum StageError {
    /// The error is specific to the current torrent. A different torrent may succeed.
    Retryable(String),
    /// Infrastructure is broken. Retrying with a different torrent will not help.
    Fatal(String),
}

impl fmt::Display for StageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StageError::Retryable(msg) | StageError::Fatal(msg) => write!(f, "{msg}"),
        }
    }
}

impl From<qbit_rs::Error> for StageError {
    fn from(e: qbit_rs::Error) -> Self {
        StageError::Retryable(e.to_string())
    }
}

impl From<Box<dyn std::error::Error>> for StageError {
    fn from(e: Box<dyn std::error::Error>) -> Self {
        StageError::Retryable(e.to_string())
    }
}

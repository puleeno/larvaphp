//! Error types for LarvaPHP.
//!
//! Mirrors the error definitions in `frankenphp.go`.

use std::fmt;

/// Result type alias for LarvaPHP.
pub type Result<T> = std::result::Result<T, Error>;

/// Main error type for LarvaPHP operations.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("not a LarvaPHP request")]
    InvalidRequest,

    #[error("LarvaPHP is already started")]
    AlreadyStarted,

    #[error("LarvaPHP is only compatible with PHP 8.2+")]
    InvalidPHPVersion,

    #[error("LarvaPHP is not compatible with Zend Signals, recompile PHP with --disable-zend-signals")]
    ZendSignals,

    #[error("error creating the main thread")]
    MainThreadCreation,

    #[error("error creating a PHP thread")]
    ThreadCreation,

    #[error("error during PHP script execution")]
    ScriptExecution,

    #[error("server is not registered, you must first call init() with WithServer()")]
    NotRunning,

    #[error("maximum request handling time exceeded")]
    MaxWaitTimeExceeded,

    #[error("request body too large")]
    RequestBodyTooLarge,

    #[error("max amount of overall threads reached")]
    MaxThreadsReached,

    #[error("split path contains non-ASCII characters")]
    InvalidSplitPath,

    #[error("unknown worker: {0}")]
    UnknownWorker(String),

    #[error("worker filename is invalid {0:?}: {1}")]
    InvalidWorkerFilename(String, String),

    #[error("worker file not found {0:?}: {1}")]
    WorkerFileNotFound(String, String),

    #[error("two global workers cannot have the same filename: {0:?}")]
    DuplicateWorkerFilename(String),

    #[error("worker {0:?} has a request matcher but no server scope")]
    WorkerMatcherWithoutServer(String),

    #[error("two workers in a server cannot have the same filename: {0:?}")]
    DuplicateServerWorkerFilename(String),

    #[error("two workers cannot have the same name: {0:?}")]
    DuplicateWorkerName(String),

    #[error("invalid response: {0}")]
    Rejected(RejectedError),

    #[error("failed to initialize workers: {0}")]
    WorkerInit(String),

    #[error("invalid Content-Length header: {0:?}")]
    InvalidContentLengthHeader(String),

    #[error("invalid request path")]
    InvalidRequestPath,

    #[error("invalid script path")]
    InvalidScriptPath,

    #[error("watcher support is not enabled")]
    WatcherNotEnabled,

    #[error("embedded app error: {0}")]
    EmbeddedApp(String),

    #[error("PHP FFI error: {0}")]
    Ffi(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("channel error: {0}")]
    Channel(String),

    #[error("thread error: {0}")]
    Thread(String),

    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("ACME / Let's Encrypt error: {0}")]
    Acme(String),
}

/// Mirrors the `ErrRejected` type from `frankenphp.go`.
#[derive(Debug, Clone)]
pub struct RejectedError {
    pub message: String,
    pub status: u16,
}

impl fmt::Display for RejectedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl RejectedError {
    pub fn new(message: impl Into<String>, status: u16) -> Self {
        Self {
            message: message.into(),
            status,
        }
    }
}

/// Convenience constructors for common rejected errors.
pub fn err_invalid_request_path() -> Error {
    Error::Rejected(RejectedError::new("invalid request path", 400))
}

pub fn err_invalid_script_path() -> Error {
    Error::Rejected(RejectedError::new("invalid script path", 404))
}

pub fn err_invalid_content_length_header(msg: &str) -> Error {
    Error::Rejected(RejectedError::new(
        format!("invalid Content-Length header: {:?}", msg),
        400,
    ))
}

pub fn err_max_wait_time_exceeded() -> Error {
    Error::Rejected(RejectedError::new(
        "maximum request handling time exceeded",
        503,
    ))
}

pub fn err_request_body_too_large() -> Error {
    Error::Rejected(RejectedError::new("request body too large", 413))
}

/// Check if an error is a "rejected" error (has an HTTP status).
pub fn is_rejected(err: &Error) -> bool {
    matches!(err, Error::Rejected(_))
}

/// Get the HTTP status code for a rejected error.
pub fn rejected_status(err: &Error) -> Option<u16> {
    match err {
        Error::Rejected(re) => Some(re.status),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn err_invalid_request_path_is_rejected() {
        let err = err_invalid_request_path();
        assert!(is_rejected(&err));
        assert_eq!(rejected_status(&err), Some(400));
        assert_eq!(err.to_string(), "invalid response: invalid request path");
    }

    #[test]
    fn err_invalid_script_path_is_rejected() {
        let err = err_invalid_script_path();
        assert!(is_rejected(&err));
        assert_eq!(rejected_status(&err), Some(404));
    }

    #[test]
    fn err_invalid_content_length_header_is_rejected() {
        let err = err_invalid_content_length_header("abc");
        assert!(is_rejected(&err));
        assert_eq!(rejected_status(&err), Some(400));
    }

    #[test]
    fn err_max_wait_time_exceeded_is_rejected() {
        let err = err_max_wait_time_exceeded();
        assert!(is_rejected(&err));
        assert_eq!(rejected_status(&err), Some(503));
    }

    #[test]
    fn err_request_body_too_large_is_rejected() {
        let err = err_request_body_too_large();
        assert!(is_rejected(&err));
        assert_eq!(rejected_status(&err), Some(413));
    }

    #[test]
    fn non_rejected_error() {
        let err = Error::InvalidRequest;
        assert!(!is_rejected(&err));
        assert_eq!(rejected_status(&err), None);
    }

    #[test]
    fn rejected_error_display() {
        let re = RejectedError::new("test error", 418);
        assert_eq!(re.message, "test error");
        assert_eq!(re.status, 418);
        assert_eq!(re.to_string(), "test error");
    }

    #[test]
    fn error_variants_display() {
        assert_eq!(Error::InvalidRequest.to_string(), "not a LarvaPHP request");
        assert_eq!(Error::AlreadyStarted.to_string(), "LarvaPHP is already started");
        assert_eq!(Error::NotRunning.to_string(), "server is not registered, you must first call init() with WithServer()");
        assert_eq!(Error::WatcherNotEnabled.to_string(), "watcher support is not enabled");
        assert_eq!(Error::Ffi("test ffi".to_string()).to_string(), "PHP FFI error: test ffi");
        assert_eq!(Error::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "file not found")).to_string(), "IO error: file not found");
    }
}
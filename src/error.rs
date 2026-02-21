/// Error types for svcmgr
use std::fmt;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    /// Git operation error
    Git(String),

    /// IO error
    Io(std::io::Error),

    /// Configuration error
    Config(String),

    /// Invalid argument
    InvalidArgument(String),

    /// Operation not supported
    NotSupported(String),

    /// External command failed
    CommandFailed {
        command: String,
        exit_code: Option<i32>,
        stderr: String,
    },

    /// Generic error
    Other(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Git(msg) => write!(f, "Git error: {}", msg),
            Error::Io(err) => write!(f, "IO error: {}", err),
            Error::Config(msg) => write!(f, "Configuration error: {}", msg),
            Error::InvalidArgument(msg) => write!(f, "Invalid argument: {}", msg),
            Error::NotSupported(msg) => write!(f, "Not supported: {}", msg),
            Error::CommandFailed {
                command,
                exit_code,
                stderr,
            } => {
                write!(f, "Command '{}' failed", command)?;
                if let Some(code) = exit_code {
                    write!(f, " with exit code {}", code)?;
                }
                if !stderr.is_empty() {
                    write!(f, ": {}", stderr)?;
                }
                Ok(())
            }
            Error::Other(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io(err)
    }
}

impl From<git2::Error> for Error {
    fn from(err: git2::Error) -> Self {
        Error::Git(err.message().to_string())
    }
}

impl From<anyhow::Error> for Error {
    fn from(err: anyhow::Error) -> Self {
        Error::Other(err.to_string())
    }
}

use thiserror::Error;

#[derive(Error, Debug)]
#[allow(dead_code)] // Net, Blocked, RateLimited and PlayerMissing land as the P0 client and player wire in.
pub enum AppError {
    #[error("database error: {0}")]
    Db(String),
    #[error("network error: {0}")]
    Net(String),
    #[error("extension blocked: {0}")]
    Blocked(String),
    #[error("rate limited, retry in {0} seconds")]
    RateLimited(u32),
    #[error("player missing: {0}")]
    PlayerMissing(String),
    #[error("extension error: {0}")]
    Extension(String),
    #[error("extension timed out after {0} seconds")]
    ExtensionTimeout(u64),
}

impl From<rusqlite::Error> for AppError {
    fn from(value: rusqlite::Error) -> Self {
        AppError::Db(value.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;

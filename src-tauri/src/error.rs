use thiserror::Error;

#[derive(Error, Debug)]
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
}

pub type AppResult<T> = Result<T, AppError>;

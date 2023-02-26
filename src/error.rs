use cron;
use thiserror;

#[derive(thiserror::Error, Debug)]
pub enum CronError {
    #[error("Invalid cron expression: {0}")]
    ParseError(#[from] cron::error::Error),
    #[error("Unknown error")]
    Unknown,
}

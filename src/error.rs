//! Error types for the CronTab library.
//!
//! This module defines the error types that can be returned by operations
//! in the CronTab library, primarily from parsing cron expressions.

use cron;
use thiserror;

/// Errors that can occur when using CronTab.
///
/// This enum represents all possible errors that can occur during
/// cron job scheduling operations.
///
/// # Examples
///
/// ```rust
/// use cron_tab::{Cron, CronError};
/// use chrono::Utc;
///
/// let mut cron = Cron::new(Utc);
/// 
/// // This will result in a ParseError due to invalid cron expression
/// match cron.add_fn("invalid expression", || println!("Hello")) {
///     Ok(job_id) => println!("Job added with ID: {}", job_id),
///     Err(CronError::ParseError(e)) => println!("Invalid cron expression: {}", e),
///     Err(CronError::Unknown) => println!("An unknown error occurred"),
/// }
/// ```
#[derive(thiserror::Error, Debug)]
pub enum CronError {
    /// Error that occurs when a cron expression cannot be parsed.
    ///
    /// This is the most common error and occurs when an invalid cron expression
    /// is provided to [`add_fn`](crate::Cron::add_fn) or [`add_fn`](crate::AsyncCron::add_fn).
    ///
    /// # Common Causes
    ///
    /// - Invalid field values (e.g., hour > 23, minute > 59)
    /// - Incorrect number of fields (should be 7: sec min hour day month weekday year)
    /// - Invalid special characters or syntax
    /// - Malformed range expressions
    ///
    /// # Examples
    ///
    /// ```rust
    /// use cron_tab::{Cron, CronError};
    /// use chrono::Utc;
    ///
    /// let mut cron = Cron::new(Utc);
    /// 
    /// // These will all result in ParseError:
    /// 
    /// // Too few fields
    /// assert!(matches!(
    ///     cron.add_fn("* * *", || {}),
    ///     Err(CronError::ParseError(_))
    /// ));
    /// 
    /// // Invalid hour (25 > 23)
    /// assert!(matches!(
    ///     cron.add_fn("0 0 25 * * * *", || {}),
    ///     Err(CronError::ParseError(_))
    /// ));
    /// 
    /// // Invalid syntax
    /// assert!(matches!(
    ///     cron.add_fn("not-a-cron-expression", || {}),
    ///     Err(CronError::ParseError(_))
    /// ));
    /// ```
    #[error("Invalid cron expression: {0}")]
    ParseError(#[from] cron::error::Error),
    
    /// A catch-all error for unexpected conditions.
    ///
    /// This error type is reserved for future use and should rarely occur
    /// in normal operation. If you encounter this error, it may indicate
    /// a bug in the library or an unexpected system condition.
    ///
    /// # When This Occurs
    ///
    /// Currently, this error is not actively used by the library but is
    /// included for forward compatibility and unexpected error conditions.
    #[error("Unknown error")]
    Unknown,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_error_display() {
        // Create a simple parse error
        let result: Result<cron::Schedule, cron::error::Error> = "invalid".parse();
        let cron_error = result.unwrap_err();
        let error = CronError::ParseError(cron_error);
        let error_string = format!("{}", error);
        assert!(error_string.len() > 0);
    }

    #[test]
    fn test_parse_error_debug() {
        let result: Result<cron::Schedule, cron::error::Error> = "invalid".parse();
        let cron_error = result.unwrap_err();
        let error = CronError::ParseError(cron_error);
        let debug_string = format!("{:?}", error);
        assert!(debug_string.contains("ParseError"));
    }

    #[test]
    fn test_error_from_cron_error() {
        let result: Result<cron::Schedule, cron::error::Error> = "invalid".parse();
        let cron_error = result.unwrap_err();
        let error: CronError = cron_error.into();
        match error {
            CronError::ParseError(_) => {},
            CronError::Unknown => {},
        }
    }
}

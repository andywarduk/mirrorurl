use std::error::Error;
use std::fmt::Display;

use url::ParseError;

/// Reason for skipping a file
#[derive(Debug)]
pub enum SkipReason {
    Transport,
    SkipList,
    NotRelative,
    Fragment,
    Query,
    NotValid(ParseError),
    RedirectNotRel(String),
    TooManyRedirects,
}

impl Display for SkipReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use SkipReason::*;
        match self {
            Transport => f.write_str("The transport is not supported"),
            SkipList => f.write_str("Path is in the skip list"),
            NotRelative => f.write_str("URL is not relative to the base URL"),
            Fragment => f.write_str("URL is a fragment"),
            Query => f.write_str("URL has a query"),
            NotValid(e) => write!(f, "URL is not valid: {e}"),
            RedirectNotRel(to) => write!(f, "Redirect to {to} is not relative to the base URL"),
            TooManyRedirects => f.write_str("Too many redirects"),
        }
    }
}

/// Error encapsulation a skipped file reason
#[derive(Debug)]
pub struct SkipReasonErr {
    /// The skipped URL
    url: String,
    /// Reason for skipping
    reason: SkipReason,
}

impl SkipReasonErr {
    /// Creates a new skip reason error
    pub fn new(url: String, reason: SkipReason) -> Self {
        Self { url, reason }
    }
}

impl Display for SkipReasonErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Skipping {}: {}", self.url, self.reason)
    }
}

impl Error for SkipReasonErr {}

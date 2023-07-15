use url::Position;
pub use url::Url;

use crate::skipreason::{SkipReason, SkipReasonErr};

pub trait UrlExt {
    /// Returns true if the URL can be handled
    fn is_handled(&self) -> Result<(), SkipReasonErr>;

    /// Returns true if test URL is relative to a base URL
    fn is_relative_to(&self, base_url: &Url) -> bool;

    /// Returns the relative path for a URL from a base URL
    fn relative_path<'a>(&'a self, base_url: &Url) -> Option<&'a str>;

    /// Returns the full path of the URL including query and hash strings
    fn full_path(&self) -> &str;
}

impl UrlExt for Url {
    /// Checks the passed URL can be handled
    fn is_handled(&self) -> Result<(), SkipReasonErr> {
        // Check scheme
        match self.scheme() {
            "http" | "https" => (),
            _ => {
                return Err(SkipReasonErr::new(
                    self.to_string().clone(),
                    SkipReason::Transport,
                ))
            }
        }

        Ok(())
    }

    fn is_relative_to(&self, base_url: &Url) -> bool {
        self.relative_path(base_url).is_some()
    }

    fn relative_path<'a>(&'a self, base_url: &Url) -> Option<&'a str> {
        let base_path = base_url.full_path();

        if self.host_str() == base_url.host_str() && self.full_path().starts_with(base_path) {
            let chop_pos = base_path.len();
            let rel = &self.full_path()[chop_pos..];

            if rel.is_empty() || base_path.ends_with('/') || rel.starts_with('/') {
                // Trim leading slashes from the relative path
                let rel = rel.trim_start_matches('/');

                return Some(rel);
            }
        }

        None
    }

    fn full_path(&self) -> &str {
        &self[Position::BeforePath..]
    }
}

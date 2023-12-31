use once_cell::sync::Lazy;
use reqwest::header::CONTENT_TYPE;
pub use reqwest::Response;

use crate::mime::{Mime, MimeExt};
use crate::output::debug;
use crate::state::ArcState;

/// Extension trait for a reqwest Response
pub trait ResponseExt {
    fn is_html(&self, state: &ArcState) -> bool;
}

/// HMTL MIME type
static MIME_HTML: Lazy<Mime> = Lazy::new(|| "text/html".parse::<Mime>().unwrap());

/// XHTML MIME type
static MIME_XHTML: Lazy<Mime> = Lazy::new(|| "application/xhtml+xml".parse::<Mime>().unwrap());

impl ResponseExt for Response {
    /// Returns true if the response can be parsed as HTML
    fn is_html(&self, state: &ArcState) -> bool {
        // Get content MIME type
        if let Some(mime_type) = self
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<Mime>().ok())
        {
            debug!(state, 2, "MIME type of {} is {mime_type}", self.url());

            // Is it html or xhtml?
            mime_type.equal(&MIME_HTML) || mime_type.equal(&MIME_XHTML)
        } else {
            debug!(
                state,
                1,
                "No content (MIME) type received for {}",
                self.url()
            );

            false
        }
    }
}

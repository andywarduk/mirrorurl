use url::{Position, Url};

/// Returns true if test URL is relative to a base URL
pub fn url_relative_to(test_url: &Url, base_url: &Url) -> bool {
    test_url.host_str() == base_url.host_str()
        && test_url[Position::BeforePath..].starts_with(&base_url[Position::BeforePath..])
}

/// Returns the relative path for a URL from a base URL
pub fn url_relative_path<'a>(url: &'a Url, base_url: &Url) -> Option<&'a str> {
    if url_relative_to(url, base_url) {
        let chop_pos = base_url[Position::BeforePath..].len();
        Some(&url[Position::BeforePath..][chop_pos..])
    } else {
        None
    }
}
